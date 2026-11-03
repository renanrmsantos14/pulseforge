use pulseforge::audio::system::PactlBackend;

#[test]
fn parses_pactl_short_sink_rows() {
    let rows = "42 alsa_output.pci-0000_00_1f.3.analog-stereo module-alsa-card.c  s32le 2ch 48000Hz IDLE\n";
    let outputs = PactlBackend::parse_for_test(rows);
    assert_eq!(outputs[0].id, "42");
    assert_eq!(outputs[0].kind, "sink");
}
