use gtk::prelude::*;
use gtk4 as gtk;
use std::{
    cell::{Cell, RefCell},
    f64::consts::PI,
    rc::Rc,
};

fn dragging(area: &gtk::DrawingArea) -> bool {
    area.has_css_class("dragging")
}

fn set_dragging(area: &gtk::DrawingArea, on: bool) {
    if on {
        area.add_css_class("dragging");
    } else {
        area.remove_css_class("dragging");
    }
}

pub const MIN_HZ: f64 = 20.0;
pub const MAX_HZ: f64 = 20_000.0;
const RANGE_DB: f64 = 18.0;
const PAD: (f64, f64, f64, f64) = (38.0, 12.0, 10.0, 22.0); // esquerda, direita, topo, base
const HIT_RADIUS: f64 = 16.0;

#[derive(Default)]
pub struct CurveData {
    /// Resposta total (frequência, dB), em escala log.
    pub points: Vec<(f64, f64)>,
    /// Posição de cada banda sobre a curva (frequência, dB).
    pub markers: Vec<(f64, f64)>,
    /// Banda sob o cursor ou sendo arrastada.
    pub active: Option<usize>,
    /// Ganho (dB) de cada banda no rascunho, usado como base do arraste.
    pub gains: Vec<f64>,
}

fn to_xy(f: f64, db: f64, w: f64, h: f64) -> (f64, f64) {
    let (left, right, top, bottom) = PAD;
    let (pw, ph) = (w - left - right, h - top - bottom);
    (
        left + (f.ln() - MIN_HZ.ln()) / (MAX_HZ.ln() - MIN_HZ.ln()) * pw,
        top + (RANGE_DB - db.clamp(-RANGE_DB, RANGE_DB)) / (2.0 * RANGE_DB) * ph,
    )
}

fn hit(data: &CurveData, px: f64, py: f64, w: f64, h: f64) -> Option<usize> {
    data.markers
        .iter()
        .enumerate()
        .map(|(i, (f, db))| {
            let (x, y) = to_xy(*f, *db, w, h);
            (i, (x - px).hypot(y - py))
        })
        .filter(|(_, d)| *d <= HIT_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// `on_gain(banda, ganho_db)` é chamado ao arrastar um ponto (ganho absoluto
/// em dB, já limitado a ±12 e arredondado a 0,1) ou ao dar duplo clique (0 dB).
pub fn area(data: Rc<RefCell<CurveData>>, on_gain: impl Fn(usize, f64) + 'static) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_height(190)
        .hexpand(true)
        .build();
    area.set_tooltip_text(Some(
        "Resposta do perfil — arraste um ponto para ajustar a banda; duplo clique zera",
    ));
    {
        let data = Rc::clone(&data);
        area.set_draw_func(move |_, cr, width, height| {
            draw(&data.borrow(), cr, f64::from(width), f64::from(height))
        });
    }
    let on_gain = Rc::new(on_gain);

    // Cursor/realce ao passar sobre um ponto.
    let motion = gtk::EventControllerMotion::new();
    {
        let (data, area) = (Rc::clone(&data), area.clone());
        motion.connect_motion(move |_, x, y| {
            let (w, h) = (f64::from(area.width()), f64::from(area.height()));
            let mut d = data.borrow_mut();
            if d.active.is_some() && dragging(&area) {
                return;
            }
            let found = hit(&d, x, y, w, h);
            if found != d.active {
                d.active = found;
                area.set_cursor_from_name(Some(if found.is_some() { "grab" } else { "default" }));
                area.queue_draw();
            }
        });
    }
    {
        let (data, area) = (Rc::clone(&data), area.clone());
        motion.connect_leave(move |_| {
            if !dragging(&area) {
                data.borrow_mut().active = None;
                area.queue_draw();
            }
        });
    }
    area.add_controller(motion);

    // Arrastar: ganho = ganho inicial + deslocamento vertical convertido em dB.
    let drag = gtk::GestureDrag::new();
    let state: Rc<Cell<Option<(usize, f64)>>> = Rc::new(Cell::new(None)); // (banda, ganho inicial)
    {
        let (data, area, state) = (Rc::clone(&data), area.clone(), Rc::clone(&state));
        drag.connect_drag_begin(move |_, x, y| {
            let (w, h) = (f64::from(area.width()), f64::from(area.height()));
            let mut d = data.borrow_mut();
            match hit(&d, x, y, w, h) {
                Some(i) => {
                    d.active = Some(i);
                    area.set_cursor_from_name(Some("grabbing"));
                    set_dragging(&area, true);
                    state.set(Some((i, d.gains.get(i).copied().unwrap_or(0.0))));
                }
                None => state.set(None),
            }
            area.queue_draw();
        });
    }
    {
        let (area, state, on_gain) = (area.clone(), Rc::clone(&state), Rc::clone(&on_gain));
        drag.connect_drag_update(move |_, _, dy| {
            if let Some((i, start)) = state.get() {
                let ph = f64::from(area.height()) - PAD.2 - PAD.3;
                let db = start - dy / ph * 2.0 * RANGE_DB;
                let db = (db.clamp(-12.0, 12.0) * 10.0).round() / 10.0;
                on_gain(i, if db.abs() < 0.25 { 0.0 } else { db });
            }
        });
    }
    {
        let (data, area, state) = (Rc::clone(&data), area.clone(), Rc::clone(&state));
        drag.connect_drag_end(move |_, _, _| {
            state.set(None);
            set_dragging(&area, false);
            data.borrow_mut().active = None;
            area.set_cursor_from_name(Some("default"));
            area.queue_draw();
        });
    }
    area.add_controller(drag);

    let click = gtk::GestureClick::new();
    {
        let (data, area) = (Rc::clone(&data), area.clone());
        click.connect_pressed(move |_, n, x, y| {
            if n == 2 {
                let (w, h) = (f64::from(area.width()), f64::from(area.height()));
                let found = hit(&data.borrow(), x, y, w, h);
                if let Some(i) = found {
                    on_gain(i, 0.0);
                }
            }
        });
    }
    area.add_controller(click);
    area
}

fn draw(data: &CurveData, cr: &gtk::cairo::Context, w: f64, h: f64) {
    let (left, right, top, bottom) = PAD;
    let (pw, ph) = (w - left - right, h - top - bottom);
    if pw <= 0.0 || ph <= 0.0 {
        return;
    }
    let x = |f: f64| left + (f.ln() - MIN_HZ.ln()) / (MAX_HZ.ln() - MIN_HZ.ln()) * pw;
    let y = |db: f64| top + (RANGE_DB - db.clamp(-RANGE_DB, RANGE_DB)) / (2.0 * RANGE_DB) * ph;
    cr.select_font_face("Sans", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Normal);
    cr.set_font_size(10.0);

    // Grade: ganho (dB) e frequência (Hz).
    for db in [-12.0, -6.0, 0.0, 6.0, 12.0] {
        let zero = db == 0.0;
        cr.set_source_rgba(1.0, 1.0, 1.0, if zero { 0.22 } else { 0.07 });
        cr.set_line_width(1.0);
        cr.move_to(left, y(db));
        cr.line_to(left + pw, y(db));
        let _ = cr.stroke();
        cr.set_source_rgba(0.55, 0.58, 0.64, 1.0);
        cr.move_to(4.0, y(db) + 3.5);
        let _ = cr.show_text(&format!("{db:+.0}"));
    }
    for (f, label) in [(50.0, "50"), (100.0, "100"), (500.0, "500"), (1000.0, "1k"), (5000.0, "5k"), (10000.0, "10k")] {
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
        cr.move_to(x(f), top);
        cr.line_to(x(f), top + ph);
        let _ = cr.stroke();
        cr.set_source_rgba(0.55, 0.58, 0.64, 1.0);
        cr.move_to(x(f) - 6.0, h - 6.0);
        let _ = cr.show_text(label);
    }
    if data.points.len() < 2 {
        return;
    }

    // Área sob/sobre a curva em relação a 0 dB e a linha da resposta.
    cr.move_to(x(data.points[0].0), y(0.0));
    for (f, db) in &data.points {
        cr.line_to(x(*f), y(*db));
    }
    cr.line_to(x(data.points[data.points.len() - 1].0), y(0.0));
    cr.close_path();
    cr.set_source_rgba(0.24, 0.86, 0.72, 0.14);
    let _ = cr.fill();

    cr.set_line_width(2.2);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    cr.set_source_rgba(0.24, 0.86, 0.72, 1.0);
    cr.move_to(x(data.points[0].0), y(data.points[0].1));
    for (f, db) in &data.points[1..] {
        cr.line_to(x(*f), y(*db));
    }
    let _ = cr.stroke();

    for (i, (f, db)) in data.markers.iter().enumerate() {
        let active = data.active == Some(i);
        if active {
            cr.set_source_rgba(0.24, 0.86, 0.72, 0.25);
            cr.arc(x(*f), y(*db), 12.0, 0.0, 2.0 * PI);
            let _ = cr.fill();
        }
        cr.set_source_rgba(0.06, 0.07, 0.09, 1.0);
        cr.arc(x(*f), y(*db), if active { 6.0 } else { 4.2 }, 0.0, 2.0 * PI);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(0.24, 0.86, 0.72, 1.0);
        cr.set_line_width(1.6);
        let _ = cr.stroke();
    }
}
