use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

/// Water surface (2D wave equation). Beats drop big stones in, treble
/// adds a drizzle of small drops.
pub struct Ripples {
    cur: Vec<f32>,
    prev: Vec<f32>,
    w: usize,
    h: usize,
    rng: Rng,
    hue: f32,
}

impl Ripples {
    pub fn new() -> Self {
        Self { cur: Vec::new(), prev: Vec::new(), w: 0, h: 0, rng: Rng::new(0x5EA), hue: 0.0 }
    }

    fn drop(&mut self, strength: f32) {
        let (w, h) = (self.w, self.h);
        let x = 2 + self.rng.next_u32() as usize % (w - 4);
        let y = 2 + self.rng.next_u32() as usize % (h - 4);
        for dy in 0..5 {
            for dx in 0..5 {
                self.cur[(y + dy - 2) * w + x + dx - 2] += strength;
            }
        }
    }
}

impl Effect for Ripples {
    fn name(&self) -> &'static str {
        "Ripples"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if (self.w, self.h) != (c.w, c.h) {
            (self.w, self.h) = (c.w, c.h);
            self.cur = vec![0.0; c.w * c.h];
            self.prev = vec![0.0; c.w * c.h];
        }
        let (w, h) = (self.w, self.h);
        if w < 6 || h < 6 {
            return;
        }
        self.hue += dt * 0.01;
        if f.beat {
            self.hue += 0.03;
            self.drop(4.0 + 4.0 * f.bass);
        }
        if self.rng.f32() < f.treble * 0.3 {
            self.drop(1.5);
        }

        // `prev` becomes the next state, then the buffers swap.
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let i = y * w + x;
                let v = (self.cur[i - 1] + self.cur[i + 1] + self.cur[i - w] + self.cur[i + w]) * 0.5 - self.prev[i];
                self.prev[i] = v * 0.97;
            }
        }
        std::mem::swap(&mut self.cur, &mut self.prev);

        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let slope = if x > 0 && x < w - 1 { self.cur[i + 1] - self.cur[i - 1] } else { 0.0 };
                let val = (0.25 + slope * 0.35 + self.cur[i] * 0.1).clamp(0.02, 1.0);
                c.set(x, y, hsv(0.55 + self.hue, 0.7 - slope.abs().min(0.5), val));
            }
        }
    }
}
