use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

/// Interference of two sets of concentric rings. Bass speeds up the waves,
/// mids tighten the rings.
pub struct Moire {
    phase: f32,
    hue: f32,
}

impl Moire {
    pub fn new() -> Self {
        Self { phase: 0.0, hue: 0.55 }
    }
}

impl Effect for Moire {
    fn name(&self) -> &'static str {
        "Moire"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.phase += dt * (2.0 + f.bass * 8.0);
        self.hue += dt * (0.02 + f.treble * 0.1);

        let (w, h) = (c.w as f32, c.h as f32);
        let size = w.min(h).max(1.0);
        let p1 = (w * (0.5 + 0.3 * (t * 0.37).sin()), h * (0.5 + 0.3 * (t * 0.29).cos()));
        let p2 = (
            w * (0.5 + 0.3 * (t * 0.23 + 2.0).cos()),
            h * (0.5 + 0.3 * (t * 0.31 + 1.0).sin()),
        );
        let freq = (0.6 + 0.6 * f.mid) * 64.0 / size;
        let flash = 0.7 + 0.3 * f.pulse;

        for y in 0..c.h {
            let fy = y as f32;
            for x in 0..c.w {
                let fx = x as f32;
                let d1 = ((fx - p1.0).powi(2) + (fy - p1.1).powi(2)).sqrt();
                let d2 = ((fx - p2.0).powi(2) + (fy - p2.1).powi(2)).sqrt();
                let v = ((d1 * freq - self.phase).sin() + (d2 * freq - self.phase).sin()) * 0.5;
                let val = (v * 0.5 + 0.5).powi(2) * flash + 0.03;
                c.set(x, y, hsv(self.hue + (d1 - d2) / size * 0.3, 0.75, val));
            }
        }
    }
}
