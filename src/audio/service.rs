use crate::audio::pipewire_config::{
    balance_params, filter_chain_with_balance, live_params, skeleton, SKELETON_PREFIX,
};
use crate::config::model::Profile;
use anyhow::{Context, Result};
use directories::BaseDirs;
use std::{fs, path::PathBuf, process::Command, sync::Mutex};

pub use crate::audio::pipewire_config::balance_gains;

const SINK: &str = "pulseforge";
const UNIT: &str = "filter-chain.service";
/// Serializa alterações (UI, CLI em threads) sobre o mesmo arquivo/serviço.
static APPLY_LOCK: Mutex<()> = Mutex::new(());

pub struct FilterService {
    config_path: PathBuf,
}

impl FilterService {
    pub fn new() -> Result<Self> {
        let dir = BaseDirs::new()
            .context("XDG directories unavailable")?
            .config_dir()
            .join("pipewire/filter-chain.conf.d");
        fs::create_dir_all(&dir)?;
        Ok(Self {
            config_path: dir.join("pulseforge.conf"),
        })
    }

    pub fn is_running(&mut self) -> bool {
        systemctl_ok(&["is-active", "--quiet", UNIT])
    }

    pub fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }

    /// Applies a profile (+ balance). If the graph structure did not change
    /// and the filter is running, values are updated live in PipeWire (no
    /// audio dropout, volume and routing untouched); otherwise the
    /// filter-chain is restarted.
    pub fn apply(&mut self, profile: &Profile, output: Option<&str>, balance: f64) -> Result<()> {
        let _guard = APPLY_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let unchanged = self.stored_skeleton().as_deref() == Some(skeleton(profile, output).as_str());
        self.write_config(&filter_chain_with_balance(profile, output, balance))?;
        if unchanged && self.is_running() && set_params(&live_params(profile, balance)).is_ok() {
            return Ok(());
        }
        self.restart()
    }

    /// Updates only the stereo balance, live. Needs a running filter.
    pub fn set_balance(&self, balance: f64) -> Result<()> {
        set_params(&balance_params(balance))
    }

    pub fn stop(&mut self) {
        let _ = Command::new("systemctl")
            .args(["--user", "stop", UNIT])
            .status();
    }

    fn stored_skeleton(&self) -> Option<String> {
        let text = fs::read_to_string(&self.config_path).ok()?;
        text.lines()
            .next()?
            .strip_prefix(SKELETON_PREFIX)
            .map(str::to_owned)
    }

    fn write_config(&self, text: &str) -> Result<()> {
        let tmp = self.config_path.with_extension("conf.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &self.config_path)?;
        Ok(())
    }

    fn restart(&mut self) -> Result<()> {
        // Recreating the sink resets its volume; keep it to restore afterwards.
        let previous = sink_volumes();
        let _ = systemctl_ok(&["reset-failed", UNIT]);
        if !systemctl_ok(&["restart", UNIT]) {
            anyhow::bail!("filter-chain.service restart failed");
        }
        for _ in 0..40 {
            let sink_ready = Command::new("pactl")
                .args(["list", "short", "sinks"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains(SINK))
                .unwrap_or(false);
            if sink_ready && systemctl_ok(&["is-active", "--quiet", UNIT]) {
                let _ = Command::new("pactl").args(["set-default-sink", SINK]).status();
                if let Some((left, right)) = previous {
                    let _ = Command::new("pactl")
                        .args(["set-sink-volume", SINK, &format!("{left}%"), &format!("{right}%")])
                        .status();
                }
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        anyhow::bail!("filter-chain.service did not become active")
    }
}

fn systemctl_ok(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Id of the PipeWire node of the PulseForge sink (where the controls live).
fn node_id() -> Option<String> {
    let out = Command::new("pw-cli").args(["ls", "Node"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut current = None;
    for line in text.lines().map(str::trim) {
        if let Some(rest) = line.strip_prefix("id ") {
            current = rest.split(',').next().map(str::to_owned);
        } else if line == format!("node.name = \"{SINK}\"") {
            return current;
        }
    }
    None
}

/// Máximo de pares nome/valor por chamada do `pw-cli s … Props`.
const PARAMS_PER_CALL: usize = 12;

/// Sets filter controls on the running graph without restarting it.
fn set_params(params: &[(String, f32)]) -> Result<()> {
    let id = node_id().context("sink do PulseForge não encontrado no PipeWire")?;
    // O pw-cli ignora em silêncio listas com mais de ~20 pares: aplica em lotes.
    for batch in params.chunks(PARAMS_PER_CALL) {
        let list: Vec<String> = batch
            .iter()
            .map(|(name, value)| format!("\"{name}\" {value:.6}"))
            .collect();
        let status = Command::new("pw-cli")
            .args(["s", &id, "Props", &format!("{{ params = [ {} ] }}", list.join(" "))])
            .output()
            .context("pw-cli indisponível")?;
        if !status.status.success() || !status.stderr.is_empty() {
            anyhow::bail!("pw-cli não aplicou os parâmetros");
        }
    }
    Ok(())
}

/// Volume (%) of the left and right channels of the PulseForge sink.
fn sink_volumes() -> Option<(u32, u32)> {
    let out = Command::new("pactl").args(["get-sink-volume", SINK]).output().ok()?;
    parse_channel_volumes(&String::from_utf8_lossy(&out.stdout))
}

/// Extrai as porcentagens (L, R) da saída de `pactl get-sink-volume`.
pub fn parse_channel_volumes(text: &str) -> Option<(u32, u32)> {
    let mut values = text
        .split_whitespace()
        .filter_map(|t| t.strip_suffix('%')?.parse::<u32>().ok());
    let left = values.next()?;
    Some((left, values.next().unwrap_or(left)))
}
