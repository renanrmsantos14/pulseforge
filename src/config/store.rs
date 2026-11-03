use crate::config::model::AppConfig;
use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::{fs, path::PathBuf};

pub fn config_path() -> Result<PathBuf> {
    ProjectDirs::from("com", "PulseForge", "PulseForge")
        .map(|d| d.config_dir().join("config.json"))
        .context("XDG config directory unavailable")
}

pub fn load() -> Result<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let mut config: AppConfig = serde_json::from_str(&fs::read_to_string(path)?)?;
    // Add new built-in profiles and bands without overwriting the user's
    // existing local presets.
    let defaults = AppConfig::default();
    for profile in &mut config.profiles {
        if let Some(default_profile) = defaults.profiles.iter().find(|item| item.id == profile.id) {
            for band in &default_profile.bands {
                if !profile
                    .bands
                    .iter()
                    .any(|item| item.frequency_hz == band.frequency_hz)
                {
                    profile.bands.push(band.clone());
                }
            }
            profile
                .bands
                .sort_by(|left, right| left.frequency_hz.total_cmp(&right.frequency_hz));
        }
    }
    for default_profile in defaults.profiles {
        if !config
            .profiles
            .iter()
            .any(|item| item.id == default_profile.id)
        {
            config.profiles.push(default_profile);
        }
    }
    Ok(config)
}

pub fn save(config: &AppConfig) -> Result<()> {
    let path = config_path()?;
    fs::create_dir_all(path.parent().context("config path has no parent")?)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(config)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}
