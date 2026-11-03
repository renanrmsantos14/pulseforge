use pulseforge::audio::backend::AudioBackend;
use pulseforge::audio::service::FilterService;
use pulseforge::audio::system::PactlBackend;
use pulseforge::autostart;
use pulseforge::AppConfig;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {
        println!("PulseForge {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|arg| arg == "--update") {
        match pulseforge::updater::update() {
            Ok(()) => println!("updated; restart PulseForge"),
            Err(error) => eprintln!("update failed: {error}"),
        }
        return;
    }
    if args
        .iter()
        .any(|arg| arg == "--autostart" || arg == "--no-autostart")
    {
        let enabled = args.iter().any(|arg| arg == "--autostart");
        autostart::set_enabled(enabled).expect("unable to configure autostart");
        println!(
            "autostart: {}",
            if enabled { "enabled" } else { "disabled" }
        );
        return;
    }
    if args.iter().any(|arg| arg == "--gui") {
        #[cfg(feature = "gui")]
        {
            pulseforge::ui::run(args.iter().any(|arg| arg == "--start-minimized"));
            return;
        }
        #[cfg(not(feature = "gui"))]
        {
            eprintln!("GUI unavailable: rebuild with --features gui");
            return;
        }
    }
    let config = pulseforge::config::store::load().unwrap_or_else(|_| AppConfig::default());
    let active = config
        .profiles
        .iter()
        .find(|p| p.id == config.fallback_profile_id)
        .unwrap_or(&config.profiles[0]);
    if args.iter().any(|arg| arg == "--start-filter") {
        let backend = PactlBackend::default();
        let output = backend
            .default_output()
            .expect("unable to query default output");
        let target = output.as_ref().map(|sink| sink.name.as_str());
        let mut service = FilterService::new().expect("XDG directories unavailable");
        service
            .apply(active, target, f64::from(config.balance))
            .expect("unable to start filter-chain");
        println!(
            "filter running for {}; press Ctrl+C to stop",
            target.unwrap_or("default output")
        );
        loop {
            std::thread::park();
        }
    }
    if args.iter().any(|arg| arg == "--filter-config") {
        let service = FilterService::new().expect("XDG cache unavailable");
        std::fs::write(
            service.config_path(),
            pulseforge::audio::pipewire_config::filter_chain_with_balance(active, None, f64::from(config.balance)),
        )
        .expect("unable to write filter config");
        println!("filter config: {}", service.config_path().display());
        return;
    }
    let backend = PactlBackend::default();
    match backend.list_outputs() {
        Ok(outputs) => println!(
            "PulseForge {} — {} perfil, {} bandas, {} saídas",
            env!("CARGO_PKG_VERSION"),
            config.profiles.len(),
            config.profiles[0].bands.len(),
            outputs.len()
        ),
        Err(error) => println!(
            "PulseForge {} — áudio indisponível: {error}",
            env!("CARGO_PKG_VERSION")
        ),
    }
}
