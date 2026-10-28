use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

/// Blobby metaballs with a glow. Bass inflates them, beats shift the hue.
pub struct Metaballs {
    phase: f32,
    hue: f32,
}

impl Metaballs {
    pub fn new() -> Self {
        Self { phase: 0.0, hue: 0.8 }
    }
}

impl Effect for Metaballs {
    fn name(&self) -> &'static str {
        "Metaballs"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.phase += dt * (0.4 + f.bass * 1.5);
        self.hue += dt * 0.03 + if f.beat { 0.07 } else { 0.0 };

        let (w, h) = (c.w as f32, c.h as f32);
        let size = w.min(h);
        let balls: Vec<(f32, f32, f32)> = (0..6)
            .map(|i| {
                let i = i as f32;
                let x = w * (0.5 + 0.35 * (self.phase * (0.7 + i * 0.13) + i).sin());
                let y = h * (0.5 + 0.35 * (self.phase * (0.9 + i * 0.11) + i * 2.0).cos());
                let r = size * (0.10 + 0.03 * (i * 1.7).sin() + 0.08 * f.bass + 0.05 * f.pulse);
                (x, y, r * r)
            })
            .collect();

        for y in 0..c.h {
            let fy = y as f32;
            for x in 0..c.w {
                let fx = x as f32;
                let v: f32 = balls
                    .iter()
                    .map(|&(bx, by, r2)| r2 / ((fx - bx).powi(2) + (fy - by).powi(2) + 1.0))
                    .sum();
                let col = if v >= 1.0 {
                    hsv(self.hue + v * 0.04, 0.7, (0.6 + (v - 1.0) * 0.3).min(1.0))
                } else {
                    hsv(self.hue + 0.5, 0.6, v * v * 0.35)
                };
                c.set(x, y, col);
            }
        }
    }
}
