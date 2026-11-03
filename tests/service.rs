use pulseforge::audio::service::FilterService;

#[test]
fn service_has_stable_lifecycle_before_start() {
    let mut service = FilterService::new().unwrap();
    let _ = service.is_running();
}

#[test]
fn balance_keeps_the_louder_channel_at_full_and_attenuates_the_other() {
    use pulseforge::audio::service::balance_gains;
    assert_eq!(balance_gains(0.0), (1.0, 1.0));
    assert_eq!(balance_gains(1.0), (0.0, 1.0));
    assert_eq!(balance_gains(-1.0), (1.0, 0.0));
    assert_eq!(balance_gains(0.5), (0.5, 1.0));
}

#[test]
fn parses_left_and_right_volume_from_pactl() {
    use pulseforge::audio::service::parse_channel_volumes;
    let text = "Volume: front-left: 53000 /  81% / -5,50 dB,   front-right: 26500 /  40% / -23,9 dB\n        balance -0,51";
    assert_eq!(parse_channel_volumes(text), Some((81, 40)));
    assert_eq!(parse_channel_volumes("sem volume"), None);
}
