use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

/// Digital rain. Bass speeds up the streams, treble tints them towards cyan.
pub struct Matrix {
    heads: Vec<f32>,
    speeds: Vec<f32>,
    rng: Rng,
}

impl Matrix {
    pub fn new() -> Self {
        Self { heads: Vec::new(), speeds: Vec::new(), rng: Rng::new(0x3A7) }
    }
}

impl Effect for Matrix {
    fn name(&self) -> &'static str {
        "Matrix"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        let h = c.h as f32;
        if self.heads.len() != c.w {
            self.heads = (0..c.w).map(|_| -self.rng.range(0.0, h)).collect();
            self.speeds = (0..c.w).map(|_| self.rng.range(0.3, 1.0)).collect();
        }
        c.fade(0.88 - 0.08 * f.level);
        let tint = 0.33 - f.treble * 0.12;

        // Streams on every other column leave dark gaps between them.
        for x in (0..c.w).step_by(2) {
            let prev = self.heads[x];
            self.heads[x] += self.speeds[x] * dt * h * (0.4 + f.bass * 1.2 + f.pulse * 0.6);
            let head = self.heads[x] as i32;
            for y in prev.max(0.0) as i32..head {
                c.plot(x as i32, y, hsv(tint, 0.7, 0.6));
            }
            c.plot(x as i32, head, hsv(tint, 0.25, 1.0));
            if self.heads[x] > h + self.rng.range(0.0, h * 0.5) {
                self.heads[x] = -self.rng.range(0.0, h * 0.5);
                self.speeds[x] = self.rng.range(0.3, 1.0);
            }
        }

        // Glitter: a few trail pixels flare up.
        if c.w > 0 && c.h > 0 {
            for _ in 0..c.w * c.h / 200 {
                let x = self.rng.next_u32() as usize % c.w;
                let y = self.rng.next_u32() as usize % c.h;
                if c.get(x, y)[1] > 40 {
                    c.set(x, y, hsv(tint, 0.3, 0.9));
                }
            }
        }
    }
}
