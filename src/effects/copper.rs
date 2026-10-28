use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb, scale};

const BARS: usize = 8;

/// Amiga-style copper bars swinging up and down. Each bar's thickness
/// follows one slice of the spectrum, bass drives the swing.
pub struct Copper {
    phase: f32,
}

impl Copper {
    pub fn new() -> Self {
        Self { phase: 0.0 }
    }
}

impl Effect for Copper {
    fn name(&self) -> &'static str {
        "Copper"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.phase += dt * (0.8 + f.bass * 3.0);
        let h = c.h as f32;
        for y in 0..c.h {
            let col = rgb(0.02, 0.0, 0.05 + 0.08 * y as f32 / h);
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }

        let angle = |i: usize| self.phase * 0.8 + i as f32 * 0.55;
        // Back to front, so bars swinging towards the viewer cover the others.
        let mut order: Vec<usize> = (0..BARS).collect();
        order.sort_by(|&a, &b| angle(a).cos().total_cmp(&angle(b).cos()));

        for i in order {
            let a = angle(i);
            let center = h * (0.5 + 0.38 * a.sin());
            let band = f.spectrum[i * f.spectrum.len() / BARS];
            let thickness = (h * (0.03 + 0.06 * band)).max(1.5);
            let depth = 0.55 + 0.45 * a.cos();
            let hue = i as f32 / BARS as f32 + t * 0.03;
            let y0 = (center - thickness).floor().max(0.0) as usize;
            let y1 = ((center + thickness).ceil() as usize).min(c.h);
            for y in y0..y1 {
                let k = 1.0 - ((y as f32 - center).abs() / thickness).min(1.0);
                let base = hsv(hue, 0.75 - 0.5 * k * k, k.powf(0.6) * depth);
                for x in 0..c.w {
                    let shimmer = 0.85 + 0.15 * (x as f32 * 0.15 + t * 4.0 + i as f32).sin();
                    c.set(x, y, scale(base, shimmer));
                }
            }
        }
    }
}
