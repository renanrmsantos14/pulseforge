//! Interface GTK4/libadwaita do PulseForge (estilo "estúdio": barra lateral
//! de perfis, curva, medidor L/R, faders e knobs).
//!
//! Edição ao vivo: os controles alteram um *rascunho* do perfil e o áudio é
//! atualizado em seguida; o perfil salvo só muda em "Salvar". O balanço L/R é
//! global, aplicado ao vivo e persistido sozinho.

mod curve;
mod dialogs;
mod knob;
mod meter;
mod style;

use crate::audio::{backend::AudioBackend, response, service::FilterService, service::balance_gains};
use crate::config::model::{AppConfig, Profile};
use adw::prelude::*;
use gtk4 as gtk;
use knob::Knob;
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

type Points = Rc<RefCell<Vec<(f64, f64)>>>;

struct Thumb {
    points: Points,
    area: gtk::DrawingArea,
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

    list: gtk::ListBox,
    thumbs: RefCell<Vec<Thumb>>,
    title: gtk::Label,
    output: gtk::DropDown,
    bypass: gtk::ToggleButton,
    menu_buttons: Vec<gtk::Button>,

    preamp: Rc<Knob>,
    preamp_text: gtk::Label,
    bands: Vec<gtk::Scale>,
    band_values: Vec<gtk::Label>,
    limiter: gtk::Switch,
    headroom: gtk::Label,

    balance_text: gtk::Label,
    meter: meter::Meter,

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

fn set_tone(label: &gtk::Label, value: f64) {
    if value.abs() < 0.05 {
        label.add_css_class("zero");
    } else {
        label.remove_css_class("zero");
    }
}

fn card() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 8);
    b.add_css_class("card-pf");
    b
}

fn section(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("section");
    l.set_halign(gtk::Align::Start);
    l
}

fn fader() -> gtk::Scale {
    let s = gtk::Scale::with_range(gtk::Orientation::Vertical, -12.0, 12.0, 0.1);
    s.set_inverted(true);
    s.set_vexpand(true);
    s.set_draw_value(false);
    s.set_round_digits(1);
    s.set_size_request(34, 150);
    s.add_css_class("fader");
    s.add_mark(0.0, gtk::PositionType::Right, None);
    s
}

fn thumb_points(profile: &Profile) -> Vec<(f64, f64)> {
    response::curve(profile, 48, curve::MIN_HZ, curve::MAX_HZ)
}

fn thumb_area(points: Points) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder().content_height(18).hexpand(true).build();
    area.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (f64::from(w), f64::from(h));
        let pts = points.borrow();
        if pts.len() < 2 {
            return;
        }
        let x = |f: f64| (f.ln() - curve::MIN_HZ.ln()) / (curve::MAX_HZ.ln() - curve::MIN_HZ.ln()) * w;
        let y = |db: f64| h / 2.0 - db.clamp(-12.0, 12.0) / 12.0 * (h / 2.0 - 1.0);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
        cr.set_line_width(1.0);
        cr.move_to(0.0, y(0.0));
        cr.line_to(w, y(0.0));
        let _ = cr.stroke();
        cr.set_source_rgba(0.24, 0.86, 0.72, 0.9);
        cr.set_line_width(1.5);
        cr.move_to(x(pts[0].0), y(pts[0].1));
        for (f, db) in &pts[1..] {
            cr.line_to(x(*f), y(*db));
        }
        let _ = cr.stroke();
    });
    area
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
        self.title.set_text(&draft.name);
        self.preamp_text
            .set_text(&format!("PRÉ-AMP · {} dB", db_text(f64::from(draft.preamp_db))));
        for (band, label) in draft.bands.iter().zip(&self.band_values) {
            label.set_text(&db_text(f64::from(band.gain_db)));
            set_tone(label, f64::from(band.gain_db));
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
        self.curve_data.borrow_mut().gains = draft.bands.iter().map(|b| f64::from(b.gain_db)).collect();
        if let Some(t) = self.thumbs.borrow().get(self.current.get()) {
            *t.points.borrow_mut() = thumb_points(&draft);
            t.area.queue_draw();
        }
        drop(draft);
        self.curve_area.queue_draw();

        let dirty = self.dirty();
        self.dirty_label.set_text(if dirty { "● não salvo" } else { "" });
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

    /// Ajusta uma banda vinda do arraste na curva (mantém fader e rascunho em sincronia).
    fn set_band(self: &Rc<Self>, band: usize, gain: f64) {
        let Some(scale) = self.bands.get(band) else {
            return;
        };
        {
            let mut d = self.draft.borrow_mut();
            let Some(b) = d.bands.get_mut(band) else {
                return;
            };
            if (f64::from(b.gain_db) - gain).abs() < 1e-6 {
                return;
            }
            b.gain_db = gain as f32;
        }
        self.loading.set(true);
        scale.set_value(gain);
        self.loading.set(false);
        self.edited();
    }

    fn update_balance_text(&self, v: f64) {
        let (l, r) = balance_gains(v);
        self.balance_text.set_text(&format!(
            "BALANÇO · L {}% · R {}%",
            (l * 100.0).round() as i32,
            (r * 100.0).round() as i32
        ));
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

    fn select_row(&self, index: usize) {
        self.loading.set(true);
        self.list.select_row(self.list.row_at_index(index as i32).as_ref());
        self.loading.set(false);
    }

    /// Reconstrói a lista de perfis (miniaturas incluídas) e seleciona `select`.
    fn reload_list(&self, select: usize) {
        self.loading.set(true);
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        let mut thumbs = Vec::new();
        for p in &self.config.borrow().profiles {
            let points: Points = Rc::new(RefCell::new(thumb_points(p)));
            let area = thumb_area(Rc::clone(&points));
            let name = gtk::Label::new(Some(&p.name));
            name.set_halign(gtk::Align::Start);
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
            content.append(&name);
            content.append(&area);
            self.list.append(&content);
            thumbs.push(Thumb { points, area });
        }
        *self.thumbs.borrow_mut() = thumbs;
        self.list.select_row(self.list.row_at_index(select as i32).as_ref());
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

    fn balance_changed(self: &Rc<Self>, v: f64) {
        self.update_balance_text(v);
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

    fn profile_selected(self: &Rc<Self>, sel: usize) {
        if self.loading.get() || sel == self.current.get() {
            return;
        }
        if !self.dirty() {
            self.load_profile(sel);
            return;
        }
        // Volta a seleção e pergunta antes de descartar.
        self.select_row(self.current.get());
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
            // Restaura a miniatura do perfil descartado antes de trocar.
            ui.select_row(sel);
            ui.load_profile(sel);
            ui.reload_list(sel);
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
                let info = adw::MessageDialog::builder()
                    .transient_for(&window)
                    .modal(true)
                    .heading("Atualização")
                    .body(short)
                    .build();
                info.add_response("ok", "OK");
                info.present();
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

fn knob_column(knob: &Rc<Knob>, text: &gtk::Label) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.add_css_class("freq");
    b.append(knob.widget());
    b.append(text);
    b
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

    // ---- Cabeçalho (janela) ----
    let bypass = gtk::ToggleButton::with_label("Bypass");
    bypass.set_tooltip_text(Some("Desliga o equalizador (áudio sem processamento)"));
    let update = gtk::Button::with_label("Atualizar");
    update.set_tooltip_text(Some("Procurar nova versão no GitHub"));
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(None)));
    header.pack_end(&bypass);
    header.pack_end(&update);

    // ---- Barra lateral ----
    let brand = gtk::Label::new(Some("PulseForge"));
    brand.add_css_class("title-pf");
    brand.set_halign(gtk::Align::Start);
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("profiles");
    let scroller = gtk::ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();

    let btn_new = gtk::Button::with_label("＋ Novo perfil");
    btn_new.set_hexpand(true);
    let btn_dup = gtk::Button::with_label("Duplicar");
    let btn_ren = gtk::Button::with_label("Renomear");
    let btn_del = gtk::Button::with_label("Excluir");
    let menu_buttons = vec![btn_new.clone(), btn_dup.clone(), btn_ren.clone(), btn_del.clone()];
    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    for b in [&btn_dup, &btn_ren, &btn_del] {
        b.add_css_class("flat");
        menu_box.append(b);
    }
    let popover = gtk::Popover::new();
    popover.set_child(Some(&menu_box));
    let menu = gtk::MenuButton::new();
    menu.set_icon_name("view-more-symbolic");
    menu.set_popover(Some(&popover));
    menu.set_tooltip_text(Some("Duplicar, renomear ou excluir o perfil"));
    let profile_actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    profile_actions.append(&btn_new);
    profile_actions.append(&menu);

    let output_names: Vec<&str> = outputs.iter().map(|s| s.name.as_str()).collect();
    let output = gtk::DropDown::from_strings(&output_names);
    output.set_tooltip_text(Some("Saída de áudio"));
    if let Some(i) = default_output
        .as_ref()
        .and_then(|d| outputs.iter().position(|s| s.name == d.name))
    {
        output.set_selected(i as u32);
    }
    let version = gtk::Label::new(Some(&format!(
        "v{} · {}",
        env!("CARGO_PKG_VERSION"),
        BUILD_TIME
    )));
    version.add_css_class("version");
    version.set_halign(gtk::Align::Start);

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sidebar.add_css_class("sidebar-pf");
    sidebar.set_width_request(236);
    sidebar.append(&brand);
    sidebar.append(&section("PERFIS"));
    sidebar.append(&scroller);
    sidebar.append(&profile_actions);
    sidebar.append(&section("SAÍDA"));
    sidebar.append(&output);
    sidebar.append(&version);

    // ---- Título + ações do perfil ----
    let title = gtk::Label::new(None);
    title.add_css_class("title-pf");
    let dirty_label = gtk::Label::new(None);
    dirty_label.add_css_class("dirty");
    let undo = gtk::Button::with_label("Desfazer");
    let save = gtk::Button::with_label("Salvar");
    save.add_css_class("suggested-action");
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    top.append(&title);
    top.append(&dirty_label);
    top.append(&spacer);
    top.append(&undo);
    top.append(&save);

    // ---- Curva + medidor ----
    let curve_ui: Rc<RefCell<Option<Rc<Ui>>>>;
    let curve_data = Rc::new(RefCell::new(curve::CurveData::default()));
    let curve_area = {
        let slot: Rc<RefCell<Option<Rc<Ui>>>> = Rc::new(RefCell::new(None));
        curve_ui = Rc::clone(&slot);
        curve::area(Rc::clone(&curve_data), move |band, gain| {
            if let Some(ui) = slot.borrow().as_ref() {
                ui.set_band(band, gain);
            }
        })
    };
    let curve_card = card();
    curve_card.set_hexpand(true);
    curve_card.append(&section("RESPOSTA EM FREQUÊNCIA"));
    curve_card.append(&curve_area);
    let meter = meter::Meter::new();
    let meter_card = card();
    meter_card.append(&section("NÍVEL L · R"));
    meter_card.append(&meter.area);
    let mid = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    mid.append(&curve_card);
    mid.append(&meter_card);

    // ---- Faders ----
    let faders_card = card();
    faders_card.append(&section("EQUALIZADOR"));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let mut bands = Vec::new();
    let mut band_values = Vec::new();
    for band in &initial.bands {
        let scale = fader();
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
        let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
        col.set_hexpand(true);
        for w in [value.upcast_ref::<gtk::Widget>(), scale.upcast_ref(), label.upcast_ref()] {
            w.set_halign(gtk::Align::Center);
            col.append(w);
        }
        row.append(&col);
        bands.push(scale);
        band_values.push(value);
    }
    faders_card.append(&row);

    // ---- Rodapé: knobs, limiter, headroom ----
    let footer = card();
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 28);
    let balance = Knob::new(-1.0, 1.0, 0.0, 0.01, (0.24, 0.86, 0.72));
    balance.widget().set_tooltip_text(Some(
        "Balanço estéreo — arraste, role ou clique duas vezes para centralizar",
    ));
    let balance_text = gtk::Label::new(None);
    let preamp = Knob::new(-24.0, 6.0, 0.0, 0.1, (0.96, 0.72, 0.24));
    preamp.widget().set_tooltip_text(Some(
        "Ganho geral antes do equalizador — clique duas vezes para zerar",
    ));
    let preamp_text = gtk::Label::new(None);
    controls.append(&knob_column(&balance, &balance_text));
    controls.append(&knob_column(&preamp, &preamp_text));

    let limiter = gtk::Switch::new();
    limiter.set_valign(gtk::Align::Center);
    limiter.set_tooltip_text(Some("Protege contra clipping no fim da cadeia"));
    let lim_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    lim_box.set_valign(gtk::Align::Center);
    lim_box.append(&gtk::Label::new(Some("Limiter")));
    lim_box.append(&limiter);
    controls.append(&lim_box);
    let headroom = gtk::Label::new(None);
    headroom.set_valign(gtk::Align::Center);
    controls.append(&headroom);
    footer.append(&controls);

    let status = gtk::Label::new(None);
    status.set_halign(gtk::Align::Start);
    status.add_css_class("version");

    let main = gtk::Box::new(gtk::Orientation::Vertical, 12);
    main.set_hexpand(true);
    main.set_margin_top(12);
    main.set_margin_bottom(8);
    main.set_margin_start(14);
    main.set_margin_end(14);
    main.append(&top);
    main.append(&mid);
    main.append(&faders_card);
    main.append(&footer);
    main.append(&status);

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.append(&sidebar);
    body.append(&main);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header);
    root.append(&body);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("PulseForge")
        .default_width(1180)
        .default_height(760)
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
        list: list.clone(),
        thumbs: RefCell::new(Vec::new()),
        title,
        output: output.clone(),
        bypass: bypass.clone(),
        menu_buttons,
        preamp: Rc::clone(&preamp),
        preamp_text,
        bands: bands.clone(),
        band_values,
        limiter: limiter.clone(),
        headroom,
        balance_text,
        meter,
        dirty_label,
        undo: undo.clone(),
        save: save.clone(),
        status,
        curve_data,
        curve_area,
    });

    *curve_ui.borrow_mut() = Some(Rc::clone(&ui));

    // Estado inicial dos widgets (sem disparar edições).
    ui.reload_list(initial_idx);
    balance.set_value(balance_value);
    ui.update_balance_text(balance_value);
    ui.push_draft_to_widgets();
    ui.loading.set(false);
    ui.refresh_view();
    ui.apply_now();

    // ---- Ligações ----
    {
        let ui2 = Rc::clone(&ui);
        preamp.on_change(move |v| {
            ui2.draft.borrow_mut().preamp_db = v as f32;
            ui2.edited();
        });
    }
    {
        let ui2 = Rc::clone(&ui);
        balance.on_change(move |v| ui2.balance_changed(v));
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
        list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                ui.profile_selected(row.index() as usize);
            }
        });
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

    // ---- Medidor de nível (dados reais do monitor do sink) ----
    {
        let rx = meter::spawn();
        let ui = Rc::clone(&ui);
        gtk::glib::timeout_add_local(Duration::from_millis(50), move || {
            let (mut l, mut r) = (0f32, 0f32);
            while let Ok((pl, pr)) = rx.try_recv() {
                l = l.max(pl);
                r = r.max(pr);
            }
            ui.meter.push(meter::level(l), meter::level(r));
            gtk::glib::ControlFlow::Continue
        });
    }

    // ---- Gerenciar perfis ----
    {
        let ui = Rc::clone(&ui);
        btn_new.connect_clicked(move |_| {
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
                        ui2.reload_list(idx);
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
                            ui2.reload_list(idx);
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
                    let idx = ui2.current.get();
                    ui2.reload_list(idx);
                    ui2.load_profile(idx);
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
                    ui2.reload_list(0);
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
