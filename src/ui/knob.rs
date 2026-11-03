//! Knob rotativo (arrastar na vertical, rolar, duplo clique zera, setas).
use gtk::prelude::*;
use gtk4 as gtk;
use std::{cell::{Cell, RefCell}, f64::consts::PI, rc::Rc};

type Handler = Box<dyn Fn(f64)>;

pub struct Knob {
    area: gtk::DrawingArea,
    value: Rc<Cell<f64>>,
    min: f64,
    max: f64,
    default: f64,
    step: f64,
    handler: RefCell<Option<Handler>>,
}

impl Knob {
    pub fn new(min: f64, max: f64, default: f64, step: f64, rgb: (f64, f64, f64)) -> Rc<Self> {
        let value = Rc::new(Cell::new(default));
        let area = gtk::DrawingArea::builder()
            .content_width(60)
            .content_height(60)
            .focusable(true)
            .halign(gtk::Align::Center)
            .build();
        area.set_cursor_from_name(Some("ns-resize"));
        {
            let value = Rc::clone(&value);
            area.set_draw_func(move |widget, cr, w, h| {
                let (w, h) = (f64::from(w), f64::from(h));
                let (cx, cy, r) = (w / 2.0, h / 2.0, w.min(h) / 2.0 - 7.0);
                let ang = |v: f64| 0.75 * PI + ((v - min) / (max - min)).clamp(0.0, 1.0) * 1.5 * PI;
                cr.set_line_cap(gtk::cairo::LineCap::Round);
                cr.set_line_width(4.0);
                cr.set_source_rgb(0.15, 0.17, 0.21);
                cr.arc(cx, cy, r, 0.75 * PI, 2.25 * PI);
                let _ = cr.stroke();
                let v = value.get();
                let (a, b) = (ang(default).min(ang(v)), ang(default).max(ang(v)));
                if b - a > 0.001 {
                    cr.set_source_rgb(rgb.0, rgb.1, rgb.2);
                    cr.arc(cx, cy, r, a, b);
                    let _ = cr.stroke();
                }
                cr.set_source_rgb(0.9, 0.91, 0.93);
                cr.set_line_width(2.5);
                cr.move_to(cx + 0.35 * r * ang(v).cos(), cy + 0.35 * r * ang(v).sin());
                cr.line_to(cx + 0.9 * r * ang(v).cos(), cy + 0.9 * r * ang(v).sin());
                let _ = cr.stroke();
                if gtk::prelude::WidgetExt::has_focus(widget) {
                    cr.set_source_rgba(rgb.0, rgb.1, rgb.2, 0.5);
                    cr.set_line_width(1.0);
                    cr.arc(cx, cy, r + 5.0, 0.0, 2.0 * PI);
                    let _ = cr.stroke();
                }
            });
        }
        let knob = Rc::new(Self { area, value, min, max, default, step, handler: RefCell::new(None) });

        let drag = gtk::GestureDrag::new();
        let start = Rc::new(Cell::new(0.0));
        {
            let (k, s) = (Rc::clone(&knob), Rc::clone(&start));
            drag.connect_drag_begin(move |_, _, _| {
                k.area.grab_focus();
                s.set(k.value.get());
            });
        }
        {
            let (k, s) = (Rc::clone(&knob), Rc::clone(&start));
            drag.connect_drag_update(move |_, _, dy| {
                k.user_set(s.get() - dy / 160.0 * (k.max - k.min));
            });
        }
        knob.area.add_controller(drag);

        let click = gtk::GestureClick::new();
        {
            let k = Rc::clone(&knob);
            click.connect_pressed(move |_, n, _, _| {
                if n == 2 {
                    k.user_set(k.default);
                }
            });
        }
        knob.area.add_controller(click);

        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        {
            let k = Rc::clone(&knob);
            scroll.connect_scroll(move |_, _, dy| {
                k.user_set(k.value.get() - dy * k.step * 5.0);
                gtk::glib::Propagation::Stop
            });
        }
        knob.area.add_controller(scroll);

        let keys = gtk::EventControllerKey::new();
        {
            let k = Rc::clone(&knob);
            keys.connect_key_pressed(move |_, key, _, _| {
                let d = match key {
                    gtk::gdk::Key::Up | gtk::gdk::Key::Right => 1.0,
                    gtk::gdk::Key::Down | gtk::gdk::Key::Left => -1.0,
                    _ => return gtk::glib::Propagation::Proceed,
                };
                k.user_set(k.value.get() + d * k.step * 5.0);
                gtk::glib::Propagation::Stop
            });
        }
        knob.area.add_controller(keys);
        let focus = gtk::EventControllerFocus::new();
        {
            let a = knob.area.clone();
            focus.connect_enter(move |_| a.queue_draw());
        }
        {
            let a = knob.area.clone();
            focus.connect_leave(move |_| a.queue_draw());
        }
        knob.area.add_controller(focus);
        knob
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.area
    }

    /// Altera o valor sem chamar o handler (uso programático).
    pub fn set_value(&self, v: f64) {
        self.value.set(v.clamp(self.min, self.max));
        self.area.queue_draw();
    }

    pub fn on_change(&self, f: impl Fn(f64) + 'static) {
        *self.handler.borrow_mut() = Some(Box::new(f));
    }

    fn user_set(&self, raw: f64) {
        let v = ((raw.clamp(self.min, self.max) / self.step).round() * self.step).clamp(self.min, self.max);
        if (v - self.value.get()).abs() < 1e-9 {
            return;
        }
        self.value.set(v);
        self.area.queue_draw();
        if let Some(h) = self.handler.borrow().as_ref() {
            h(v);
        }
    }
}
