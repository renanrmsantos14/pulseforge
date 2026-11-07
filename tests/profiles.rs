use pulseforge::{profiles, AppConfig};

#[test]
fn profile_crud_preserves_fallback() {
    let mut config = AppConfig::default();
    profiles::duplicate(&mut config, "flat", "desk", "Desk").unwrap();
    profiles::rename(&mut config, "desk", "Desk Reference").unwrap();
    assert!(profiles::delete(&mut config, "flat").is_err());
    assert_eq!(profiles::delete(&mut config, "desk").unwrap().id, "desk");
}

#[test]
fn remembered_profile_only_when_enabled_and_existing() {
    let mut config = pulseforge::config::model::AppConfig::default();
    config.last_profile_id = Some(config.profiles[2].id.clone());
    assert_eq!(config.remembered_profile(), None);
    config.remember_last_profile = true;
    assert_eq!(config.remembered_profile(), Some(2));
    config.last_profile_id = Some("apagado".into());
    assert_eq!(config.remembered_profile(), None);
}
