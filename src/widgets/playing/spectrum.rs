//! `NocturneSpectrum`: audio visualizer drawn from `CurrentState::magnitudes()`.
use crate::{config::settings, integration};
use gtk::{glib, prelude::*, subclass::prelude::*};
use std::{cell::RefCell, f64::consts::PI};

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Spectrum {
        pub current: RefCell<Vec<f64>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Spectrum {
        const NAME: &'static str = "NocturneSpectrum";
        type Type = super::Spectrum;
        type ParentType = gtk::DrawingArea;
    }

    impl ObjectImpl for Spectrum {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            settings().bind("show-visualizer", &*obj, "visible").build();
            obj.set_draw_func(glib::clone!(#[weak] obj, move |_, cr, w, h| obj.draw(cr, w as f64, h as f64)));
            glib::timeout_add_local(std::time::Duration::from_millis(16), glib::clone!(#[weak] obj, #[upgrade_or] glib::ControlFlow::Break, move || {
                if obj.is_visible() {
                    obj.tick();
                }
                glib::ControlFlow::Continue
            }));
        }
    }
    impl WidgetImpl for Spectrum {}
    impl DrawingAreaImpl for Spectrum {}
}

glib::wrapper! {
    pub struct Spectrum(ObjectSubclass<imp::Spectrum>)
        @extends gtk::DrawingArea, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for Spectrum {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl Spectrum {
    fn tick(&self) {
        let state = integration::current().map(|i| i.current_state());
        let stopped = state.as_ref().is_none_or(|s| s.buttonState() == "play");
        let target: Vec<f64> = match &state {
            Some(s) if !stopped => s.magnitudes().into_iter().map(f64::from).collect(),
            _ => vec![0.0; settings().int("visualizer-bar-n").max(2) as usize],
        };
        let mut cur = self.imp().current.borrow_mut();
        if cur.len() != target.len() {
            *cur = vec![0.0; target.len()];
        }
        let speed = 0.03;
        for (c, t) in cur.iter_mut().zip(&target) {
            let m = speed * (10.0 * (t - *c).abs()).clamp(0.25, 1.0);
            *c = if t >= c { t.min(*c + m) } else { (*c - m).max(0.0) };
        }
        drop(cur);
        self.queue_draw();
    }

    fn color(&self) -> (f64, f64, f64) {
        let s = settings();
        let accent = integration::current()
            .and_then(|i| i.current_state().accent_color())
            .map(|(r, g, b)| (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0))
            .unwrap_or((0.2, 0.6, 0.9));
        if s.boolean("visualizer-auto-color") {
            if s.boolean("visualizer-auto-color-invert") { (1.0 - accent.0, 1.0 - accent.1, 1.0 - accent.2) } else { accent }
        } else {
            let v: Vec<f64> = s.string("visualizer-manual-color").split(',').filter_map(|c| c.trim().parse().ok()).collect();
            if let [r, g, b, ..] = v[..] { (r, g, b) } else { accent }
        }
    }

    fn draw(&self, cr: &gtk::cairo::Context, width: f64, height: f64) {
        let mags = self.imp().current.borrow().clone();
        let n = mags.len();
        if n < 2 || width.min(height) < 24.0 {
            return;
        }
        let s = settings();
        let dx = width / (n - 1) as f64;
        let fill = s.string("visualizer-fill-mode");
        let kind = s.string("visualizer-type");
        let border = fill == "border";
        let (r, g, b) = self.color();
        cr.set_source_rgba(r, g, b, if fill == "translucent" { 0.75 } else { 1.0 });
        if kind == "wave" {
            if !border {
                cr.move_to(0.0, height);
            }
            cr.line_to(0.0, height * (1.0 - mags[0]));
            for i in 0..n - 1 {
                let (x1, y1) = (i as f64 * dx, height * (1.0 - mags[i]));
                let (x2, y2) = ((i + 1) as f64 * dx, height * (1.0 - mags[i + 1]));
                let xc = (x1 + x2) / 2.0;
                cr.curve_to(xc, y1, xc, y2, x2, y2);
            }
            if border {
                cr.set_line_width(3.0);
                let _ = cr.stroke();
            } else {
                cr.line_to(width, height);
                cr.close_path();
                let _ = cr.fill_preserve();
            }
        } else if kind == "bars" || kind == "particles" {
            let particles = kind == "particles";
            let bar_w = dx - if particles { 4.0 } else { 2.0 };
            let radius = if particles { bar_w } else { 6.0 };
            for (i, m) in mags.iter().enumerate() {
                let x = i as f64 * dx - bar_w / 2.0;
                let (h, y) = if particles {
                    (bar_w, (height * (1.0 - m)).min(height - bar_w).max(0.0))
                } else {
                    (m * height, height - m * height)
                };
                if h <= 1.0 {
                    continue;
                }
                let r = radius.min(bar_w / 2.0).min(h / 2.0);
                cr.new_sub_path();
                cr.arc(x + bar_w - r, y + r, r, -PI / 2.0, 0.0);
                cr.arc(x + bar_w - r, y + h - r, r, 0.0, PI / 2.0);
                cr.arc(x + r, y + h - r, r, PI / 2.0, PI);
                cr.arc(x + r, y + r, r, PI, 3.0 * PI / 2.0);
                cr.close_path();
                if border {
                    cr.set_line_width(3.0);
                    let _ = cr.stroke();
                } else {
                    let _ = cr.fill();
                }
            }
        }
    }
}
