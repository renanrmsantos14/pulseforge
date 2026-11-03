use crate::audio::backend::AudioBackend;
use adw::prelude::MessageDialogExt;
use anyhow::Context;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

pub fn run(start_minimized: bool) {
    adw::init().expect("libadwaita initialization failed");
    let app = adw::Application::builder()
        .application_id("com.pulseforge.PulseForge")
        .build();
    app.connect_activate(move |app| {
        if let Some(window) = app.windows().first() {
            window.present();
        } else {
            build_window(app, start_minimized);
        }
    });
    // `--gui` is consumed by our CLI before GTK starts; do not pass it to
    // GApplication's option parser.
    app.run_with_args(&["pulseforge"]);
}

fn build_window(app: &adw::Application, start_minimized: bool) {
    crate::tray::start();
    let config = Rc::new(RefCell::new(
        crate::config::store::load().unwrap_or_default(),
    ));
    let outputs = crate::audio::system::PactlBackend::default()
        .list_outputs()
        .unwrap_or_default();
    let outputs: Vec<_> = outputs
        .into_iter()
        .filter(|sink| !crate::audio::system::PactlBackend::is_pulseforge(sink))
        .collect();
    let header = adw::HeaderBar::builder()
        .title_widget(&gtk::Label::new(Some("PulseForge / Equalizer")))
        .build();
    let status = gtk::Label::new(Some(&format!(
        "●  PipeWire connected  ·  PulseForge v{}",
        env!("CARGO_PKG_VERSION")
    )));
    status.add_css_class("dim-label");
    header.pack_end(&status);

    let profile_label = gtk::Label::new(Some("Perfil"));
    profile_label.set_width_request(90);
    let profile_names: Vec<String> = config
        .borrow()
        .profiles
        .iter()
        .map(|item| item.name.clone())
        .collect();
    let profile_name_refs: Vec<&str> = profile_names.iter().map(String::as_str).collect();
    let profile_model = gtk::StringList::new(&profile_name_refs);
    let profile = gtk::DropDown::new(Some(profile_model.clone()), None::<&gtk::Expression>);
    let default_output = crate::audio::system::PactlBackend::default()
        .default_output()
        .ok()
        .flatten();
    let selected_profile_id = default_output.as_ref().and_then(|sink| {
        let config = config.borrow();
        crate::output_rules::match_profile(
            sink,
            &config.output_rules,
            &config.profiles,
            &config.fallback_profile_id,
        )
        .map(|item| item.id.clone())
    });
    let selected_index = config
        .borrow()
        .profiles
        .iter()
        .position(|item| Some(&item.id) == selected_profile_id.as_ref())
        .unwrap_or(0) as u32;
    profile.set_selected(selected_index);
    let output_label = gtk::Label::new(Some("Saída de áudio"));
    output_label.set_width_request(90);
    let output_names: Vec<&str> = outputs.iter().map(|sink| sink.name.as_str()).collect();
    let output = gtk::DropDown::from_strings(&output_names);
    if let Some(default_output) = default_output.as_ref() {
        if let Some(index) = outputs
            .iter()
            .position(|sink| sink.name == default_output.name)
        {
            output.set_selected(index as u32);
        }
    }
    let profile_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    profile_row.append(&profile_label);
    profile_row.append(&profile);
    profile.set_hexpand(true);
    let output_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    output_row.append(&output_label);
    output_row.append(&output);
    output.set_hexpand(true);
    if let Some(current_profile) = config.borrow().profiles.get(selected_index as usize) {
        let output_name = default_output.as_ref().map(|sink| sink.name.as_str());
        if let Ok(mut service) = crate::audio::service::FilterService::new() {
            let _ = service.start_for_output(current_profile, output_name);
        }
    }

    let selected_gains: Vec<f32> = config
        .borrow()
        .profiles
        .get(selected_index as usize)
        .map(|item| item.bands.iter().map(|band| band.gain_db).collect())
        .unwrap_or_else(|| vec![0.0; 12]);
    let frequencies = [
        "31", "62", "125", "250", "500", "1k", "2k", "4k", "6k", "8k", "12k", "16k",
    ];
    let eq = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let sliders = Rc::new(RefCell::new(Vec::new()));
    for (freq, gain) in frequencies.iter().zip(selected_gains.iter().copied()) {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
        column.set_valign(gtk::Align::Center);
        let value = gtk::Label::new(Some(&format_db(gain as f64)));
        value.add_css_class("accent");
        let slider = gtk::Scale::with_range(gtk::Orientation::Vertical, -12., 12., 1.);
        slider.set_value(gain as f64);
        slider.set_inverted(true);
        slider.set_vexpand(true);
        slider.set_size_request(28, 230);
        let value_label = value.clone();
        slider.connect_value_changed(move |slider| {
            value_label.set_text(&format_db(slider.value()));
        });
        sliders.borrow_mut().push(slider.clone());
        let label = gtk::Label::new(Some(&format!("{freq} Hz")));
        label.add_css_class("dim-label");
        column.append(&value);
        column.append(&slider);
        column.append(&label);
        eq.append(&column);
    }

    let selected_preamp = config.borrow().profiles.get(selected_index as usize)
        .map(|item| item.preamp_db).unwrap_or(0.0);
    let master_label = gtk::Label::new(Some("Volume geral"));
    let master_value = gtk::Label::new(Some(&format_db(selected_preamp as f64)));
    let master_gain = gtk::Scale::with_range(gtk::Orientation::Horizontal, -24.0, 6.0, 0.1);
    master_gain.set_value(selected_preamp as f64);
    master_gain.set_hexpand(true);
    master_gain.set_tooltip_text(Some("Ganho geral antes do equalizador"));
    let master_row = gtk::Box::new(gtk::Orientation::Vertical, 8);
    master_row.set_width_request(92);
    master_row.set_valign(gtk::Align::Fill);
    master_row.append(&master_label);
    master_gain.set_orientation(gtk::Orientation::Vertical);
    master_gain.set_inverted(true);
    master_gain.set_vexpand(true);
    master_gain.set_size_request(42, 260);
    master_row.append(&master_gain);
    master_row.append(&master_value);
    let limiter = gtk::CheckButton::with_label("Limiter");
    limiter.set_active(config.borrow().profiles.get(selected_index as usize).map(|p| p.limiter).unwrap_or(true));
    limiter.set_tooltip_text(Some("Protege contra clipping no final da cadeia"));
    let clip_indicator = gtk::Label::new(Some("● Headroom seguro"));
    clip_indicator.add_css_class("accent");

    let output_targets: Vec<String> = outputs.iter().map(|sink| sink.name.clone()).collect();
    let apply_generation = Rc::new(Cell::new(0u64));
    for slider in sliders.borrow().iter() {
        let config = Rc::clone(&config);
        let profile = profile.clone();
        let output = output.clone();
        let master_gain = master_gain.clone();
        let limiter = limiter.clone();
        let output_targets = output_targets.clone();
        let sliders = Rc::clone(&sliders);
        let apply_generation = Rc::clone(&apply_generation);
        let master_gain = master_gain.clone();
        let limiter = limiter.clone();
        let clip_indicator = clip_indicator.clone();
        slider.connect_value_changed(move |_| {
            update_clip_indicator(&clip_indicator, &limiter, master_gain.value(), &sliders);
            schedule_live(
                &config,
                &profile,
                &output,
                &output_targets,
                &sliders,
                &apply_generation,
                &master_gain,
                &limiter,
            );
        });
    }

    {
        let config = Rc::clone(&config);
        let sliders = Rc::clone(&sliders);
        let output = output.clone();
        let output_targets = output_targets.clone();
        let apply_generation = Rc::clone(&apply_generation);
        let status = status.clone();
        let master_gain = master_gain.clone();
        let limiter = limiter.clone();
        let clip_indicator = clip_indicator.clone();
        profile.connect_selected_notify(move |row| {
            let selected = row.selected() as usize;
            {
                let config = config.borrow();
                if let Some(selected_profile) = config.profiles.get(selected) {
                    for (band, slider) in selected_profile.bands.iter().zip(sliders.borrow().iter())
                    {
                        slider.set_value(band.gain_db as f64);
                    }
                    if let Some(selected_profile) = config.profiles.get(selected) {
                        master_gain.set_value(selected_profile.preamp_db as f64);
                        limiter.set_active(selected_profile.limiter);
                    }
                    update_clip_indicator(&clip_indicator, &limiter, master_gain.value(), &sliders);
                }
            }
            schedule_live(
                &config,
                row,
                &output,
                &output_targets,
                &sliders,
                &apply_generation,
                &master_gain,
                &limiter,
            );
            status.set_text(&format!(
                "●  PipeWire connected  ·  {}",
                row.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .map(|item| item.string().to_string())
                    .unwrap_or_else(|| "Perfil selecionado".into())
            ));
        });
    }

    {
        let config = Rc::clone(&config);
        let profile = profile.clone();
        let output = output.clone();
        let output_targets = output_targets.clone();
        let apply_generation = Rc::clone(&apply_generation);
        let master_value = master_value.clone();
        let clip_indicator = clip_indicator.clone();
        let limiter = limiter.clone();
        let sliders = Rc::clone(&sliders);
        master_gain.connect_value_changed(move |slider| {
            master_value.set_text(&format_db(slider.value()));
            update_clip_indicator(&clip_indicator, &limiter, slider.value(), &sliders);
            schedule_live(&config, &profile, &output, &output_targets, &sliders, &apply_generation, slider, &limiter);
        });
    }
    {
        let config = Rc::clone(&config);
        let profile = profile.clone();
        let output = output.clone();
        let output_targets = output_targets.clone();
        let apply_generation = Rc::clone(&apply_generation);
        let clip_indicator = clip_indicator.clone();
        let master_gain = master_gain.clone();
        let sliders = Rc::clone(&sliders);
        limiter.connect_toggled(move |button| {
            update_clip_indicator(&clip_indicator, button, master_gain.value(), &sliders);
            schedule_live(&config, &profile, &output, &output_targets, &sliders, &apply_generation, &master_gain, button);
        });
    }

    {
        let config = Rc::clone(&config);
        let profile = profile.clone();
        let sliders = Rc::clone(&sliders);
        let output_targets = output_targets.clone();
        let apply_generation = Rc::clone(&apply_generation);
        let status = status.clone();
        let master_gain = master_gain.clone();
        let limiter = limiter.clone();
        let clip_indicator = clip_indicator.clone();
        output.connect_selected_notify(move |row| {
            schedule_live(
                &config,
                &profile,
                row,
                &output_targets,
                &sliders,
                &apply_generation,
                &master_gain,
                &limiter,
            );
            update_clip_indicator(&clip_indicator, &limiter, master_gain.value(), &sliders);
            status.set_text(&format!(
                "●  PipeWire connected  ·  saída {}",
                row.selected_item()
                    .and_downcast::<gtk::StringObject>()
                    .map(|item| item.string().to_string())
                    .unwrap_or_else(|| "selecionada".into())
            ));
        });
    }

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let balance_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let left_label = gtk::Label::new(Some("L 100%"));
    let right_label = gtk::Label::new(Some("R 100%"));
    let balance = gtk::Scale::with_range(gtk::Orientation::Horizontal, -1.0, 1.0, 0.01);
    balance.set_value(0.0);
    balance.set_width_request(180);
    balance.set_tooltip_text(Some("Balanço estéreo: L / R"));
    balance_box.append(&left_label);
    balance_box.append(&balance);
    balance_box.append(&right_label);
    let left_label_live = left_label.clone();
    let right_label_live = right_label.clone();
    balance.connect_value_changed(move |slider| {
        let (left, right) = crate::audio::service::balance_gains(slider.value());
        let left_percent = (left * 100.0).round() as i32;
        let right_percent = (right * 100.0).round() as i32;
        left_label_live.set_text(&format!("L {left_percent}%"));
        right_label_live.set_text(&format!("R {right_percent}%"));
        if let Ok(service) = crate::audio::service::FilterService::new() {
            let _ = service.set_balance(slider.value());
        }
    });
    controls.append(&balance_box);
    controls.append(&limiter);
    controls.append(&clip_indicator);
    let bypass = gtk::ToggleButton::with_label("Bypass");
    {
        let config = Rc::clone(&config);
        let profile = profile.clone();
        bypass.connect_toggled(move |button| {
            let mut service = match crate::audio::service::FilterService::new() {
                Ok(service) => service,
                Err(_) => return,
            };
            if button.is_active() {
                service.stop();
                button.set_label("Bypass active");
            } else {
                let selected = profile.selected() as usize;
                if let Some(current) = config.borrow().profiles.get(selected) {
                    let _ = service.start(current);
                }
                button.set_label("Bypass");
            }
        });
    }
    let new_profile = gtk::Button::with_label("Novo");
    let duplicate_profile = gtk::Button::with_label("Duplicar");
    let rename_profile = gtk::Button::with_label("Renomear");
    let delete_profile = gtk::Button::with_label("Excluir");
    let save = gtk::Button::with_label("Salvar perfil");
    save.add_css_class("suggested-action");

    let profile_actions = [
        new_profile.clone(), duplicate_profile.clone(), rename_profile.clone(),
        delete_profile.clone(), save.clone(),
    ];
    {
        let config = Rc::clone(&config);
        let profile = profile.clone();
        let profile_model = profile_model.clone();
        new_profile.connect_clicked(move |button| {
            let window = button.root().and_then(|r| r.downcast::<adw::ApplicationWindow>().ok());
            if let Some(window) = window {
                let config = Rc::clone(&config); let profile = profile.clone(); let profile_model = profile_model.clone();
                profile_name_dialog(&window, "Novo profile", "Meu profile", "Criar", move |name| {
                    let mut cfg = config.borrow_mut();
                    let id = unique_profile_id(&cfg, &name);
                    if crate::profiles::create(&mut cfg, &id, &name).and_then(|_| crate::config::store::save(&cfg)).is_ok() {
                        refresh_profile_model(&profile_model, &cfg);
                        if let Some(index) = cfg.profiles.iter().position(|p| p.id == id) { profile.set_selected(index as u32); }
                    }
                });
            }
        });
    }
    {
        let config = Rc::clone(&config);
        let profile = profile.clone();
        let profile_model = profile_model.clone();
        duplicate_profile.connect_clicked(move |button| {
            let window = button.root().and_then(|r| r.downcast::<adw::ApplicationWindow>().ok());
            let Some(window) = window else { return };
            let source = config.borrow().profiles.get(profile.selected() as usize).cloned();
            let Some(source) = source else { return };
            let config = Rc::clone(&config); let profile = profile.clone(); let profile_model = profile_model.clone();
            profile_name_dialog(&window, "Duplicar profile", &format!("{} cópia", source.name), "Duplicar", move |name| {
                let mut cfg = config.borrow_mut(); let id = unique_profile_id(&cfg, &name);
                if crate::profiles::duplicate(&mut cfg, &source.id, &id, &name).and_then(|_| crate::config::store::save(&cfg)).is_ok() {
                    refresh_profile_model(&profile_model, &cfg);
                    if let Some(index) = cfg.profiles.iter().position(|p| p.id == id) { profile.set_selected(index as u32); }
                }
            });
        });
    }
    {
        let config = Rc::clone(&config); let profile = profile.clone(); let profile_model = profile_model.clone();
        rename_profile.connect_clicked(move |button| {
            let Some(window) = button.root().and_then(|r| r.downcast::<adw::ApplicationWindow>().ok()) else { return };
            let Some(current) = config.borrow().profiles.get(profile.selected() as usize).cloned() else { return };
            let config = Rc::clone(&config); let profile_model = profile_model.clone();
            profile_name_dialog(&window, "Renomear profile", &current.name, "Salvar", move |name| {
                let mut cfg = config.borrow_mut();
                if crate::profiles::rename(&mut cfg, &current.id, &name).and_then(|_| crate::config::store::save(&cfg)).is_ok() { refresh_profile_model(&profile_model, &cfg); }
            });
        });
    }
    {
        let config = Rc::clone(&config); let profile = profile.clone(); let profile_model = profile_model.clone();
        delete_profile.connect_clicked(move |button| {
            let Some(window) = button.root().and_then(|r| r.downcast::<adw::ApplicationWindow>().ok()) else { return };
            let Some(current) = config.borrow().profiles.get(profile.selected() as usize).cloned() else { return };
            let dialog = adw::MessageDialog::builder().transient_for(&window).modal(true).heading("Excluir profile?").body(&format!("Excluir {} permanentemente?", current.name)).build();
            dialog.add_response("cancel", "Cancelar"); dialog.add_response("delete", "Excluir"); dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            let config = Rc::clone(&config); let profile = profile.clone(); let profile_model = profile_model.clone();
            dialog.connect_response(Some("delete"), move |dialog, _| { let mut cfg = config.borrow_mut(); if crate::profiles::delete(&mut cfg, &current.id).and_then(|_| crate::config::store::save(&cfg)).is_ok() { refresh_profile_model(&profile_model, &cfg); profile.set_selected(0); } dialog.close(); });
            dialog.present();
        });
    }
    {
        let config = Rc::clone(&config);
        let sliders = Rc::clone(&sliders);
        let profile = profile.clone();
        let output = output.clone();
        let output_names: Vec<String> = outputs.iter().map(|sink| sink.name.clone()).collect();
        save.connect_clicked(move |button| {
            let mut config = config.borrow_mut();
            let selected = profile.selected() as usize;
            if let Some(target) = config.profiles.get_mut(selected) {
                for (band, slider) in target.bands.iter_mut().zip(sliders.borrow().iter()) {
                    band.gain_db = slider.value() as f32;
                }
                target.preamp_db = master_gain.value() as f32;
                target.limiter = limiter.is_active();
                let output_name = output_names
                    .get(output.selected() as usize)
                    .map(String::as_str);
                match crate::audio::service::FilterService::new()
                    .and_then(|mut service| service.start_for_output(target, output_name))
                    .and_then(|_| crate::config::store::save(&config))
                {
                    Ok(()) => button.set_label("Saved ✓"),
                    Err(_) => button.set_label("Save failed"),
                }
            }
        });
    }
    for button in profile_actions { controls.append(&button); }
    controls.append(&bypass);
    let update = gtk::Button::with_label("Check for updates");
    update.connect_clicked(|button| {
        button.set_label("Checking…");
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = crate::updater::update().map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        let button = button.clone();
        gtk::glib::timeout_add_local(Duration::from_millis(100), move || {
            match receiver.try_recv() {
                Ok(Ok(())) => {
                    button.set_label("Updated — restart");
                    offer_restart(&button);
                    gtk::glib::ControlFlow::Break
                }
                Ok(Err(error)) => {
                    let short_error = error.chars().take(90).collect::<String>();
                    button.set_label(&format!("No update: {short_error}"));
                    gtk::glib::ControlFlow::Break
                }
                Err(mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => {
                    button.set_label("Update unavailable");
                    gtk::glib::ControlFlow::Break
                }
            }
        });
    });
    controls.append(&update);
    let page = gtk::Box::new(gtk::Orientation::Vertical, 18);
    page.set_margin_top(24);
    page.set_margin_bottom(24);
    page.set_margin_start(28);
    page.set_margin_end(28);
    page.prepend(&header);
    page.append(&controls);
    page.append(&profile_row);
    page.append(&output_row);
    let eq_with_master = gtk::Box::new(gtk::Orientation::Horizontal, 18);
    eq_with_master.append(&eq);
    eq_with_master.append(&master_row);
    page.append(&eq_with_master);
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("PulseForge")
        .default_width(1050)
        .default_height(660)
        .content(&page)
        .build();
    window.set_icon_name(Some("pulseforge"));
    window.connect_close_request(|window| {
        // Keep the equalizer and its PipeWire graph alive when the user
        // closes the window. Activating PulseForge again presents it.
        window.hide();
        gtk::glib::Propagation::Stop
    });
    if start_minimized {
        window.hide();
    } else {
        window.present();
    }
}

fn offer_restart(button: &gtk::Button) {
    let Some(window) = button
        .root()
        .and_then(|root| root.downcast::<adw::ApplicationWindow>().ok())
    else {
        return;
    };
    let dialog = adw::MessageDialog::builder()
        .transient_for(&window)
        .modal(true)
        .heading("Atualização pronta")
        .body("Uma nova versão foi instalada localmente. Reiniciar agora?")
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

fn refresh_profile_model(model: &gtk::StringList, config: &crate::AppConfig) {
    let names: Vec<String> = config.profiles.iter().map(|p| p.name.clone()).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    model.splice(0, model.n_items(), &refs);
}

fn format_db(value: f64) -> String {
    format!("{value:+.1} dB")
}

fn update_clip_indicator(indicator: &gtk::Label, limiter: &gtk::CheckButton, preamp: f64, sliders: &Rc<RefCell<Vec<gtk::Scale>>>) {
    let peak_gain = sliders.borrow().iter().map(|slider| slider.value()).fold(0.0_f64, f64::max);
    if preamp + peak_gain > 0.0 {
        indicator.set_text(if limiter.is_active() { "● Limiter protegendo" } else { "● CLIPPING" });
        indicator.remove_css_class("accent");
        indicator.add_css_class(if limiter.is_active() { "warning" } else { "error" });
    } else {
        indicator.set_text("● Headroom seguro");
        indicator.remove_css_class("warning");
        indicator.remove_css_class("error");
        indicator.add_css_class("accent");
    }
}

fn unique_profile_id(config: &crate::AppConfig, name: &str) -> String {
    let base: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() { "profile".to_string() } else { base };
    if !config.profiles.iter().any(|p| p.id == base) { return base; }
    for n in 2..1000 { let id = format!("{base}-{n}"); if !config.profiles.iter().any(|p| p.id == id) { return id; } }
    format!("{base}-new")
}

fn profile_name_dialog(
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

fn apply_live(
    config: &RefCell<crate::AppConfig>,
    profile: &gtk::DropDown,
    output: &gtk::DropDown,
    output_targets: &[String],
    sliders: &Rc<RefCell<Vec<gtk::Scale>>>,
    master_gain: &gtk::Scale,
    limiter: &gtk::CheckButton,
) -> anyhow::Result<()> {
    let mut config = config.borrow_mut();
    let selected = profile.selected() as usize;
    let current = config
        .profiles
        .get_mut(selected)
        .context("perfil selecionado inexistente")?;
    for (band, slider) in current.bands.iter_mut().zip(sliders.borrow().iter()) {
        band.gain_db = slider.value() as f32;
    }
    current.preamp_db = master_gain.value() as f32;
    current.limiter = limiter.is_active();
    let output_name = output_targets
        .get(output.selected() as usize)
        .map(String::as_str);
    let mut service = crate::audio::service::FilterService::new()?;
    service.start_for_output(current, output_name)
}

fn schedule_live(
    config: &Rc<RefCell<crate::AppConfig>>,
    profile: &gtk::DropDown,
    output: &gtk::DropDown,
    output_targets: &[String],
    sliders: &Rc<RefCell<Vec<gtk::Scale>>>,
    generation: &Rc<Cell<u64>>,
    master_gain: &gtk::Scale,
    limiter: &gtk::CheckButton,
) {
    let next = generation.get().wrapping_add(1);
    generation.set(next);
    let config = Rc::clone(config);
    let profile = profile.clone();
    let output = output.clone();
    let output_targets = output_targets.to_vec();
    let sliders = Rc::clone(sliders);
    let generation = Rc::clone(generation);
    let master_gain = master_gain.clone();
    let limiter = limiter.clone();
    gtk::glib::timeout_add_local_once(Duration::from_millis(250), move || {
        if generation.get() == next {
            let _ = apply_live(&config, &profile, &output, &output_targets, &sliders, &master_gain, &limiter);
        }
    });
}
