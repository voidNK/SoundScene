use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

/// Kernel radius in grid cells.
const R: i32 = 6;
/// The growth center swings around `MU`, so the pattern keeps breathing;
/// the curve width is `SIGMA` times the center.
const MU: f32 = 0.138;
const SIGMA: f32 = 0.1;
/// Seconds before the world is reseeded even if it is still alive.
const LIFETIME: f32 = 45.0;

/// Lenia, a continuous cellular automaton: every cell weighs its
/// neighborhood with a smooth ring kernel and grows or decays along a bell
/// curve, which grows soft, coral-like worms and cells that wander, merge
/// and divide. Bass nudges the growth curve, loudness sets the speed, beats
/// drop new matter, growing edges glow and the colors follow the spectrum.
pub struct Lenia {
    a: Vec<f32>,
    pot: Vec<f32>,
    /// Smoothed recent change per cell, positive where matter grows.
    growth: Vec<f32>,
    colors: Vec<[f32; 3]>,
    /// (dx, dy, weight) for every kernel cell, weights summing to 1.
    kernel: Vec<(i32, i32, f32)>,
    gw: usize,
    gh: usize,
    size: (usize, usize),
    rng: Rng,
    acc: f32,
    age: f32,
    phase: f32,
    hue: f32,
}

impl Lenia {
    pub fn new() -> Self {
        let mut kernel = Vec::new();
        for dy in -R..=R {
            for dx in -R..=R {
                let r = ((dx * dx + dy * dy) as f32).sqrt() / R as f32;
                if r > 0.0 && r < 1.0 {
                    kernel.push((dx, dy, (4.0 - 1.0 / (r * (1.0 - r))).exp()));
                }
            }
        }
        let sum: f32 = kernel.iter().map(|k| k.2).sum();
        for k in &mut kernel {
            k.2 /= sum.max(1e-6);
        }
        Self {
            a: Vec::new(),
            pot: Vec::new(),
            growth: Vec::new(),
            colors: Vec::new(),
            kernel,
            gw: 0,
            gh: 0,
            size: (0, 0),
            rng: Rng::new(0x1E7A),
            acc: 0.0,
            age: 0.0,
            phase: 0.0,
            hue: 0.45,
        }
    }

    /// Drops a square of random matter at a random spot.
    fn blob(&mut self) {
        let (gw, gh) = (self.gw, self.gh);
        let s = (R as usize * 2).min(gw).min(gh);
        let x0 = self.rng.next_u32() as usize % gw;
        let y0 = self.rng.next_u32() as usize % gh;
        for dy in 0..s {
            for dx in 0..s {
                let i = (y0 + dy) % gh * gw + (x0 + dx) % gw;
                self.a[i] = self.rng.f32();
            }
        }
    }

    fn reseed(&mut self) {
        self.a.fill(0.0);
        let n = (self.gw * self.gh / 300).clamp(2, 14);
        for _ in 0..n {
            self.blob();
        }
        self.age = 0.0;
    }

    /// One Lenia update with time step `h` and growth center `mu` (the
    /// curve width scales with it); returns the total mass.
    fn step(&mut self, h: f32, mu: f32) -> f32 {
        let (gw, gh) = (self.gw, self.gh);
        self.pot.fill(0.0);
        // Accumulate the convolution one kernel offset at a time, so the
        // inner loop runs over contiguous row slices.
        for &(dx, dy, wt) in &self.kernel {
            let sx = dx.rem_euclid(gw as i32) as usize;
            for y in 0..gh {
                let sy = (y as i32 + dy).rem_euclid(gh as i32) as usize;
                let src = &self.a[sy * gw..(sy + 1) * gw];
                let dst = &mut self.pot[y * gw..(y + 1) * gw];
                let (d0, d1) = dst.split_at_mut(gw - sx);
                for (d, s) in d0.iter_mut().zip(&src[sx..]) {
                    *d += wt * s;
                }
                for (d, s) in d1.iter_mut().zip(&src[..sx]) {
                    *d += wt * s;
                }
            }
        }
        let sigma = mu * SIGMA;
        let k = 1.0 / (2.0 * sigma * sigma);
        let mut mass = 0.0;
        for ((v, &u), gr) in self.a.iter_mut().zip(&self.pot).zip(&mut self.growth) {
            let d = u - mu;
            let g = 2.0 * (-d * d * k).exp() - 1.0;
            let old = *v;
            *v = (*v + h * g).clamp(0.0, 1.0);
            *gr = *gr * 0.8 + (*v - old) * 2.0;
            mass += *v;
        }
        mass
    }
}

impl Effect for Lenia {
    fn name(&self) -> &'static str {
        "Lenia"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            // The world keeps roughly the same size in kernel units, so tiny
            // tiles show the same creatures, just coarser.
            self.gh = (c.h / 3).clamp(24, 40);
            self.gw = (self.gh * c.w / c.h.max(1)).clamp(24, 110);
            let n = self.gw * self.gh;
            self.a = vec![0.0; n];
            self.pot = vec![0.0; n];
            self.growth = vec![0.0; n];
            self.colors = vec![[0.0; 3]; n];
            self.reseed();
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        self.hue += dt * 0.01;
        self.age += dt;
        if self.age > LIFETIME {
            self.reseed();
        }
        if f.beat && self.rng.f32() < 0.25 {
            self.blob();
        }
        self.phase += dt * (0.25 + 0.3 * f.level);
        let mu = MU + 0.014 * self.phase.sin() + 0.006 * (f.bass - 0.4);
        self.acc = (self.acc + dt * (12.0 + 14.0 * f.level)).min(2.0);
        let cells = (self.gw * self.gh) as f32;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let mass = self.step(0.1, mu);
            // Died out or exploded into a uniform soup: start over.
            if mass < cells * 0.002 || mass > cells * 0.45 {
                self.reseed();
            }
        }

        let bins = f.spectrum.len();
        for (i, ((col, &v), &gr)) in self.colors.iter_mut().zip(&self.a).zip(&self.growth).enumerate() {
            let band = if bins == 0 {
                0.0
            } else {
                f.spectrum[(i % self.gw) * bins / self.gw]
            };
            let p = hsv(
                self.hue + 0.35 * v + 0.05 * band,
                0.9 - 0.55 * v * v,
                (v * (1.1 + 0.2 * f.pulse)).min(1.0).powf(0.8),
            );
            // Growing edges glow warm, shrinking ones cool.
            let (warm, cool) = ((gr * 1.5).clamp(0.0, 0.6), (-gr * 1.5).clamp(0.0, 0.5));
            *col = [
                p[0] as f32 / 255.0 + 0.02 + warm,
                p[1] as f32 / 255.0 + 0.02 + warm * 0.7,
                p[2] as f32 / 255.0 + 0.05 + cool,
            ];
        }
        upscale(c, &self.colors, self.gw, self.gh);
    }
}
