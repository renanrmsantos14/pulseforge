//! Medidor de nível L/R lido do monitor do sink `pulseforge` (via `parec`).
use gtk::prelude::*;
use gtk4 as gtk;
use std::{
    cell::Cell,
    io::Read,
    process::{Command, Stdio},
    rc::Rc,
    sync::mpsc,
    time::Duration,
};

const SEGMENTS: usize = 18;
const FLOOR_DB: f32 = -60.0;

/// Inicia a leitura em segundo plano; envia (pico L, pico R) a cada ~50 ms.
/// A thread termina quando o receptor é descartado.
pub fn spawn() -> mpsc::Receiver<(f32, f32)> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || loop {
        let child = Command::new("parec")
            .args([
                "-d",
                "pulseforge.monitor",
                "--format=float32le",
                "--channels=2",
                "--rate=24000",
                "--latency-msec=25",
                "--raw",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut child) = child {
            if let Some(mut out) = child.stdout.take() {
                let mut buf = [0u8; 8 * 1200];
                while out.read_exact(&mut buf).is_ok() {
                    let (mut l, mut r) = (0f32, 0f32);
                    for frame in buf.as_chunks::<8>().0 {
                        l = l.max(f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]).abs());
                        r = r.max(f32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]).abs());
                    }
                    if tx.send((l, r)).is_err() {
                        let _ = child.kill();
                        return;
                    }
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        // O monitor some quando o filtro reinicia: tenta de novo.
        std::thread::sleep(Duration::from_secs(1));
        if tx.send((0.0, 0.0)).is_err() {
            return;
        }
    });
    rx
}

pub fn level(peak: f32) -> f64 {
    let db = 20.0 * peak.max(1e-6).log10();
    f64::from(((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0))
}

pub struct Meter {
    pub area: gtk::DrawingArea,
    left: Rc<Cell<f64>>,
    right: Rc<Cell<f64>>,
}

impl Meter {
    pub fn new() -> Self {
        let (left, right) = (Rc::new(Cell::new(0.0)), Rc::new(Cell::new(0.0)));
        let area = gtk::DrawingArea::builder().content_width(110).vexpand(true).build();
        area.set_tooltip_text(Some("Nível de saída (pico) após o equalizador"));
        let (l, r) = (Rc::clone(&left), Rc::clone(&right));
        area.set_draw_func(move |_, cr, w, h| {
            let (w, h) = (f64::from(w), f64::from(h) - 16.0);
            let col_w = (w - 12.0) / 2.0;
            let seg_h = h / SEGMENTS as f64;
            for (c, level) in [l.get(), r.get()].into_iter().enumerate() {
                let x = c as f64 * (col_w + 12.0);
                let lit = (level * SEGMENTS as f64).round() as usize;
                for i in 0..SEGMENTS {
                    let (red, grn, blu) = if i >= SEGMENTS - 2 {
                        (1.0, 0.42, 0.42)
                    } else if i >= SEGMENTS - 5 {
                        (0.96, 0.72, 0.24)
                    } else {
                        (0.24, 0.86, 0.72)
                    };
                    cr.set_source_rgba(red, grn, blu, if i < lit { 1.0 } else { 0.16 });
                    let y = h - (i as f64 + 1.0) * seg_h;
                    cr.rectangle(x, y + 1.5, col_w, seg_h - 3.0);
                    let _ = cr.fill();
                }
                cr.set_source_rgb(0.54, 0.58, 0.64);
                cr.set_font_size(10.0);
                cr.move_to(x + col_w / 2.0 - 3.0, h + 13.0);
                let _ = cr.show_text(if c == 0 { "L" } else { "R" });
            }
        });
        Self { area, left, right }
    }

    /// Aplica novo pico com queda suave.
    pub fn push(&self, l: f64, r: f64) {
        for (cell, new) in [(&self.left, l), (&self.right, r)] {
            let old = cell.get();
            cell.set(if new >= old { new } else { (old - 0.05).max(new) });
        }
        self.area.queue_draw();
    }
}
