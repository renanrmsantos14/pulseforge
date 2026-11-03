use crate::audio::backend::{AudioBackend, AudioError};
use crate::config::model::{Profile, SinkInfo};
use directories::BaseDirs;
use std::{fs, process::Command};

/// Lightweight compatibility backend. It uses the PipeWire PulseAudio bridge,
/// so no native system headers are required for discovery and diagnostics.
#[derive(Default)]
pub struct PactlBackend {
    current: Option<SinkInfo>,
}

impl PactlBackend {
    pub fn is_pulseforge(sink: &SinkInfo) -> bool {
        sink.name.eq_ignore_ascii_case("pulseforge")
            || sink.name.to_ascii_lowercase().contains("pulseforge")
    }

    fn configured_target() -> Option<String> {
        let path = BaseDirs::new()?
            .config_dir()
            .join("pipewire/filter-chain.conf.d/pulseforge.conf");
        let text = fs::read_to_string(path).ok()?;
        text.split("node.target = \"")
            .nth(1)?
            .split('\"')
            .next()
            .map(str::to_owned)
    }
    fn run(args: &[&str]) -> Result<String, AudioError> {
        let out = Command::new("pactl")
            .args(args)
            .output()
            .map_err(|e| AudioError::Unavailable(e.to_string()))?;
        if !out.status.success() {
            return Err(AudioError::Unavailable(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
    fn parse_sinks(text: &str) -> Vec<SinkInfo> {
        text.lines()
            .filter_map(|line| {
                let cols: Vec<_> = line.split_whitespace().collect();
                if cols.len() < 2 || cols[0].parse::<u32>().is_err() {
                    return None;
                }
                Some(SinkInfo {
                    id: cols[0].to_string(),
                    name: cols[1].to_string(),
                    kind: "sink".into(),
                })
            })
            .collect()
    }

    pub fn parse_for_test(text: &str) -> Vec<SinkInfo> {
        Self::parse_sinks(text)
    }
}

impl AudioBackend for PactlBackend {
    fn list_outputs(&self) -> Result<Vec<SinkInfo>, AudioError> {
        Self::run(&["list", "short", "sinks"]).map(|s| Self::parse_sinks(&s))
    }
    fn default_output(&self) -> Result<Option<SinkInfo>, AudioError> {
        let name = Self::run(&["get-default-sink"])?.trim().to_string();
        let outputs = self.list_outputs()?;
        let saved_target = Self::configured_target();
        Ok(outputs
            .iter()
            .find(|sink| sink.name == name && !Self::is_pulseforge(sink))
            .cloned()
            .or_else(|| {
                saved_target
                    .and_then(|target| outputs.iter().find(|sink| sink.name == target).cloned())
            })
            .or_else(|| {
                outputs
                    .iter()
                    .find(|sink| {
                        !Self::is_pulseforge(sink)
                            && sink.name.contains("usb")
                            && sink.name.contains("analog")
                    })
                    .cloned()
            })
            .or_else(|| outputs.into_iter().find(|sink| !Self::is_pulseforge(sink))))
    }
    fn attach_filter(&mut self, output: &SinkInfo) -> Result<(), AudioError> {
        self.current = Some(output.clone());
        Ok(())
    }
    fn detach_filter(&mut self) -> Result<(), AudioError> {
        self.current = None;
        Ok(())
    }
    fn apply_profile(&mut self, _profile: &Profile) -> Result<(), AudioError> {
        self.current
            .as_ref()
            .map(|_| ())
            .ok_or_else(|| AudioError::Unavailable("no output attached".into()))
    }
}
