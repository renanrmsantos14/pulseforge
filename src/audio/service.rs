use crate::audio::pipewire_config::filter_chain_for_output;
use crate::config::model::Profile;
use anyhow::{Context, Result};
use directories::BaseDirs;
use std::{fs, path::PathBuf, process::Command};

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
        Command::new("systemctl")
            .args(["--user", "is-active", "--quiet", "filter-chain.service"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn config_path(&self) -> &std::path::Path {
        &self.config_path
    }

    pub fn start(&mut self, profile: &Profile) -> Result<()> {
        self.start_for_output(profile, None)
    }

    pub fn start_for_output(&mut self, profile: &Profile, output: Option<&str>) -> Result<()> {
        // Recriar o sink zera o volume/balanço; guardamos L/R para restaurar depois.
        let previous = self.channel_volumes();
        fs::write(&self.config_path, filter_chain_for_output(profile, output))?;
        let _ = Command::new("systemctl")
            .args(["--user", "reset-failed", "filter-chain.service"])
            .status();
        let status = Command::new("systemctl")
            .args(["--user", "restart", "filter-chain.service"])
            .status()
            .context("failed to restart filter-chain.service")?;
        if !status.success() {
            anyhow::bail!("filter-chain.service restart failed");
        }
        for _ in 0..20 {
            let service_active = Command::new("systemctl")
                .args(["--user", "is-active", "--quiet", "filter-chain.service"])
                .status()
                .map(|result| result.success())
                .unwrap_or(false);
            let sink_ready = Command::new("pactl")
                .args(["list", "short", "sinks"])
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).contains("pulseforge"))
                .unwrap_or(false);
            if service_active && sink_ready {
                let _ = Command::new("pactl")
                    .args(["set-default-sink", "pulseforge"])
                    .status();
                if let Some((left, right)) = previous {
                    let _ = Command::new("pactl")
                        .args(["set-sink-volume", "pulseforge", &format!("{left}%"), &format!("{right}%")])
                        .status();
                }
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        anyhow::bail!("filter-chain.service did not become active")
    }

    pub fn stop(&mut self) {
        let _ = Command::new("systemctl")
            .args(["--user", "stop", "filter-chain.service"])
            .status();
    }

    /// Applies stereo balance to the local PulseForge sink without changing the
    /// overall volume. `balance` is in [-1, 1]: negative favors left, positive
    /// favors right. The louder channel keeps the current sink volume.
    pub fn set_balance(&self, balance: f64) -> Result<()> {
        let balance = balance.clamp(-1.0, 1.0);
        let (left, right) = balance_gains(balance);
        let base = self.current_volume_percent().unwrap_or(100.0);
        let left = format!("{:.0}%", base * left);
        let right = format!("{:.0}%", base * right);
        let status = Command::new("pactl")
            .args(["set-sink-volume", "pulseforge", &left, &right])
            .status()
            .context("failed to set PulseForge stereo balance")?;
        if !status.success() {
            anyhow::bail!("unable to set PulseForge stereo balance");
        }
        Ok(())
    }

    /// Volume (%) of the left and right channels of the PulseForge sink.
    fn channel_volumes(&self) -> Option<(u32, u32)> {
        let out = Command::new("pactl")
            .args(["get-sink-volume", "pulseforge"])
            .output()
            .ok()?;
        parse_channel_volumes(&String::from_utf8_lossy(&out.stdout))
    }

    /// Master volume of the sink: the loudest channel, in percent.
    fn current_volume_percent(&self) -> Option<f64> {
        self.channel_volumes().map(|(l, r)| f64::from(l.max(r)))
    }
}

/// Per-channel gain factors (left, right) for a balance in [-1, 1].
pub fn balance_gains(balance: f64) -> (f64, f64) {
    let balance = balance.clamp(-1.0, 1.0);
    (
        if balance > 0.0 { 1.0 - balance } else { 1.0 },
        if balance < 0.0 { 1.0 + balance } else { 1.0 },
    )
}

/// Extrai as porcentagens (L, R) da saída de `pactl get-sink-volume`.
pub fn parse_channel_volumes(text: &str) -> Option<(u32, u32)> {
    let mut values = text
        .split_whitespace()
        .filter_map(|t| t.strip_suffix('%')?.parse::<u32>().ok());
    let left = values.next()?;
    Some((left, values.next().unwrap_or(left)))
}
