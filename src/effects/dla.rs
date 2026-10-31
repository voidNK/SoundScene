use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, add, hsv, scale};
use std::f32::consts::TAU;

const WALKERS: usize = 400;
/// Random-walk steps per second, split over all walkers.
const STEPS_PER_SEC: f32 = 1_500_000.0;
const MAX_STEPS: usize = 120_000;
/// Seconds before a cluster starts over even if it never filled the screen.
const LIFETIME: f32 = 45.0;

/// Diffusion-limited aggregation: random walkers stick to a crystal growing
/// from the center, colored by arrival order, newest branches glowing.
/// Bass speeds up the growth, beats flash the fresh tips; the crystal
/// dissolves and starts over when it reaches the edge or gets old.
pub struct Dla {
    /// Arrival index per pixel, 0 = empty.
    grid: Vec<u32>,
    w: usize,
    h: usize,
    walkers: Vec<(i32, i32)>,
    count: u32,
    radius: f32,
    age: f32,
    /// Fade-out progress after the cluster is done, 0 = growing.
    dissolve: f32,
    rng: Rng,
    hue: f32,
}

impl Dla {
    pub fn new() -> Self {
        Self {
            grid: Vec::new(),
            w: 0,
            h: 0,
            walkers: Vec::new(),
            count: 0,
            radius: 1.0,
            age: 0.0,
            dissolve: 0.0,
            rng: Rng::new(0xD1A),
            hue: 0.0,
        }
    }

    fn reset(&mut self) {
        self.grid.fill(0);
        self.count = 1;
        self.radius = 1.0;
        self.age = 0.0;
        self.dissolve = 0.0;
        self.hue = self.rng.f32();
        if self.w > 0 && self.h > 0 {
            self.grid[self.h / 2 * self.w + self.w / 2] = 1;
        }
        self.walkers.clear();
    }

    /// A walker on a circle just outside the cluster.
    fn spawn(&mut self) -> (i32, i32) {
        let a = self.rng.range(0.0, TAU);
        let r = self.radius + 3.0;
        (
            (self.w / 2) as i32 + (a.cos() * r) as i32,
            (self.h / 2) as i32 + (a.sin() * r) as i32,
        )
    }

    fn occupied(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.w
            && (y as usize) < self.h
            && self.grid[y as usize * self.w + x as usize] != 0
    }
}

impl Effect for Dla {
    fn name(&self) -> &'static str {
        "DLA"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if (self.w, self.h) != (c.w, c.h) {
            (self.w, self.h) = (c.w, c.h);
            self.grid = vec![0; c.w * c.h];
            self.reset();
        }
        let (w, h) = (self.w as i32, self.h as i32);
        let (cx, cy) = ((self.w / 2) as f32, (self.h / 2) as f32);
        // Largest radius that still fits in the canvas.
        let limit = (cx.min(cy) - 1.0).max(1.0);
        self.age += dt;
        self.hue += dt * 0.01;

        if self.dissolve > 0.0 {
            self.dissolve += dt / 1.5;
            if self.dissolve >= 1.0 {
                self.reset();
            }
        } else if self.radius >= limit || self.age > LIFETIME {
            self.dissolve = 1e-3;
        } else {
            let walkers = WALKERS.min(self.w * self.h / 4 + 1).min(self.radius as usize * 8 + 16);
            while self.walkers.len() < walkers {
                let p = self.spawn();
                self.walkers.push(p);
            }
            let budget = (dt * STEPS_PER_SEC * (0.35 + 1.0 * f.bass + 1.0 * f.pulse)) as usize;
            let per = budget.min(MAX_STEPS) / self.walkers.len().max(1);
            let kill = self.radius * 2.0 + 12.0;
            for k in 0..self.walkers.len() {
                let (mut x, mut y) = self.walkers[k];
                for _ in 0..per {
                    match self.rng.next_u32() & 3 {
                        0 => x += 1,
                        1 => x -= 1,
                        2 => y += 1,
                        _ => y -= 1,
                    }
                    let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                    if dx * dx + dy * dy > kill * kill || x < 0 || y < 0 || x >= w || y >= h {
                        (x, y) = self.spawn();
                        continue;
                    }
                    let touching = self.occupied(x + 1, y)
                        || self.occupied(x - 1, y)
                        || self.occupied(x, y + 1)
                        || self.occupied(x, y - 1);
                    if touching && !self.occupied(x, y) {
                        self.count += 1;
                        self.grid[y as usize * self.w + x as usize] = self.count;
                        self.radius = self.radius.max((dx * dx + dy * dy).sqrt());
                        (x, y) = self.spawn();
                    }
                }
                self.walkers[k] = (x, y);
            }
        }

        // Older particles drift through the hue range, the newest glow white-hot.
        let total = self.count.max(1) as f32;
        let fade = (1.0 - self.dissolve).clamp(0.0, 1.0);
        let glow_n = (total * 0.04).max(30.0);
        let bg = hsv(self.hue + 0.6, 0.6, 0.04 + 0.04 * f.level);
        for (p, &n) in c.px.iter_mut().zip(&self.grid) {
            *p = if n == 0 {
                bg
            } else {
                let k = n as f32 / total;
                let recent = (1.0 - (total - n as f32) / glow_n).max(0.0);
                let v = (0.55 + 0.35 * k + recent * (0.3 + 0.6 * f.pulse)).min(1.0) * fade;
                hsv(self.hue + k * 0.6, 0.85 - 0.6 * recent, v)
            };
        }
        if self.dissolve == 0.0 {
            let dot = scale(hsv(self.hue + 0.3, 0.3, 1.0), 0.25 + 0.3 * f.treble);
            for &(x, y) in &self.walkers {
                if x >= 0 && y >= 0 && x < w && y < h {
                    let i = y as usize * self.w + x as usize;
                    c.px[i] = add(c.px[i], dot);
                }
            }
        }
    }
}
