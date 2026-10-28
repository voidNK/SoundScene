use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

const SEGMENTS: f32 = 8.0;

/// Plasma folded into eight mirrored segments. Bass zooms and speeds it up,
/// mids rotate it.
pub struct Kaleidoscope {
    rot: f32,
    phase: f32,
    hue: f32,
}

impl Kaleidoscope {
    pub fn new() -> Self {
        Self { rot: 0.0, phase: 0.0, hue: 0.3 }
    }
}

impl Effect for Kaleidoscope {
    fn name(&self) -> &'static str {
        "Kaleidoscope"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.rot += dt * (0.1 + f.mid * 0.8);
        self.phase += dt * (0.5 + f.bass * 2.5);
        self.hue += dt * 0.03 + if f.beat { 0.06 } else { 0.0 };

        let seg = TAU / SEGMENTS;
        let scale = (5.0 - 2.0 * f.bass) / c.w.min(c.h).max(1) as f32;
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let p = self.phase;
        let flash = 0.7 + 0.3 * f.pulse;

        for y in 0..c.h {
            let dy = y as f32 - cy;
            for x in 0..c.w {
                let dx = x as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt() * scale;
                let mut a = (dy.atan2(dx) + self.rot).rem_euclid(seg);
                if a > seg * 0.5 {
                    a = seg - a;
                }
                let (u, v) = (r * a.cos(), r * a.sin());
                let val = ((u * 3.0 + p).sin()
                    + (v * 4.0 - p * 1.3).sin()
                    + ((u + v) * 2.0 + p * 0.7).sin()
                    + (r * 2.0 - p).sin())
                    * 0.25;
                let bright = (0.5 + 0.5 * (val * TAU).sin()).powf(1.5) * flash + 0.05;
                c.set(x, y, hsv(self.hue + val * 0.3 + r * 0.05, 0.8, bright));
            }
        }
    }
}
