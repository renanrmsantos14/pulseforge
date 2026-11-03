use pulseforge::audio::pipewire_config::{filter_chain, filter_chain_for_output};
use pulseforge::AppConfig;

#[test]
fn config_contains_all_bands_and_links() {
    let profile = &AppConfig::default().profiles[0];
    let config = filter_chain(profile);
    assert_eq!(config.matches("label = bq_peaking").count(), 24);
    assert!(config.contains("\"eq_band_0_L:Out\" input = \"eq_band_1_L:In\""));
    assert!(config.contains("\"eq_band_0_R:Out\" input = \"eq_band_1_R:In\""));
    assert!(config.contains("inputs = [ \"master_gain_L:In\" \"master_gain_R:In\" ]"));
    assert!(config.contains("outputs = [ \"output_limiter_L:Out\" \"output_limiter_R:Out\" ]"));
    assert!(config.contains("node.name = \"pulseforge\""));
}

#[test]
fn output_target_is_quoted_and_injected() {
    let profile = &AppConfig::default().profiles[0];
    let config = filter_chain_for_output(profile, Some("alsa_output.usb"));
    assert!(config.contains("node.target = \"alsa_output.usb\""));
}
