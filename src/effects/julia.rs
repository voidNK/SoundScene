use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

const MAX_ITER: u32 = 40;

/// Animated Julia set: the constant c travels around a circle, which keeps
/// the fractal on the interesting edge. Mids push it along, bass zooms in.
pub struct Julia {
    angle: f32,
    hue: f32,
}

impl Julia {
    pub fn new() -> Self {
        Self { angle: 0.0, hue: 0.6 }
    }
}

impl Effect for Julia {
    fn name(&self) -> &'static str {
        "Julia"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.angle += dt * (0.08 + f.mid * 0.3) + if f.beat { 0.03 } else { 0.0 };
        self.hue += dt * 0.03;

        let (cr, ci) = (0.7885 * self.angle.cos(), 0.7885 * self.angle.sin());
        let zoom = 1.5 / (c.w.min(c.h).max(1) as f32 * 0.5) * (1.0 - 0.15 * f.bass);
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let flash = 0.7 + 0.3 * f.pulse;

        for y in 0..c.h {
            for x in 0..c.w {
                let (mut zr, mut zi) = ((x as f32 - cx) * zoom, (y as f32 - cy) * zoom);
                let mut n = 0;
                while n < MAX_ITER && zr * zr + zi * zi < 16.0 {
                    (zr, zi) = (zr * zr - zi * zi + cr, 2.0 * zr * zi + ci);
                    n += 1;
                }
                let col = if n == MAX_ITER {
                    hsv(self.hue + 0.5, 0.6, 0.1 * f.pulse)
                } else {
                    // Smooth iteration count, avoids visible color bands.
                    let s = n as f32 + 1.0 - (zr * zr + zi * zi).sqrt().ln().log2();
                    hsv(self.hue + s * 0.025, 0.75, (s / MAX_ITER as f32 * 3.0).min(1.0) * flash)
                };
                c.set(x, y, col);
            }
        }
    }
}
