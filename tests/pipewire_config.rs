use pulseforge::audio::pipewire_config::{
    filter_chain, filter_chain_for_output, filter_chain_with_balance, live_params, skeleton,
};
use pulseforge::AppConfig;

#[test]
fn config_contains_all_bands_and_links() {
    let profile = &AppConfig::default().profiles[0];
    let config = filter_chain(profile);
    assert_eq!(config.matches("label = bq_peaking").count(), 24);
    assert!(config.contains("\"eq_band_0_L:Out\" input = \"eq_band_1_L:In\""));
    assert!(config.contains("\"eq_band_0_R:Out\" input = \"eq_band_1_R:In\""));
    assert!(config.contains("inputs = [ \"master_gain_L:In\" \"master_gain_R:In\" ]"));
    assert!(config.contains("outputs = [ \"balance_L:Out\" \"balance_R:Out\" ]"));
    assert!(config.contains("node.name = \"pulseforge\""));
}

#[test]
fn output_target_is_quoted_and_injected() {
    let profile = &AppConfig::default().profiles[0];
    let config = filter_chain_for_output(profile, Some("alsa_output.usb"));
    assert!(config.contains("node.target = \"alsa_output.usb\""));
}

#[test]
fn balance_is_a_gain_stage_per_channel() {
    let profile = &AppConfig::default().profiles[0];
    let config = filter_chain_with_balance(profile, None, 0.5);
    assert!(config.contains("name = balance_L label = linear control = { \"Mult\" = 0.500000"));
    assert!(config.contains("name = balance_R label = linear control = { \"Mult\" = 1.000000"));
}

#[test]
fn skeleton_header_and_live_params_cover_every_band() {
    let profile = &AppConfig::default().profiles[0];
    let config = filter_chain_for_output(profile, Some("alsa_x"));
    assert!(config.starts_with(&format!("# pulseforge-skeleton: {}\n", skeleton(profile, Some("alsa_x")))));
    // 2 canais x (1 ganho + 12 bandas x 3) + 2 de balanço
    assert_eq!(live_params(profile, 0.0).len(), 2 * (1 + 12 * 3) + 2);
    assert_ne!(skeleton(profile, None), skeleton(profile, Some("x")));
}
