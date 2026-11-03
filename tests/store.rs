use pulseforge::output_rules::match_profile;
use pulseforge::SinkInfo;
use pulseforge::{AppConfig, Band, Profile};

#[test]
fn exact_sink_rule_wins_over_name_rule() {
    let mut config = AppConfig::default();
    config.profiles.push(Profile {
        id: "headphones".into(),
        name: "Headphones".into(),
        preamp_db: 0.,
        bands: vec![Band {
            frequency_hz: 1000.,
            gain_db: 2.,
            q: 1.,
        }],
        limiter: true,
    });
    config.output_rules = vec![
        pulseforge::OutputRule {
            sink_id: None,
            name_contains: Some("usb".into()),
            kind: None,
            profile_id: "flat".into(),
            enabled: true,
        },
        pulseforge::OutputRule {
            sink_id: Some("sink-1".into()),
            name_contains: None,
            kind: None,
            profile_id: "headphones".into(),
            enabled: true,
        },
    ];
    let sink = SinkInfo {
        id: "sink-1".into(),
        name: "USB DAC".into(),
        kind: "usb".into(),
    };
    assert_eq!(
        match_profile(&sink, &config.output_rules, &config.profiles, "flat")
            .unwrap()
            .id,
        "headphones"
    );
}
