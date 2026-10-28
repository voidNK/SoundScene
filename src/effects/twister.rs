use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};
use std::f32::consts::{FRAC_PI_2, SQRT_2};

/// The classic twister: a square column twisting along its height. Mids
/// drive the rotation, bass makes it sway.
pub struct Twister {
    phase: f32,
    hue: f32,
}

impl Twister {
    pub fn new() -> Self {
        Self { phase: 0.0, hue: 0.0 }
    }
}

impl Effect for Twister {
    fn name(&self) -> &'static str {
        "Twister"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.phase += dt * (0.6 + f.mid * 3.0);
        self.hue += dt * 0.02 + if f.beat { 0.05 } else { 0.0 };

        let (w, h) = (c.w as f32, c.h as f32);
        let width = w.min(h) * 0.28 * (1.0 + 0.25 * f.pulse);
        let twist = 6.0 * (t * 0.4).sin() / h.max(1.0);

        for y in 0..c.h {
            let fy = y as f32;
            let bg = rgb(0.03, 0.02, 0.06 + 0.06 * fy / h);
            for x in 0..c.w {
                c.set(x, y, bg);
            }
            let a = self.phase + fy * twist;
            let center = w * 0.5 + (fy * 0.04 + t * 1.3).sin() * w * 0.15 * (0.3 + f.bass);
            let xs: [f32; 4] = std::array::from_fn(|i| center + (a + i as f32 * FRAC_PI_2).sin() * width);
            for i in 0..4 {
                let (x0, x1) = (xs[i], xs[(i + 1) % 4]);
                // Only faces facing the viewer have their corners in left-to-right order.
                if x1 <= x0 {
                    continue;
                }
                let shade = ((x1 - x0) / (width * SQRT_2)).min(1.0);
                let col = hsv(self.hue + i as f32 * 0.25, 0.7, 0.25 + 0.75 * shade);
                for x in x0.max(0.0) as usize..(x1.ceil() as usize).min(c.w) {
                    c.set(x, y, col);
                }
            }
        }
    }
}
