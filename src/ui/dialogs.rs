use adw::prelude::MessageDialogExt;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

pub fn offer_restart(window: &adw::ApplicationWindow) {
    let dialog = adw::MessageDialog::builder()
        .transient_for(window)
        .modal(true)
        .heading("Atualização pronta")
        .body("A nova versão foi baixada do GitHub e instalada. Reiniciar agora?")
        .build();
    dialog.add_response("later", "Depois");
    dialog.add_response("restart", "Reiniciar agora");
    dialog.set_response_appearance("restart", adw::ResponseAppearance::Suggested);
    dialog.connect_response(Some("restart"), move |dialog, _| {
        if let Ok(executable) = std::env::current_exe() {
            // The app uses a single GTK application ID. Schedule the new
            // process, then terminate this process explicitly; application
            // quit alone can be intercepted by the hide-to-tray close hook.
            if let Some(executable) = executable.to_str() {
                let _ = std::process::Command::new("sh")
                    .args([
                        "-c",
                        "sleep 1; exec \"$1\" --gui",
                        "pulseforge-restart",
                        executable,
                    ])
                    .spawn();
            }
        }
        if let Some(application) = dialog.application() {
            application.quit();
        }
        std::process::exit(0);
    });
    dialog.present();
}

pub fn unique_profile_id(config: &crate::AppConfig, name: &str) -> String {
    let base: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() { "profile".to_string() } else { base };
    if !config.profiles.iter().any(|p| p.id == base) { return base; }
    for n in 2..1000 { let id = format!("{base}-{n}"); if !config.profiles.iter().any(|p| p.id == id) { return id; } }
    format!("{base}-new")
}

pub fn profile_name_dialog(
    parent: &adw::ApplicationWindow,
    title: &str,
    initial: &str,
    accept: &str,
    on_accept: impl Fn(String) + 'static,
) {
    let dialog = gtk::Dialog::builder().transient_for(parent).modal(true).title(title).build();
    let entry = gtk::Entry::builder().text(initial).hexpand(true).build();
    entry.set_activates_default(true);
    dialog.content_area().append(&entry);
    dialog.add_button("Cancelar", gtk::ResponseType::Cancel);
    dialog.add_button(accept, gtk::ResponseType::Accept);
    dialog.set_default_response(gtk::ResponseType::Accept);
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            let name = entry.text().trim().to_string();
            if !name.is_empty() { on_accept(name); }
        }
        dialog.close();
    });
    dialog.present();
}

