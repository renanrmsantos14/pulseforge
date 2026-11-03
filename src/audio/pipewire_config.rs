use crate::config::model::Profile;

/// Produces a user-scoped PipeWire filter-chain configuration. The daemon can
/// load this text with `pipewire -c <file>` without requiring LADSPA plugins.
pub fn filter_chain(profile: &Profile) -> String {
    filter_chain_for_output(profile, None)
}

/// Builds a filter sink routed to a concrete PipeWire output node.
pub fn filter_chain_for_output(profile: &Profile, output: Option<&str>) -> String {
    // Cada canal (L/R) tem a sua própria cadeia, com inputs/outputs explícitos:
    // assim o PipeWire nunca mistura nem duplica canais por conta própria.
    let gain = 10f32.powf(profile.preamp_db / 20.0);
    let mut nodes: Vec<String> = Vec::new();
    let mut links: Vec<String> = Vec::new();
    let mut inputs: Vec<String> = Vec::new();
    let mut outputs: Vec<String> = Vec::new();
    for ch in ["L", "R"] {
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
        links.extend(chain.windows(2).map(|pair| format!(
            "{{ output = \"{}:Out\" input = \"{}:In\" }}", pair[0], pair[1]
        )));
        inputs.push(format!("\"{}:In\"", chain[0]));
        outputs.push(format!("\"{}:Out\"", chain[chain.len() - 1]));
    }
    let target = output
        .map(|v| format!(" node.target = \"{}\"", v.replace('"', "")))
        .unwrap_or_default();
    format!("context.modules = [ {{ name = libpipewire-module-filter-chain args = {{ node.name = \"pulseforge\" node.description = \"PulseForge — {}\" media.name = \"PulseForge\" filter.graph = {{ nodes = [ {} ] links = [ {} ] inputs = [ {} ] outputs = [ {} ] }} audio.channels = 2 audio.position = [ FL FR ] capture.props = {{ node.name = \"pulseforge\" node.description = \"PulseForge — {}\" media.class = Audio/Sink }} playback.props = {{ node.name = \"pulseforge.output\" node.passive = true{} }} }} }} ]\n", profile.name, nodes.join(" "), links.join(" "), inputs.join(" "), outputs.join(" "), profile.name, target)
}
