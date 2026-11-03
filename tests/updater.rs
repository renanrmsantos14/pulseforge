use pulseforge::updater::{is_newer, parse_release, ASSET_NAME};

#[test]
fn parses_latest_release_json() {
    let json = format!(
        r#"{{"tag_name":"v0.2.0","assets":[{{"name":"outro.txt","browser_download_url":"https://x/outro"}},{{"name":"{ASSET_NAME}","browser_download_url":"https://x/bin"}}]}}"#
    );
    let release = parse_release(&json).unwrap();
    assert_eq!(release.version, "0.2.0");
    assert_eq!(release.url, "https://x/bin");
}

#[test]
fn release_without_binary_is_an_error() {
    assert!(parse_release(r#"{"tag_name":"v1.0.0","assets":[]}"#).is_err());
    assert!(parse_release("não é json").is_err());
}

#[test]
fn compares_versions_numerically() {
    assert!(is_newer("0.1.10", "0.1.9"));
    assert!(is_newer("v0.2.0", "0.1.99"));
    assert!(!is_newer("0.1.9", "0.1.9"));
    assert!(!is_newer("0.1.8", "0.1.9"));
}
