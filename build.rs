use std::process::Command;

fn main() {
    // Data/hora do build (America/Sao_Paulo), exibida no canto da interface.
    let stamp = Command::new("date")
        .env("TZ", "America/Sao_Paulo")
        .arg("+%d/%m/%Y %H:%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "data indisponível".into());
    println!("cargo:rustc-env=PULSEFORGE_BUILD_TIME={stamp}");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
}
