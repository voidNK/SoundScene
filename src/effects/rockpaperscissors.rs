use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, mix};

/// Seconds before the board restarts with a different number of species.
const LIFETIME: f32 = 35.0;

/// Rock-paper-scissors automaton: each of 3 to 5 species is eaten by the
/// next one, and a cell is taken over when enough of its eight neighbors are
/// predators (with a random threshold), which winds the board into noisy
/// spirals. Loudness sets the speed, beats drop a burst of mixed species,
/// fronts glow where cells were just conquered and each species' brightness
/// follows its own spectrum band.
pub struct RockPaperScissors {
    cells: Vec<u8>,
    next: Vec<u8>,
    fresh: Vec<f32>,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    species: u8,
    rng: Rng,
    acc: f32,
    age: f32,
    hue: f32,
}

impl RockPaperScissors {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            next: Vec::new(),
            fresh: Vec::new(),
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            species: 3,
            rng: Rng::new(0x125),
            acc: 0.0,
            age: 0.0,
            hue: 0.0,
        }
    }

    fn randomize(&mut self) {
        let n = self.species as u32;
        for v in &mut self.cells {
            *v = (self.rng.next_u32() % n) as u8;
        }
        self.age = 0.0;
    }

    fn burst(&mut self) {
        let (gw, gh) = (self.gw as i32, self.gh as i32);
        let r = (gw.min(gh) / 8).max(2);
        let cx = (self.rng.next_u32() % gw as u32) as i32;
        let cy = (self.rng.next_u32() % gh as u32) as i32;
        let n = self.species as u32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    let x = (cx + dx).rem_euclid(gw) as usize;
                    let y = (cy + dy).rem_euclid(gh) as usize;
                    self.cells[y * self.gw + x] = (self.rng.next_u32() % n) as u8;
                }
            }
        }
    }

    /// Returns the number of cells that changed.
    fn step(&mut self) -> usize {
        let (w, h) = (self.gw, self.gh);
        let n = self.species;
        let mut changed = 0;
        for y in 0..h {
            let (up, down) = (((y + h - 1) % h) * w, ((y + 1) % h) * w);
            let row = y * w;
            for x in 0..w {
                let (l, r) = ((x + w - 1) % w, (x + 1) % w);
                let s = self.cells[row + x];
                let pred = if s + 1 >= n { 0 } else { s + 1 };
                let cells = &self.cells;
                let is = |i: usize| (cells[i] == pred) as u32;
                let count = is(up + l)
                    + is(up + x)
                    + is(up + r)
                    + is(row + l)
                    + is(row + r)
                    + is(down + l)
                    + is(down + x)
                    + is(down + r);
                let threshold = 2 + (self.rng.next_u32() & 3).min(2);
                let v = if count > threshold {
                    changed += 1;
                    self.fresh[row + x] = 1.0;
                    pred
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

impl Effect for RockPaperScissors {
    fn name(&self) -> &'static str {
        "Rock Paper Scissors"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.scale = if c.w >= 160 && c.h >= 80 { 2 } else { 1 };
            self.gw = c.w.div_ceil(self.scale);
            self.gh = c.h.div_ceil(self.scale);
            let n = self.gw * self.gh;
            self.cells = vec![0; n];
            self.next = vec![0; n];
            self.fresh = vec![0.0; n];
            self.randomize();
        }
        if self.gw < 3 || self.gh < 3 {
            c.clear();
            return;
        }

        self.hue += dt * 0.01;
        self.age += dt;
        if self.age > LIFETIME {
            self.species = 3 + (self.species - 2) % 3;
            self.randomize();
        }
        if f.beat && self.rng.f32() < 0.5 {
            self.burst();
        }
        let decay = (-dt * 5.0).exp();
        for v in &mut self.fresh {
            *v *= decay;
        }
        self.acc = (self.acc + dt * (10.0 + f.level * 25.0 + f.bass * 15.0)).min(3.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            // A frozen board means one species won: start over.
            if self.step() < self.gw * self.gh / 2000 + 1 {
                self.randomize();
            }
        }

        let n = self.species as usize;
        let bins = f.spectrum.len();
        let palette: Vec<Rgb> = (0..n)
            .map(|k| {
                let band = if bins == 0 {
                    0.5
                } else {
                    f.spectrum[(k * bins / n + bins / (2 * n)).min(bins - 1)]
                };
                hsv(self.hue + k as f32 / n as f32, 0.85, 0.45 + 0.35 * band + 0.1 * f.pulse)
            })
            .collect();
        let s = self.scale;
        for y in 0..c.h {
            let row = (y / s) * self.gw;
            for x in 0..c.w {
                let i = row + x / s;
                let col = palette[self.cells[i] as usize % n];
                c.set(x, y, mix(col, [255, 255, 240], self.fresh[i] * 0.45));
            }
        }
    }
}
