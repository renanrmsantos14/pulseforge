use crate::config::model::{AppConfig, Profile};
use anyhow::{bail, Result};

pub fn create(config: &mut AppConfig, id: &str, name: &str) -> Result<()> {
    if config.profiles.iter().any(|p| p.id == id) {
        bail!("profile id already exists: {id}");
    }
    let mut profile = config
        .profiles
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("no base profile"))?;
    profile.id = id.into();
    profile.name = name.into();
    config.profiles.push(profile);
    Ok(())
}

pub fn duplicate(config: &mut AppConfig, source_id: &str, id: &str, name: &str) -> Result<()> {
    if config.profiles.iter().any(|p| p.id == id) {
        bail!("profile id already exists: {id}");
    }
    let mut profile = config
        .profiles
        .iter()
        .find(|p| p.id == source_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("source profile not found: {source_id}"))?;
    profile.id = id.into();
    profile.name = name.into();
    config.profiles.push(profile);
    Ok(())
}

pub fn rename(config: &mut AppConfig, id: &str, name: &str) -> Result<()> {
    config
        .profiles
        .iter_mut()
        .find(|p| p.id == id)
        .map(|p| p.name = name.into())
        .ok_or_else(|| anyhow::anyhow!("profile not found: {id}"))
}

pub fn delete(config: &mut AppConfig, id: &str) -> Result<Profile> {
    if id == config.fallback_profile_id {
        bail!("cannot delete fallback profile");
    }
    let index = config
        .profiles
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| anyhow::anyhow!("profile not found: {id}"))?;
    Ok(config.profiles.remove(index))
}
