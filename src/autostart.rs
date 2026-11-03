use anyhow::{Context, Result};
use directories::BaseDirs;
use std::{fs, path::PathBuf};

pub fn desktop_path() -> Result<PathBuf> {
    Ok(BaseDirs::new()
        .context("home directory unavailable")?
        .config_dir()
        .join("autostart/pulseforge.desktop"))
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let path = desktop_path()?;
    if enabled {
        fs::create_dir_all(path.parent().context("autostart path has no parent")?)?;
        fs::write(path, "[Desktop Entry]\nType=Application\nName=PulseForge\nComment=Personal PipeWire equalizer\nExec=pulseforge --gui --start-minimized\nTerminal=false\nX-GNOME-Autostart-enabled=true\n")?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}
