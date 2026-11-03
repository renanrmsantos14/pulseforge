use ksni::blocking::TrayMethods;
use gtk4::gio::prelude::ApplicationExt;

#[derive(Debug, Default)]
struct PulseForgeTray;

impl ksni::Tray for PulseForgeTray {
    fn id(&self) -> String {
        "pulseforge".into()
    }

    fn title(&self) -> String {
        "PulseForge — Equalizador ativo".into()
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        // Small cyan equalizer mark: readable on both light and dark Mint panels.
        let size = 24usize;
        let mut data = vec![0u8; size * size * 4];
        let heights = [8usize, 14, 20, 12, 17];
        for (bar, height) in heights.into_iter().enumerate() {
            let x_start = 3 + bar * 4;
            for x in x_start..x_start + 2 {
                for y in (24 - height)..21 {
                    let offset = (y * size + x) * 4;
                    data[offset..offset + 4].copy_from_slice(&[255, 84, 229, 231]);
                }
            }
        }
        vec![ksni::Icon { width: 24, height: 24, data }]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        activate_app();
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;

        vec![
            StandardItem {
                label: "Abrir PulseForge".into(),
                icon_name: "audio-volume-high-symbolic".into(),
                activate: Box::new(|_| activate_app()),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Sair".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| quit_app()),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn activate_app() {
    gtk4::glib::MainContext::default().invoke(|| {
        if let Some(application) = gtk4::gio::Application::default() {
            application.activate();
        }
    });
}

fn quit_app() {
    gtk4::glib::MainContext::default().invoke(|| {
        if let Some(application) = gtk4::gio::Application::default() {
            application.quit();
        }
    });
}

pub fn start() {
    std::thread::spawn(|| {
        if let Err(error) = PulseForgeTray.spawn() {
            eprintln!("PulseForge tray unavailable: {error}");
        }
    });
}
