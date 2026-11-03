use pulseforge::output_rules::match_profile;
use pulseforge::{AppConfig, EqualizerChain, SinkInfo};

#[test]
fn default_profile_is_flat_and_twelve_band() {
    let c = AppConfig::default();
    assert_eq!(c.profiles.len(), 6);
    assert_eq!(c.profiles[0].bands.len(), 12);
    assert!(c.profiles[0].bands.iter().all(|b| b.gain_db == 0.));
}

#[test]
fn flow_xa_10_matches_reference_curve() {
    let profile = AppConfig::default()
        .profiles
        .into_iter()
        .find(|item| item.id == "flow-xa-10")
        .expect("Flow Xa 10 profile");
    let gains: Vec<f32> = profile.bands.iter().map(|band| band.gain_db).collect();
    assert_eq!(
        gains,
        vec![3.1, 3.0, 2.8, 2.1, -0.0, -0.2, -0.1, -0.1, -0.5, 0.2, 0.9, 1.0]
    );
}
#[test]
fn bypass_preserves_audio() {
    let c = AppConfig::default();
    let mut e = EqualizerChain::from_profile(&c.profiles[0], 48000.);
    e.bypass = true;
    let mut l = [0.2, -0.4];
    let mut r = [0.1, 0.3];
    e.process_in_place(&mut l, &mut r);
    assert_eq!(l, [0.2, -0.4]);
    assert_eq!(r, [0.1, 0.3]);
}
#[test]
fn fallback_matches_sink() {
    let c = AppConfig::default();
    let sink = SinkInfo {
        id: "x".into(),
        name: "USB DAC".into(),
        kind: "usb".into(),
    };
    assert_eq!(
        match_profile(&sink, &c.output_rules, &c.profiles, "flat")
            .unwrap()
            .id,
        "flat"
    );
}
