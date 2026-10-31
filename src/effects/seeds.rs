use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, scale};

/// Seconds a pattern may run before it is replaced, even without beats.
const LIFETIME: f32 = 9.0;
/// Minimum seconds between beat reseeds, so a pattern can unfold first.
const MIN_AGE: f32 = 1.5;

/// Seeds (B2/S): every live cell dies, every dead cell with exactly two
/// live neighbors is born, so a few cells explode into fractal fireworks.
/// Beats (and the pattern reaching the edge) wipe the board and plant a
/// fresh mirror-symmetric seed, cells take the color of the generation they
/// were born in and fade out behind the front, and loudness sets the speed.
pub struct Seeds {
    cells: Vec<u8>,
    next: Vec<u8>,
    glow: Vec<f32>,
    hue: Vec<f32>,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    rng: Rng,
    acc: f32,
    age: f32,
    generation: u32,
    base_hue: f32,
}

impl Seeds {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            next: Vec::new(),
            glow: Vec::new(),
            hue: Vec::new(),
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            rng: Rng::new(0x5EED),
            acc: 0.0,
            age: 0.0,
            generation: 0,
            base_hue: 0.0,
        }
    }

    /// Clears the board and plants a random pattern mirrored on both axes
    /// (and sometimes the diagonal) around a point near the center.
    fn plant(&mut self) {
        self.cells.fill(0);
        let (gw, gh) = (self.gw as i32, self.gh as i32);
        let k = 2 + (self.rng.next_u32() % 3) as i32;
        let jitter = |rng: &mut Rng, n: i32| (rng.f32() - 0.5) * n as f32 * 0.3;
        let cx = gw / 2 + jitter(&mut self.rng, gw) as i32;
        let cy = gh / 2 + jitter(&mut self.rng, gh) as i32;
        let diagonal = self.rng.f32() < 0.5;
        for dy in 0..k {
            for dx in 0..k {
                if self.rng.f32() >= 0.45 {
                    continue;
                }
                let mirrors = if diagonal { 2 } else { 1 };
                for &(sx, sy) in [(dx, dy), (dy, dx)].iter().take(mirrors) {
                    for (x, y) in [
                        (cx + sx, cy + sy),
                        (cx - 1 - sx, cy + sy),
                        (cx + sx, cy - 1 - sy),
                        (cx - 1 - sx, cy - 1 - sy),
                    ] {
                        if x >= 0 && y >= 0 && x < gw && y < gh {
                            self.cells[(y * gw + x) as usize] = 1;
                        }
                    }
                }
            }
        }
        self.age = 0.0;
        self.base_hue = self.rng.f32();
        self.generation = 0;
    }

    /// Returns the number of live cells after the step.
    fn step(&mut self) -> usize {
        let (w, h) = (self.gw, self.gh);
        self.generation += 1;
        let hue = self.base_hue + self.generation as f32 * 0.035;
        let mut alive = 0;
        for y in 0..h {
            let up = y.checked_sub(1);
            let down = if y + 1 < h { Some(y + 1) } else { None };
            for x in 0..w {
                let i = y * w + x;
                let mut n = 0;
                if self.cells[i] == 0 {
                    let x0 = x.saturating_sub(1);
                    let x1 = (x + 1).min(w - 1);
                    for yy in [up, Some(y), down].into_iter().flatten() {
                        let row = &self.cells[yy * w + x0..=yy * w + x1];
                        n += row.iter().map(|&v| v as u32).sum::<u32>();
                    }
                }
                let born = n == 2;
                self.next[i] = born as u8;
                if born {
                    alive += 1;
                    self.glow[i] = 1.0;
                    self.hue[i] = hue;
                }
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        alive
    }

    /// True once the pattern reaches the edge of the board, where Seeds
    /// would soon turn into uniform noise.
    fn at_edge(&self) -> bool {
        let (w, h) = (self.gw, self.gh);
        let rows = self.cells[..w].iter().chain(&self.cells[(h - 1) * w..]);
        let cols = (0..h).flat_map(|y| [self.cells[y * w], self.cells[y * w + w - 1]]);
        rows.copied().chain(cols).any(|v| v != 0)
    }
}

impl Effect for Seeds {
    fn name(&self) -> &'static str {
        "Seeds"
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
            self.glow = vec![0.0; n];
            self.hue = vec![0.0; n];
            self.plant();
        }
        if self.gw < 3 || self.gh < 3 {
            c.clear();
            return;
        }

        self.age += dt;
        if (f.beat && self.age > MIN_AGE) || self.age > LIFETIME {
            self.plant();
        }
        let decay = (-dt * (1.2 + f.level)).exp();
        for g in &mut self.glow {
            *g *= decay;
        }
        self.acc = (self.acc + dt * (14.0 + f.level * 20.0)).min(3.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            if self.step() == 0 || self.at_edge() {
                self.plant();
            }
        }

        let s = self.scale;
        let bright = 0.85 + 0.15 * f.pulse;
        for y in 0..c.h {
            let row = (y / s) * self.gw;
            for x in 0..c.w {
                let i = row + x / s;
                let g = self.glow[i];
                let mut col = scale(hsv(self.hue[i], 0.8, bright), g.powf(1.5));
                if self.cells[i] != 0 {
                    col = mix(col, [255, 255, 255], 0.6);
                }
                c.set(x, y, col);
            }
        }
    }
}
