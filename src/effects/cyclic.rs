use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv};

/// (states, threshold, Moore neighborhood) — each one grows spirals.
const VARIANTS: [(u8, u8, bool); 3] = [(14, 1, false), (3, 3, true), (4, 2, true)];
const LIFETIME: f32 = 40.0;

/// Cyclic cellular automaton: a cell advances to the next of N states when
/// enough neighbors are already there, which lets rotating spirals emerge
/// from noise. Loudness sets the speed, each state lights up with its own
/// spectrum band, and beats drop a blob of fresh noise into the board.
pub struct Cyclic {
    cells: Vec<u8>,
    next: Vec<u8>,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    variant: usize,
    rng: Rng,
    acc: f32,
    age: f32,
    hue: f32,
}

impl Cyclic {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            next: Vec::new(),
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            variant: 0,
            rng: Rng::new(0xC1C),
            acc: 0.0,
            age: 0.0,
            hue: 0.0,
        }
    }

    fn states(&self) -> u8 {
        VARIANTS[self.variant].0
    }

    fn randomize(&mut self) {
        let n = self.states() as u32;
        for v in &mut self.cells {
            *v = (self.rng.next_u32() % n) as u8;
        }
        self.age = 0.0;
    }

    fn perturb(&mut self) {
        let (gw, gh) = (self.gw, self.gh);
        let r = (gw.min(gh) / 10).max(2) as i32;
        let (cx, cy) = (
            (self.rng.next_u32() as usize % gw) as i32,
            (self.rng.next_u32() as usize % gh) as i32,
        );
        let n = self.states() as u32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let x = (cx + dx).rem_euclid(gw as i32) as usize;
                    let y = (cy + dy).rem_euclid(gh as i32) as usize;
                    self.cells[y * gw + x] = (self.rng.next_u32() % n) as u8;
                }
            }
        }
    }

    /// Returns the number of cells that changed.
    fn step(&mut self) -> usize {
        let (n, threshold, moore) = VARIANTS[self.variant];
        let (w, h) = (self.gw, self.gh);
        let mut changed = 0;
        for y in 0..h {
            let (up, down) = (((y + h - 1) % h) * w, ((y + 1) % h) * w);
            let row = y * w;
            for x in 0..w {
                let (l, r) = ((x + w - 1) % w, (x + 1) % w);
                let s = self.cells[row + x];
                let succ = if s + 1 >= n { 0 } else { s + 1 };
                let cells = &self.cells;
                let is = |i: usize| (cells[i] == succ) as u8;
                let mut count = is(up + x) + is(down + x) + is(row + l) + is(row + r);
                if moore {
                    count += is(up + l) + is(up + r) + is(down + l) + is(down + r);
                }
                let v = if count >= threshold {
                    changed += 1;
                    succ
                } else {
                    s
                };
                self.next[row + x] = v;
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        changed
    }
}

impl Effect for Cyclic {
    fn name(&self) -> &'static str {
        "Cyclic"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.scale = if c.w >= 100 && c.h >= 50 { 2 } else { 1 };
            self.gw = c.w.div_ceil(self.scale);
            self.gh = c.h.div_ceil(self.scale);
            self.cells = vec![0; self.gw * self.gh];
            self.next = vec![0; self.gw * self.gh];
            self.randomize();
        }
        if self.gw < 3 || self.gh < 3 {
            c.clear();
            return;
        }

        self.hue += dt * 0.015;
        if f.beat && self.rng.f32() < 0.4 {
            self.perturb();
        }
        self.age += dt;
        if self.age > LIFETIME {
            self.variant = (self.variant + 1) % VARIANTS.len();
            self.randomize();
        }
        self.acc = (self.acc + dt * (8.0 + f.level * 25.0 + f.bass * 20.0)).min(3.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            if self.step() == 0 {
                self.randomize();
            }
        }

        let n = self.states() as usize;
        let bins = f.spectrum.len();
        let palette: Vec<Rgb> = (0..n)
            .map(|k| {
                let band = if bins == 0 {
                    0.5
                } else {
                    f.spectrum[(k * bins / n).min(bins - 1)]
                };
                hsv(
                    self.hue + k as f32 / n as f32 * 0.8,
                    0.8,
                    0.35 + 0.5 * band + 0.15 * f.pulse,
                )
            })
            .collect();
        let s = self.scale;
        for y in 0..c.h {
            let row = (y / s) * self.gw;
            for x in 0..c.w {
                c.set(x, y, palette[self.cells[row + x / s] as usize % n]);
            }
        }
    }
}
