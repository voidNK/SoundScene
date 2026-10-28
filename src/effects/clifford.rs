use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

const POINTS_PER_FRAME: usize = 40_000;

/// Clifford attractor density plot: x' = sin(a·y) + c·cos(a·x),
/// y' = sin(b·x) + d·cos(b·y). Hits accumulate with a slow fade and are
/// shown with log-density coloring, hue by how far each step jumped. The
/// parameters drift faster with bass; beats flash and kick them along.
pub struct Clifford {
    density: Vec<f32>,
    travel: Vec<f32>,
    w: usize,
    h: usize,
    x: f32,
    y: f32,
    phase: f32,
    peak: f32,
    hue: f32,
    /// Share of a coarse grid hit last frame; low means a periodic orbit.
    coverage: f32,
}

impl Clifford {
    pub fn new() -> Self {
        Self {
            density: Vec::new(),
            travel: Vec::new(),
            w: 0,
            h: 0,
            x: 0.1,
            y: 0.1,
            phase: 0.0,
            peak: 1.0,
            coverage: 1.0,
            hue: 0.6,
        }
    }
}

impl Effect for Clifford {
    fn name(&self) -> &'static str {
        "Clifford"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if (self.w, self.h) != (c.w, c.h) {
            (self.w, self.h) = (c.w, c.h);
            self.density = vec![0.0; c.w * c.h];
            self.travel = vec![0.0; c.w * c.h];
        }
        let (w, h) = (self.w, self.h);
        if w == 0 || h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.phase += dt * (0.6 + 2.5 * f.bass);
        // Hurry through parameter sets that collapse onto a few points.
        if self.coverage < 0.15 {
            self.phase += dt * 10.0;
        }
        self.hue += dt * 0.015;
        if f.beat {
            self.phase += 0.25;
            self.hue += 0.03;
        }
        let p = self.phase;
        // Wander through a region that is chaotic almost everywhere.
        let a = -1.55 + 0.25 * (p * 0.031).sin();
        let b = 1.7 + 0.2 * (p * 0.043 + 1.0).sin();
        let cc = 1.4 + 0.25 * (p * 0.027 + 2.0).sin();
        let d = 1.0 + 0.3 * (p * 0.037 + 3.0).sin();

        let fade = (-dt * 2.0).exp();
        for v in &mut self.density {
            *v *= fade;
        }
        for v in &mut self.travel {
            *v *= fade;
        }

        // Fit the attractor's bounding box (|x| <= 1 + |c|, |y| <= 1 + |d|).
        let s = (w as f32 / (2.0 * (1.0 + cc.abs()))).min(h as f32 / (2.0 * (1.0 + d.abs()))) * 0.96;
        let (ox, oy) = (w as f32 * 0.5, h as f32 * 0.5);
        let (mut x, mut y) = (self.x, self.y);
        if !x.is_finite() || !y.is_finite() {
            (x, y) = (0.1, 0.1);
        }
        let mut coarse = [false; 32 * 32];
        for _ in 0..POINTS_PER_FRAME {
            let nx = (a * y).sin() + cc * (a * x).cos();
            let ny = (b * x).sin() + d * (b * y).cos();
            let jump = (nx - x).abs() + (ny - y).abs();
            (x, y) = (nx, ny);
            let px = ox + x * s;
            let py = oy + y * s;
            if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                let i = py as usize * w + px as usize;
                self.density[i] += 1.0;
                self.travel[i] += jump;
            }
            let gx = ((x / (2.0 * (1.0 + cc.abs())) + 0.5) * 31.0).clamp(0.0, 31.0) as usize;
            let gy = ((y / (2.0 * (1.0 + d.abs())) + 0.5) * 31.0).clamp(0.0, 31.0) as usize;
            coarse[gy * 32 + gx] = true;
        }
        (self.x, self.y) = (x, y);
        self.coverage = coarse.iter().filter(|&&b| b).count() as f32 / coarse.len() as f32;

        let max = self.density.iter().fold(1.0f32, |m, &v| m.max(v));
        self.peak += (max - self.peak) * 0.2;
        let norm = 1.0 / (1.0 + self.peak.max(1.0)).ln();
        let gain = 0.85 + 0.4 * f.pulse + 0.2 * f.level;
        for (i, (&dens, &trav)) in self.density.iter().zip(&self.travel).enumerate() {
            let col = if dens < 0.05 {
                [0, 0, 6]
            } else {
                let v = ((1.0 + dens).ln() * norm * gain).min(1.0);
                let avg = trav / dens;
                hsv(self.hue + avg * 0.18, 0.85 - 0.6 * v * v, v.powf(0.8))
            };
            c.set(i % w, i / w, col);
        }
    }
}
