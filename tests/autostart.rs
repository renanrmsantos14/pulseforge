use pulseforge::autostart;

#[test]
fn autostart_path_uses_config_directory() {
    let path = autostart::desktop_path().unwrap();
    assert!(path.ends_with("autostart/pulseforge.desktop"));
}
