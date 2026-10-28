use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

/// Conway's Game of Life on a wrapping board. Bass speeds up the
/// generations, beats seed new noise, and cells change color as they age.
pub struct Life {
    cells: Vec<u16>,
    next: Vec<u16>,
    w: usize,
    h: usize,
    rng: Rng,
    acc: f32,
    hue: f32,
}

impl Life {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            next: Vec::new(),
            w: 0,
            h: 0,
            rng: Rng::new(0x11FE),
            acc: 0.0,
            hue: 0.3,
        }
    }

    /// Randomly sets cells alive in a rectangle.
    fn seed(&mut self, x0: usize, y0: usize, bw: usize, bh: usize, density: f32) {
        for y in y0..(y0 + bh).min(self.h) {
            for x in x0..(x0 + bw).min(self.w) {
                if self.rng.f32() < density {
                    self.cells[y * self.w + x] = 1;
                }
            }
        }
    }

    fn step(&mut self) {
        let (w, h) = (self.w, self.h);
        for y in 0..h {
            let (up, down) = ((y + h - 1) % h, (y + 1) % h);
            for x in 0..w {
                let (left, right) = ((x + w - 1) % w, (x + 1) % w);
                let n = [
                    up * w + left,
                    up * w + x,
                    up * w + right,
                    y * w + left,
                    y * w + right,
                    down * w + left,
                    down * w + x,
                    down * w + right,
                ]
                .iter()
                .filter(|&&i| self.cells[i] > 0)
                .count();
                let age = self.cells[y * w + x];
                self.next[y * w + x] = match (age > 0, n) {
                    (true, 2 | 3) => age.saturating_add(1),
                    (false, 3) => 1,
                    _ => 0,
                };
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
    }
}

impl Effect for Life {
    fn name(&self) -> &'static str {
        "Life"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if (self.w, self.h) != (c.w, c.h) {
            (self.w, self.h) = (c.w, c.h);
            self.cells = vec![0; c.w * c.h];
            self.next = vec![0; c.w * c.h];
            self.seed(0, 0, c.w, c.h, 0.3);
        }
        let (w, h) = (self.w, self.h);
        if w < 3 || h < 3 {
            return;
        }

        if f.beat {
            self.hue += 0.08;
            let (x, y) = (self.rng.next_u32() as usize % w, self.rng.next_u32() as usize % h);
            self.seed(x, y, 12, 12, 0.45);
        }
        self.acc = (self.acc + dt * (6.0 + f.bass * 24.0)).min(4.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.step();
        }
        if self.cells.iter().filter(|&&a| a > 0).count() < w * h / 40 {
            self.seed(0, 0, w, h, 0.2);
        }

        // Dead cells fade out instead of vanishing.
        c.fade(0.7);
        for y in 0..h {
            for x in 0..w {
                let age = self.cells[y * w + x];
                if age > 0 {
                    let a = age.min(60) as f32;
                    c.set(x, y, hsv(self.hue + a * 0.008, 0.7, 1.0 - a / 120.0));
                }
            }
        }
    }
}
