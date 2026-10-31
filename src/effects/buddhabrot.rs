use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, rgb};

/// Orbits escaping before LIMITS[0] are short, before LIMITS[1] medium,
/// up to LIMITS[2] long (nebula style).
const LIMITS: [usize; 3] = [24, 90, 260];
/// Orbits shorter than this only add a uniform haze.
const MIN_ITER: usize = 6;
/// Iterations plus plotted orbit points allowed per frame.
const BUDGET: usize = 220_000;

/// Buddhabrot: the density of all escaping Mandelbrot orbits, accumulated
/// a little more every frame and slowly fading, colored as a nebula from
/// short, medium and long orbits. The starting point z₀ drifts along the
/// real axis so the figure keeps evolving; loudness widens that drift,
/// bass speeds it up and beats flare the brightness.
pub struct Buddhabrot {
    dens: Vec<[f32; 3]>,
    size: (usize, usize),
    orbit: Vec<(f32, f32)>,
    rng: Rng,
    drift: f32,
    mean: [f32; 3],
    hue: f32,
}

impl Buddhabrot {
    pub fn new() -> Self {
        Self {
            dens: Vec::new(),
            size: (0, 0),
            orbit: Vec::with_capacity(LIMITS[2]),
            rng: Rng::new(0xB0DD),
            drift: 0.0,
            mean: [1.0; 3],
            hue: 0.0,
        }
    }
}

/// Main cardioid and period-2 bulb never escape, skip them.
fn inside(cr: f32, ci: f32) -> bool {
    let q = (cr - 0.25) * (cr - 0.25) + ci * ci;
    q * (q + cr - 0.25) < 0.25 * ci * ci || (cr + 1.0) * (cr + 1.0) + ci * ci < 0.0625
}

impl Effect for Buddhabrot {
    fn name(&self) -> &'static str {
        "Buddhabrot"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.drift += dt * (0.05 + 0.1 * f.bass);
        self.hue += dt * 0.01;
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.dens = vec![[0.0; 3]; c.w * c.h];
            self.mean = [1.0; 3];
        }
        if c.w == 0 || c.h == 0 {
            return;
        }
        let keep = 0.5f32.powf(dt / 5.0);
        for d in &mut self.dens {
            *d = d.map(|v| v * keep);
        }

        // Real axis horizontal; the image is symmetric, so each point is
        // plotted twice, mirrored around the middle row.
        let (w, h) = (c.w as f32, c.h as f32);
        let scale = (w / 3.0).min(h / 2.6);
        let z0 = (0.12 + 0.2 * f.level) * self.drift.sin();
        let mut spent = 0;
        while spent < BUDGET {
            let (cr, ci) = (self.rng.range(-2.0, 2.0), self.rng.range(0.0, 2.0));
            spent += 1;
            if inside(cr, ci) {
                continue;
            }
            self.orbit.clear();
            let (mut zr, mut zi) = (z0, 0.0f32);
            let mut escaped = false;
            for _ in 0..LIMITS[2] {
                (zr, zi) = (zr * zr - zi * zi + cr, 2.0 * zr * zi + ci);
                if zr * zr + zi * zi > 4.0 {
                    escaped = true;
                    break;
                }
                self.orbit.push((zr, zi));
            }
            spent += self.orbit.len();
            if !escaped || self.orbit.len() < MIN_ITER {
                continue;
            }
            let n = self.orbit.len();
            // Each orbit feeds one layer: long, medium or short.
            let layer = 2 - LIMITS[..2].iter().filter(|&&l| n >= l).count();
            for &(zr, zi) in &self.orbit {
                let x = w * 0.5 + (zr + 0.5) * scale;
                let dy = zi * scale;
                if x < 0.0 || x >= w {
                    continue;
                }
                for y in [h * 0.5 + dy, h * 0.5 - dy] {
                    if y >= 0.0 && y < h {
                        self.dens[y as usize * c.w + x as usize][layer] += 1.0;
                    }
                }
            }
            spent += n;
        }

        // Normalize each layer by its slowly following mean over lit pixels,
        // then square the contrast so the halo stays dark.
        let (mut sum, mut lit) = ([0.0f32; 3], [0usize; 3]);
        for d in &self.dens {
            for k in 0..3 {
                if d[k] > 0.0 {
                    sum[k] += d[k];
                    lit[k] += 1;
                }
            }
        }
        for k in 0..3 {
            let mean = (sum[k] / lit[k].max(1) as f32).max(1e-3);
            self.mean[k] += (mean - self.mean[k]) * 0.2;
        }
        let gain = 1.0 + 0.6 * f.pulse;
        let inv = self.mean.map(|m| 1.0 / (5.0 * m));
        let tone = |d: f32, k: usize| {
            let x = d * inv[k];
            (1.0 - (-x * x).exp()) * gain
        };
        let shift = (self.hue * std::f32::consts::TAU).sin() * 0.15;
        for (p, d) in c.px.iter_mut().zip(&self.dens) {
            let (long, mid, short) = (tone(d[0], 0), tone(d[1], 1), tone(d[2], 2));
            *p = rgb(
                (0.85 + shift) * long + 0.1 * mid + 0.25 * short,
                0.45 * long + 0.55 * mid + 0.1 * short,
                0.2 * long + 0.5 * mid + (0.8 - shift) * short,
            );
        }
    }
}
