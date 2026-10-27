use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

/// Sum-of-sines plasma. Bass drives speed, mid the zoom, treble the hue drift.
pub struct Plasma {
    phase: f32,
    hue: f32,
}

impl Plasma {
    pub fn new() -> Self {
        Self { phase: 0.0, hue: 0.0 }
    }
}

impl Effect for Plasma {
    fn name(&self) -> &'static str {
        "Plasma"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.phase += dt * (0.6 + f.bass * 2.5);
        self.hue += dt * (0.03 + f.treble * 0.25) + if f.beat { 0.08 } else { 0.0 };

        let p = self.phase;
        let inv = (8.0 + f.mid * 4.0) / c.w.max(c.h).max(1) as f32;
        let cx = (p * 0.31).sin() * 3.0 + c.w as f32 * inv * 0.5;
        let cy = (p * 0.23).cos() * 3.0 + c.h as f32 * inv * 0.5;
        let bright = 0.55 + 0.45 * f.pulse;

        for y in 0..c.h {
            let fy = y as f32 * inv;
            for x in 0..c.w {
                let fx = x as f32 * inv;
                let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
                let v = ((fx + p).sin()
                    + (fy * 1.3 - p * 0.8).sin()
                    + ((fx + fy) * 0.7 + p * 1.2).sin()
                    + (d * 1.6 - p * 2.0).sin())
                    * 0.25;
                let val = (0.5 + 0.5 * (v * TAU).sin()) * bright + 0.1;
                c.set(x, y, hsv(self.hue + v * 0.4, 0.75, val));
            }
        }
    }
}
