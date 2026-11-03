use pulseforge::{profiles, AppConfig};

#[test]
fn profile_crud_preserves_fallback() {
    let mut config = AppConfig::default();
    profiles::duplicate(&mut config, "flat", "desk", "Desk").unwrap();
    profiles::rename(&mut config, "desk", "Desk Reference").unwrap();
    assert!(profiles::delete(&mut config, "flat").is_err());
    assert_eq!(profiles::delete(&mut config, "desk").unwrap().id, "desk");
}
