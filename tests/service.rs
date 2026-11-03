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
