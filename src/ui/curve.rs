use gtk::prelude::*;
use gtk4 as gtk;
use std::{cell::RefCell, f64::consts::PI, rc::Rc};

pub const MIN_HZ: f64 = 20.0;
pub const MAX_HZ: f64 = 20_000.0;
const RANGE_DB: f64 = 18.0;

#[derive(Default)]
pub struct CurveData {
    /// Resposta total (frequência, dB), em escala log.
    pub points: Vec<(f64, f64)>,
    /// Posição de cada banda sobre a curva (frequência, dB).
    pub markers: Vec<(f64, f64)>,
}

pub fn area(data: Rc<RefCell<CurveData>>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_height(190)
        .hexpand(true)
        .build();
    area.set_tooltip_text(Some("Resposta em frequência do perfil (pré-amp + bandas)"));
    area.set_draw_func(move |_, cr, width, height| draw(&data.borrow(), cr, f64::from(width), f64::from(height)));
    area
}

fn draw(data: &CurveData, cr: &gtk::cairo::Context, w: f64, h: f64) {
    let (left, right, top, bottom) = (38.0, 12.0, 10.0, 22.0);
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

    for (f, db) in &data.markers {
        cr.set_source_rgba(0.06, 0.07, 0.09, 1.0);
        cr.arc(x(*f), y(*db), 4.2, 0.0, 2.0 * PI);
        let _ = cr.fill_preserve();
        cr.set_source_rgba(0.24, 0.86, 0.72, 1.0);
        cr.set_line_width(1.6);
        let _ = cr.stroke();
    }
}
