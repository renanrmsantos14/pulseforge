use crate::config::model::Profile;

/// Gains (left, right) in [0, 1] for a stereo balance in [-1, 1]: the louder
/// side stays at unity gain and the opposite side is attenuated.
pub fn balance_gains(balance: f64) -> (f64, f64) {
    let balance = balance.clamp(-1.0, 1.0);
    (
        if balance > 0.0 { 1.0 - balance } else { 1.0 },
        if balance < 0.0 { 1.0 + balance } else { 1.0 },
    )
}

/// Structural fingerprint of a filter graph. While it is unchanged, every
/// value (gains, frequencies, Q, balance) can be updated live in PipeWire;
/// when it changes the filter-chain has to be restarted.
pub fn skeleton(profile: &Profile, output: Option<&str>) -> String {
    format!(
        "bands={};limiter={};target={}",
        profile.bands.len(),
        profile.limiter,
        output.unwrap_or("")
    )
}

/// First line of the generated file, read back to decide live update vs restart.
pub const SKELETON_PREFIX: &str = "# pulseforge-skeleton: ";

/// Live-tunable control values (`node:port`, value) of the whole graph.
pub fn live_params(profile: &Profile, balance: f64) -> Vec<(String, f32)> {
    let gain = 10f32.powf(profile.preamp_db / 20.0);
    let mut params = Vec::new();
    for ch in ["L", "R"] {
        params.push((format!("master_gain_{ch}:Mult"), gain));
        for (i, band) in profile.bands.iter().enumerate() {
            params.push((format!("eq_band_{i}_{ch}:Freq"), band.frequency_hz));
            params.push((format!("eq_band_{i}_{ch}:Q"), band.q.max(0.1)));
            params.push((format!("eq_band_{i}_{ch}:Gain"), band.gain_db));
        }
    }
    params.extend(balance_params(balance));
    params
}

/// Only the balance controls (a gain stage at the end of each channel).
pub fn balance_params(balance: f64) -> Vec<(String, f32)> {
    let (left, right) = balance_gains(balance);
    vec![
        ("balance_L:Mult".into(), left as f32),
        ("balance_R:Mult".into(), right as f32),
    ]
}

/// Produces a user-scoped PipeWire filter-chain configuration. The daemon can
/// load this text with `pipewire -c <file>` without requiring LADSPA plugins.
pub fn filter_chain(profile: &Profile) -> String {
    filter_chain_for_output(profile, None)
}

/// Builds a filter sink routed to a concrete PipeWire output node.
pub fn filter_chain_for_output(profile: &Profile, output: Option<&str>) -> String {
    filter_chain_with_balance(profile, output, 0.0)
}

pub fn filter_chain_with_balance(profile: &Profile, output: Option<&str>, balance: f64) -> String {
    // Each channel (L/R) has its own chain with explicit inputs/outputs, so
    // PipeWire never mixes or duplicates channels on its own:
    // master_gain -> bands -> [limiter] -> balance.
    let gain = 10f32.powf(profile.preamp_db / 20.0);
    let (balance_left, balance_right) = balance_gains(balance);
    let mut nodes: Vec<String> = Vec::new();
    let mut links: Vec<String> = Vec::new();
    let mut inputs: Vec<String> = Vec::new();
    let mut outputs: Vec<String> = Vec::new();
    for (ch, balance_gain) in [("L", balance_left), ("R", balance_right)] {
        let mut chain = vec![format!("master_gain_{ch}")];
        nodes.push(format!(
            "{{ type = builtin name = master_gain_{ch} label = linear control = {{ \"Mult\" = {gain:.6} \"Add\" = 0.0 }} }}"
        ));
        for (i, band) in profile.bands.iter().enumerate() {
            nodes.push(format!(
                "{{ type = builtin name = eq_band_{i}_{ch} label = bq_peaking control = {{ \"Freq\" = {:.2} \"Q\" = {:.3} \"Gain\" = {:.2} }} }}",
                band.frequency_hz, band.q.max(0.1), band.gain_db
            ));
            chain.push(format!("eq_band_{i}_{ch}"));
        }
        if profile.limiter {
            nodes.push(format!(
                "{{ type = builtin name = output_limiter_{ch} label = clamp control = {{ \"Min\" = -0.98 \"Max\" = 0.98 }} }}"
            ));
            chain.push(format!("output_limiter_{ch}"));
        }
        nodes.push(format!(
            "{{ type = builtin name = balance_{ch} label = linear control = {{ \"Mult\" = {balance_gain:.6} \"Add\" = 0.0 }} }}"
        ));
        chain.push(format!("balance_{ch}"));
        links.extend(chain.windows(2).map(|pair| format!(
            "{{ output = \"{}:Out\" input = \"{}:In\" }}", pair[0], pair[1]
        )));
        inputs.push(format!("\"{}:In\"", chain[0]));
        outputs.push(format!("\"{}:Out\"", chain[chain.len() - 1]));
    }
    let target = output
        .map(|v| format!(" node.target = \"{}\"", v.replace('"', "")))
        .unwrap_or_default();
    format!(
        "{SKELETON_PREFIX}{}\ncontext.modules = [ {{ name = libpipewire-module-filter-chain args = {{ node.name = \"pulseforge\" node.description = \"PulseForge — {}\" media.name = \"PulseForge\" filter.graph = {{ nodes = [ {} ] links = [ {} ] inputs = [ {} ] outputs = [ {} ] }} audio.channels = 2 audio.position = [ FL FR ] capture.props = {{ node.name = \"pulseforge\" node.description = \"PulseForge — {}\" media.class = Audio/Sink }} playback.props = {{ node.name = \"pulseforge.output\" node.passive = true{} }} }} }} ]\n",
        skeleton(profile, output), profile.name, nodes.join(" "), links.join(" "), inputs.join(" "), outputs.join(" "), profile.name, target
    )
}
