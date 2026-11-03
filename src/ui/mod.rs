//! Interface GTK4/libadwaita do PulseForge (estilo "mesa de som").
//!
//! Edição ao vivo: os controles alteram um *rascunho* do perfil e o áudio é
//! atualizado em seguida; o perfil salvo só muda em "Salvar". O balanço L/R é
//! global, aplicado ao vivo e persistido sozinho.

mod curve;
mod dialogs;
mod style;

use crate::audio::{backend::AudioBackend, response, service::FilterService, service::balance_gains};
use crate::config::model::{AppConfig, Profile};
use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

const BUILD_TIME: &str = env!("PULSEFORGE_BUILD_TIME");
const APPLY_DEBOUNCE_MS: u64 = 200;
const SAVE_BALANCE_DEBOUNCE_MS: u64 = 500;
const EPS: f32 = 0.05;

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
    // `--gui` é consumido pela nossa CLI; não repassar ao GApplication.
    app.run_with_args(&["pulseforge"]);
}

struct Ui {
    window: adw::ApplicationWindow,
    config: RefCell<AppConfig>,
    draft: RefCell<Profile>,
    current: Cell<usize>,
    loading: Cell<bool>,
    apply_gen: Cell<u64>,
    balance_gen: Cell<u64>,
    outputs: Vec<String>,

    profile: gtk::DropDown,
    profile_model: gtk::StringList,
    output: gtk::DropDown,
    bypass: gtk::ToggleButton,
    menu_buttons: Vec<gtk::Button>,

    preamp: gtk::Scale,
    preamp_value: gtk::Label,
    bands: Vec<gtk::Scale>,
    band_values: Vec<gtk::Label>,
    limiter: gtk::Switch,
    headroom: gtk::Label,

    balance: gtk::Scale,
    balance_l: gtk::Label,
    balance_r: gtk::Label,

    dirty_label: gtk::Label,
    undo: gtk::Button,
    save: gtk::Button,
    status: gtk::Label,

    curve_data: Rc<RefCell<curve::CurveData>>,
    curve_area: gtk::DrawingArea,
}

fn db_text(v: f64) -> String {
    format!("{v:+.1}")
}

fn freq_text(hz: f32) -> String {
    if hz >= 1000.0 {
        let k = hz / 1000.0;
        if (k - k.round()).abs() < 0.05 {
            format!("{}k", k.round() as i32)
        } else {
            format!("{k:.1}k")
        }
    } else {
        format!("{}", hz.round() as i32)
    }
}

fn same_profile(a: &Profile, b: &Profile) -> bool {
    a.limiter == b.limiter
        && (a.preamp_db - b.preamp_db).abs() < EPS
        && a.bands.len() == b.bands.len()
        && a.bands
            .iter()
            .zip(&b.bands)
            .all(|(x, y)| (x.gain_db - y.gain_db).abs() < EPS)
}

fn set_tone(label: &gtk::Label, value: f64, preamp: bool) {
    label.remove_css_class("zero");
    label.remove_css_class("preamp");
    if preamp {
        label.add_css_class("preamp");
    } else if value.abs() < 0.05 {
        label.add_css_class("zero");
    }
}

fn card() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
    b.add_css_class("card-pf");
    b
}

fn fader(min: f64, max: f64, css: Option<&str>) -> gtk::Scale {
    let s = gtk::Scale::with_range(gtk::Orientation::Vertical, min, max, 0.1);
    s.set_inverted(true);
    s.set_vexpand(true);
    s.set_draw_value(false);
    s.set_round_digits(1);
    s.set_size_request(34, 200);
    s.add_css_class("fader");
    if let Some(c) = css {
        s.add_css_class(c);
    }
    s.add_mark(0.0, gtk::PositionType::Right, None);
    s
}

impl Ui {
    fn output_name(&self) -> Option<String> {
        self.outputs.get(self.output.selected() as usize).cloned()
    }

    fn dirty(&self) -> bool {
        let cfg = self.config.borrow();
        cfg.profiles
            .get(self.current.get())
            .map(|saved| !same_profile(saved, &self.draft.borrow()))
            .unwrap_or(false)
    }

    fn set_status(&self, ok: bool, text: &str) {
        self.status.set_text(text);
        self.status.remove_css_class("status-ok");
        self.status.remove_css_class("status-bad");
        self.status
            .add_css_class(if ok { "status-ok" } else { "status-bad" });
    }

    /// Atualiza rótulos, curva, headroom e estado dos botões a partir do rascunho.
    fn refresh_view(&self) {
        let draft = self.draft.borrow();
        self.preamp_value.set_text(&db_text(f64::from(draft.preamp_db)));
        set_tone(&self.preamp_value, 0.0, true);
        for (band, label) in draft.bands.iter().zip(&self.band_values) {
            label.set_text(&db_text(f64::from(band.gain_db)));
            set_tone(label, f64::from(band.gain_db), false);
        }

        let peak = draft
            .bands
            .iter()
            .map(|b| f64::from(b.gain_db))
            .fold(0.0_f64, f64::max)
            + f64::from(draft.preamp_db);
        for c in ["status-ok", "status-warn", "status-bad"] {
            self.headroom.remove_css_class(c);
        }
        if peak > 0.0 {
            if draft.limiter {
                self.headroom.set_text("● Limiter protegendo");
                self.headroom.add_css_class("status-warn");
            } else {
                self.headroom.set_text("● Risco de clipping");
                self.headroom.add_css_class("status-bad");
            }
        } else {
            self.headroom.set_text("● Headroom seguro");
            self.headroom.add_css_class("status-ok");
        }

        {
            let mut data = self.curve_data.borrow_mut();
            data.points = response::curve(&draft, 160, curve::MIN_HZ, curve::MAX_HZ);
            data.markers = draft
                .bands
                .iter()
                .map(|b| {
                    (
                        f64::from(b.frequency_hz),
                        response::profile_db(&draft, f64::from(b.frequency_hz)),
                    )
                })
                .collect();
        }
        drop(draft);
        self.curve_area.queue_draw();

        let dirty = self.dirty();
        self.dirty_label.set_text(if dirty {
            "● Alterações não salvas"
        } else {
            "Sem alterações"
        });
        if dirty {
            self.dirty_label.add_css_class("dirty");
        } else {
            self.dirty_label.remove_css_class("dirty");
        }
        self.undo.set_sensitive(dirty);
        self.save.set_sensitive(dirty);
        for b in &self.menu_buttons {
            b.set_sensitive(!dirty);
            b.set_tooltip_text(if dirty {
                Some("Salve ou desfaça as alterações antes")
            } else {
                None
            });
        }
    }

    /// Copia o rascunho para os widgets sem disparar edições.
    fn push_draft_to_widgets(&self) {
        self.loading.set(true);
        {
            let d = self.draft.borrow();
            self.preamp.set_value(f64::from(d.preamp_db));
            for (band, scale) in d.bands.iter().zip(&self.bands) {
                scale.set_value(f64::from(band.gain_db));
            }
            self.limiter.set_active(d.limiter);
        }
        self.loading.set(false);
    }

    fn load_profile(&self, index: usize) {
        let Some(profile) = self.config.borrow().profiles.get(index).cloned() else {
            return;
        };
        self.current.set(index);
        *self.draft.borrow_mut() = profile;
        self.push_draft_to_widgets();
        self.refresh_view();
        self.apply_now();
    }

    fn select_dropdown(&self, index: usize) {
        self.loading.set(true);
        self.profile.set_selected(index as u32);
        self.loading.set(false);
    }

    fn reload_model(&self, select: usize) {
        self.loading.set(true);
        dialogs::refresh_profile_model(&self.profile_model, &self.config.borrow());
        self.profile.set_selected(select as u32);
        self.loading.set(false);
    }

    fn apply_now(&self) {
        self.apply_gen.set(self.apply_gen.get().wrapping_add(1));
        if self.bypass.is_active() {
            return;
        }
        let draft = self.draft.borrow().clone();
        let balance = f64::from(self.config.borrow().balance);
        let output = self.output_name();
        match FilterService::new().and_then(|mut s| s.apply(&draft, output.as_deref(), balance)) {
            Ok(()) => self.set_status(true, "● Aplicado ao PipeWire"),
            Err(e) => self.set_status(false, &format!("● Falha ao aplicar: {e}")),
        }
    }

    fn schedule_apply(self: &Rc<Self>) {
        let next = self.apply_gen.get().wrapping_add(1);
        self.apply_gen.set(next);
        let ui = Rc::clone(self);
        gtk::glib::timeout_add_local_once(Duration::from_millis(APPLY_DEBOUNCE_MS), move || {
            if ui.apply_gen.get() == next {
                ui.apply_now();
            }
        });
    }

    fn edited(self: &Rc<Self>) {
        self.refresh_view();
        self.schedule_apply();
    }

    fn undo_changes(self: &Rc<Self>) {
        self.load_profile(self.current.get());
    }

    fn save_profile(self: &Rc<Self>) {
        let idx = self.current.get();
        let draft = self.draft.borrow().clone();
        let result = {
            let mut cfg = self.config.borrow_mut();
            match cfg.profiles.get_mut(idx) {
                Some(slot) => {
                    *slot = draft;
                    crate::config::store::save(&cfg)
                }
                None => Ok(()),
            }
        };
        match result {
            Ok(()) => {
                self.refresh_view();
                self.set_status(true, "● Perfil salvo");
            }
            Err(e) => self.set_status(false, &format!("● Falha ao salvar: {e}")),
        }
    }

    fn balance_changed(self: &Rc<Self>) {
        let v = self.balance.value();
        let (l, r) = balance_gains(v);
        self.balance_l.set_text(&format!("L {}%", (l * 100.0).round() as i32));
        self.balance_r.set_text(&format!("R {}%", (r * 100.0).round() as i32));
        if self.loading.get() {
            return;
        }
        self.config.borrow_mut().balance = v as f32;
        if !self.bypass.is_active() {
            if let Err(e) = FilterService::new().and_then(|s| s.set_balance(v)) {
                self.set_status(false, &format!("● Balanço não aplicado: {e}"));
            }
        }
        let next = self.balance_gen.get().wrapping_add(1);
        self.balance_gen.set(next);
        let ui = Rc::clone(self);
        gtk::glib::timeout_add_local_once(
            Duration::from_millis(SAVE_BALANCE_DEBOUNCE_MS),
            move || {
                if ui.balance_gen.get() == next {
                    let _ = crate::config::store::save(&ui.config.borrow());
                }
            },
        );
    }

    fn profile_selected(self: &Rc<Self>) {
        let sel = self.profile.selected() as usize;
        if self.loading.get() || sel == self.current.get() {
            return;
        }
        if !self.dirty() {
            self.load_profile(sel);
            return;
        }
        // Volta a seleção e pergunta antes de descartar.
        self.select_dropdown(self.current.get());
        let name = self
            .config
            .borrow()
            .profiles
            .get(self.current.get())
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let dialog = adw::MessageDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .heading("Descartar alterações?")
            .body(format!("O perfil \"{name}\" tem alterações não salvas."))
            .build();
        dialog.add_response("keep", "Continuar editando");
        dialog.add_response("discard", "Descartar");
        dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("keep"));
        dialog.set_close_response("keep");
        let ui = Rc::clone(self);
        dialog.connect_response(Some("discard"), move |_, _| {
            ui.select_dropdown(sel);
            ui.load_profile(sel);
        });
        dialog.present();
    }

    fn toggle_bypass(self: &Rc<Self>) {
        let Ok(mut service) = FilterService::new() else {
            return;
        };
        if self.bypass.is_active() {
            self.apply_gen.set(self.apply_gen.get().wrapping_add(1));
            service.stop();
            self.set_status(false, "● Bypass ativo (sem equalização)");
        } else {
            self.apply_now();
        }
    }

    fn check_update(&self, button: &gtk::Button) {
        button.set_sensitive(false);
        button.set_label("Verificando…");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::updater::update().map_err(|e| e.to_string()));
        });
        let button = button.clone();
        let window = self.window.clone();
        gtk::glib::timeout_add_local(Duration::from_millis(100), move || match rx.try_recv() {
            Ok(Ok(())) => {
                button.set_sensitive(true);
                button.set_label("Atualizado");
                dialogs::offer_restart(&window);
                gtk::glib::ControlFlow::Break
            }
            Ok(Err(error)) => {
                button.set_sensitive(true);
                button.set_label("Atualizar");
                button.set_tooltip_text(Some(&error));
                let short: String = error.chars().take(80).collect();
                let toast = adw::MessageDialog::builder()
                    .transient_for(&window)
                    .modal(true)
                    .heading("Atualização")
                    .body(short)
                    .build();
                toast.add_response("ok", "OK");
                toast.present();
                gtk::glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                button.set_sensitive(true);
                button.set_label("Atualizar");
                gtk::glib::ControlFlow::Break
            }
        });
    }
}

fn build_window(app: &adw::Application, start_minimized: bool) {
    crate::tray::start();
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
    style::install();

    let config = crate::config::store::load().unwrap_or_default();
    let backend = crate::audio::system::PactlBackend::default();
    let outputs: Vec<_> = backend
        .list_outputs()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| !crate::audio::system::PactlBackend::is_pulseforge(s))
        .collect();
    let default_output = backend.default_output().ok().flatten();

    let initial_idx = default_output
        .as_ref()
        .and_then(|sink| {
            crate::output_rules::match_profile(
                sink,
                &config.output_rules,
                &config.profiles,
                &config.fallback_profile_id,
            )
        })
        .and_then(|p| config.profiles.iter().position(|x| x.id == p.id))
        .unwrap_or(0);
    let initial = config
        .profiles
        .get(initial_idx)
        .cloned()
        .unwrap_or_else(|| AppConfig::default().profiles.remove(0));

    // ---- Cabeçalho ----
    let names: Vec<&str> = config.profiles.iter().map(|p| p.name.as_str()).collect();
    let profile_model = gtk::StringList::new(&names);
    let profile = gtk::DropDown::new(Some(profile_model.clone()), None::<&gtk::Expression>);
    profile.set_selected(initial_idx as u32);
    profile.set_tooltip_text(Some("Perfil de equalização"));

    let output_names: Vec<&str> = outputs.iter().map(|s| s.name.as_str()).collect();
    let output = gtk::DropDown::from_strings(&output_names);
    output.set_size_request(260, -1);
    output.set_tooltip_text(Some("Saída de áudio"));
    if let Some(i) = default_output
        .as_ref()
        .and_then(|d| outputs.iter().position(|s| s.name == d.name))
    {
        output.set_selected(i as u32);
    }

    let bypass = gtk::ToggleButton::with_label("Bypass");
    bypass.set_tooltip_text(Some("Desliga o equalizador (áudio sem processamento)"));
    let update = gtk::Button::with_label("Atualizar");
    update.set_tooltip_text(Some("Procurar nova versão no GitHub"));

    let btn_new = gtk::Button::with_label("Novo perfil");
    let btn_dup = gtk::Button::with_label("Duplicar");
    let btn_ren = gtk::Button::with_label("Renomear");
    let btn_del = gtk::Button::with_label("Excluir");
    let menu_buttons = vec![btn_new.clone(), btn_dup.clone(), btn_ren.clone(), btn_del.clone()];
    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    for b in &menu_buttons {
        b.add_css_class("flat");
        menu_box.append(b);
    }
    let popover = gtk::Popover::new();
    popover.set_child(Some(&menu_box));
    let menu = gtk::MenuButton::new();
    menu.set_icon_name("view-more-symbolic");
    menu.set_popover(Some(&popover));
    menu.set_tooltip_text(Some("Gerenciar perfis"));

    let header = adw::HeaderBar::new();
    header.pack_start(&profile);
    header.pack_start(&menu);
    header.set_title_widget(Some(&output));
    header.pack_end(&bypass);
    header.pack_end(&update);

    // ---- Curva ----
    let curve_data = Rc::new(RefCell::new(curve::CurveData::default()));
    let curve_area = curve::area(Rc::clone(&curve_data));
    let curve_card = card();
    let t = gtk::Label::new(Some("RESPOSTA EM FREQUÊNCIA"));
    t.add_css_class("section");
    t.set_halign(gtk::Align::Start);
    curve_card.append(&t);
    curve_card.append(&curve_area);

    // ---- Faders ----
    let faders_card = card();
    let t = gtk::Label::new(Some("EQUALIZADOR"));
    t.add_css_class("section");
    t.set_halign(gtk::Align::Start);
    faders_card.append(&t);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_homogeneous(false);

    let column = |value: &gtk::Label, scale: &gtk::Scale, label: &gtk::Label| {
        let c = gtk::Box::new(gtk::Orientation::Vertical, 6);
        c.set_hexpand(true);
        c.set_halign(gtk::Align::Fill);
        value.set_halign(gtk::Align::Center);
        label.set_halign(gtk::Align::Center);
        scale.set_halign(gtk::Align::Center);
        c.append(value);
        c.append(scale);
        c.append(label);
        c
    };

    let preamp = fader(-24.0, 6.0, Some("preamp"));
    let preamp_value = gtk::Label::new(None);
    preamp_value.add_css_class("db");
    let preamp_label = gtk::Label::new(Some("PRÉ-AMP"));
    preamp_label.add_css_class("freq");
    row.append(&column(&preamp_value, &preamp, &preamp_label));
    row.append(&gtk::Separator::new(gtk::Orientation::Vertical));

    let mut bands = Vec::new();
    let mut band_values = Vec::new();
    for band in &initial.bands {
        let scale = fader(-12.0, 12.0, None);
        let value = gtk::Label::new(None);
        value.add_css_class("db");
        let label = gtk::Label::new(Some(&freq_text(band.frequency_hz)));
        label.add_css_class("freq");
        label.set_tooltip_text(Some(&format!(
            "{} Hz — clique para zerar esta banda",
            band.frequency_hz.round() as i32
        )));
        let reset = gtk::GestureClick::new();
        {
            let scale = scale.clone();
            reset.connect_released(move |_, _, _, _| scale.set_value(0.0));
        }
        label.add_controller(reset);
        row.append(&column(&value, &scale, &label));
        bands.push(scale);
        band_values.push(value);
    }
    faders_card.append(&row);

    // ---- Rodapé ----
    let footer = card();
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 18);

    let balance = gtk::Scale::with_range(gtk::Orientation::Horizontal, -1.0, 1.0, 0.01);
    balance.set_draw_value(false);
    balance.set_round_digits(2);
    balance.set_width_request(240);
    balance.add_css_class("fader");
    balance.add_css_class("balance");
    balance.add_mark(0.0, gtk::PositionType::Bottom, None);
    balance.set_tooltip_text(Some("Balanço estéreo (esquerda / direita)"));
    let balance_l = gtk::Label::new(None);
    let balance_r = gtk::Label::new(None);
    balance_l.add_css_class("freq");
    balance_r.add_css_class("freq");
    let center = gtk::Button::with_label("Centro");
    center.add_css_class("flat");
    let bal_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bal_box.append(&balance_l);
    bal_box.append(&balance);
    bal_box.append(&balance_r);
    bal_box.append(&center);
    controls.append(&bal_box);

    let limiter = gtk::Switch::new();
    limiter.set_valign(gtk::Align::Center);
    limiter.set_tooltip_text(Some("Protege contra clipping no fim da cadeia"));
    let lim_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    lim_box.append(&gtk::Label::new(Some("Limiter")));
    lim_box.append(&limiter);
    controls.append(&lim_box);

    let headroom = gtk::Label::new(None);
    controls.append(&headroom);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    controls.append(&spacer);

    let dirty_label = gtk::Label::new(None);
    let undo = gtk::Button::with_label("Desfazer");
    let save = gtk::Button::with_label("Salvar perfil");
    save.add_css_class("suggested-action");
    controls.append(&dirty_label);
    controls.append(&undo);
    controls.append(&save);
    footer.append(&controls);

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.set_hexpand(true);
    status.add_css_class("version");
    let version = gtk::Label::new(Some(&format!(
        "v{} · {}",
        env!("CARGO_PKG_VERSION"),
        BUILD_TIME
    )));
    version.add_css_class("version");
    version.set_halign(gtk::Align::End);
    let bottom = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bottom.append(&status);
    bottom.append(&version);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_top(12);
    body.set_margin_bottom(8);
    body.set_margin_start(14);
    body.set_margin_end(14);
    body.append(&curve_card);
    body.append(&faders_card);
    body.append(&footer);
    body.append(&bottom);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header);
    root.append(&body);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("PulseForge")
        .default_width(1060)
        .default_height(720)
        .content(&root)
        .build();
    window.add_css_class("pf");
    window.set_icon_name(Some("pulseforge"));
    window.connect_close_request(|window| {
        // Mantém o equalizador ativo: fechar apenas esconde a janela.
        window.hide();
        gtk::glib::Propagation::Stop
    });

    let balance_value = f64::from(config.balance);
    let ui = Rc::new(Ui {
        window: window.clone(),
        config: RefCell::new(config),
        draft: RefCell::new(initial),
        current: Cell::new(initial_idx),
        loading: Cell::new(true),
        apply_gen: Cell::new(0),
        balance_gen: Cell::new(0),
        outputs: outputs.iter().map(|s| s.name.clone()).collect(),
        profile: profile.clone(),
        profile_model,
        output: output.clone(),
        bypass: bypass.clone(),
        menu_buttons,
        preamp: preamp.clone(),
        preamp_value,
        bands: bands.clone(),
        band_values,
        limiter: limiter.clone(),
        headroom,
        balance: balance.clone(),
        balance_l,
        balance_r,
        dirty_label,
        undo: undo.clone(),
        save: save.clone(),
        status,
        curve_data,
        curve_area,
    });

    // Estado inicial dos widgets (sem disparar edições).
    balance.set_value(balance_value);
    ui.balance_changed();
    ui.push_draft_to_widgets();
    ui.loading.set(false);
    ui.refresh_view();
    ui.apply_now();

    // ---- Ligações ----
    {
        let ui = Rc::clone(&ui);
        preamp.connect_value_changed(move |s| {
            if ui.loading.get() {
                return;
            }
            ui.draft.borrow_mut().preamp_db = s.value() as f32;
            ui.edited();
        });
    }
    for (i, scale) in bands.iter().enumerate() {
        let ui = Rc::clone(&ui);
        scale.connect_value_changed(move |s| {
            if ui.loading.get() {
                return;
            }
            if let Some(b) = ui.draft.borrow_mut().bands.get_mut(i) {
                b.gain_db = s.value() as f32;
            }
            ui.edited();
        });
    }
    {
        let ui = Rc::clone(&ui);
        limiter.connect_active_notify(move |s| {
            if ui.loading.get() {
                return;
            }
            ui.draft.borrow_mut().limiter = s.is_active();
            ui.edited();
        });
    }
    {
        let ui = Rc::clone(&ui);
        balance.connect_value_changed(move |_| ui.balance_changed());
    }
    {
        let balance = balance.clone();
        center.connect_clicked(move |_| balance.set_value(0.0));
    }
    {
        let ui = Rc::clone(&ui);
        profile.connect_selected_notify(move |_| ui.profile_selected());
    }
    {
        let ui = Rc::clone(&ui);
        output.connect_selected_notify(move |_| {
            if !ui.loading.get() {
                ui.schedule_apply();
            }
        });
    }
    {
        let ui = Rc::clone(&ui);
        bypass.connect_toggled(move |_| ui.toggle_bypass());
    }
    {
        let ui = Rc::clone(&ui);
        undo.connect_clicked(move |_| ui.undo_changes());
    }
    {
        let ui = Rc::clone(&ui);
        save.connect_clicked(move |_| ui.save_profile());
    }
    {
        let ui = Rc::clone(&ui);
        update.connect_clicked(move |b| ui.check_update(b));
    }

    // ---- Gerenciar perfis ----
    {
        let ui = Rc::clone(&ui);
        let popover = popover.clone();
        btn_new.connect_clicked(move |_| {
            popover.popdown();
            let ui2 = Rc::clone(&ui);
            dialogs::profile_name_dialog(&ui.window, "Novo perfil", "Meu perfil", "Criar", move |name| {
                let id = dialogs::unique_profile_id(&ui2.config.borrow(), &name);
                let created = {
                    let mut cfg = ui2.config.borrow_mut();
                    crate::profiles::create(&mut cfg, &id, &name)
                        .and_then(|_| crate::config::store::save(&cfg))
                };
                if created.is_ok() {
                    let idx = ui2.config.borrow().profiles.iter().position(|p| p.id == id);
                    if let Some(idx) = idx {
                        ui2.reload_model(idx);
                        ui2.load_profile(idx);
                    }
                }
            });
        });
    }
    {
        let ui = Rc::clone(&ui);
        let popover = popover.clone();
        btn_dup.connect_clicked(move |_| {
            popover.popdown();
            let Some(src) = ui.config.borrow().profiles.get(ui.current.get()).cloned() else {
                return;
            };
            let ui2 = Rc::clone(&ui);
            dialogs::profile_name_dialog(
                &ui.window,
                "Duplicar perfil",
                &format!("{} cópia", src.name),
                "Duplicar",
                move |name| {
                    let id = dialogs::unique_profile_id(&ui2.config.borrow(), &name);
                    let done = {
                        let mut cfg = ui2.config.borrow_mut();
                        crate::profiles::duplicate(&mut cfg, &src.id, &id, &name)
                            .and_then(|_| crate::config::store::save(&cfg))
                    };
                    if done.is_ok() {
                        let idx = ui2.config.borrow().profiles.iter().position(|p| p.id == id);
                        if let Some(idx) = idx {
                            ui2.reload_model(idx);
                            ui2.load_profile(idx);
                        }
                    }
                },
            );
        });
    }
    {
        let ui = Rc::clone(&ui);
        let popover = popover.clone();
        btn_ren.connect_clicked(move |_| {
            popover.popdown();
            let Some(cur) = ui.config.borrow().profiles.get(ui.current.get()).cloned() else {
                return;
            };
            let ui2 = Rc::clone(&ui);
            dialogs::profile_name_dialog(&ui.window, "Renomear perfil", &cur.name, "Salvar", move |name| {
                let done = {
                    let mut cfg = ui2.config.borrow_mut();
                    crate::profiles::rename(&mut cfg, &cur.id, &name)
                        .and_then(|_| crate::config::store::save(&cfg))
                };
                if done.is_ok() {
                    ui2.reload_model(ui2.current.get());
                }
            });
        });
    }
    {
        let ui = Rc::clone(&ui);
        let popover = popover.clone();
        btn_del.connect_clicked(move |_| {
            popover.popdown();
            let Some(cur) = ui.config.borrow().profiles.get(ui.current.get()).cloned() else {
                return;
            };
            let dialog = adw::MessageDialog::builder()
                .transient_for(&ui.window)
                .modal(true)
                .heading("Excluir perfil?")
                .body(format!("Excluir \"{}\" permanentemente?", cur.name))
                .build();
            dialog.add_response("cancel", "Cancelar");
            dialog.add_response("delete", "Excluir");
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            dialog.set_close_response("cancel");
            let ui2 = Rc::clone(&ui);
            dialog.connect_response(Some("delete"), move |_, _| {
                let done = {
                    let mut cfg = ui2.config.borrow_mut();
                    crate::profiles::delete(&mut cfg, &cur.id)
                        .and_then(|_| crate::config::store::save(&cfg))
                };
                if done.is_ok() {
                    ui2.reload_model(0);
                    ui2.load_profile(0);
                }
            });
            dialog.present();
        });
    }

    if start_minimized {
        window.hide();
    } else {
        window.present();
    }
}
