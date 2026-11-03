use pulseforge::audio::response::{curve, peaking_db, profile_db};
use pulseforge::AppConfig;

#[test]
fn peak_equals_band_gain_at_center_and_is_flat_far_away() {
    assert!((peaking_db(1000.0, 1000.0, 6.0, 1.4) - 6.0).abs() < 0.05);
    assert!(peaking_db(20.0, 4000.0, 6.0, 1.4).abs() < 0.2);
    assert!((peaking_db(1000.0, 1000.0, -9.0, 1.4) + 9.0).abs() < 0.05);
}

#[test]
fn flat_profile_is_zero_and_preamp_shifts_everything() {
    let mut profile = AppConfig::default().profiles[0].clone();
    assert!(profile_db(&profile, 500.0).abs() < 1e-6);
    profile.preamp_db = -6.0;
    assert!((profile_db(&profile, 500.0) + 6.0).abs() < 1e-6);
    let points = curve(&profile, 64, 20.0, 20_000.0);
    assert_eq!(points.len(), 64);
    assert!((points[0].0 - 20.0).abs() < 1e-9 && (points[63].0 - 20_000.0).abs() < 1e-6);
}
