use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

/// Pixels per computed cell along each axis.
const CELL: usize = 2;
const WARMUP: usize = 24;
const ITER: usize = 64;
/// Growth sequences, `true` = use b. Switched every few beats.
const SEQS: [&[bool]; 5] = [
    &[false, true],
    &[false, false, true, true],
    &[false, false, true, false, true],
    &[
        true, true, true, true, true, true, false, false, false, false, false, false,
    ],
    &[false, true, true, false, true],
];

/// Lyapunov fractal of the logistic map with alternating growth rates a and
/// b: stable regions glow gold, chaos fades to deep blue. The view pans
/// slowly through (a, b) space; bass zooms, every eighth beat switches the
/// AB sequence and the pulse brightens the gold.
pub struct Lyapunov {
    colors: Vec<[f32; 3]>,
    pan: f32,
    hue: f32,
    beats: usize,
    seq: usize,
    bass: f32,
}

impl Lyapunov {
    pub fn new() -> Self {
        Self {
            colors: Vec::new(),
            pan: 0.0,
            hue: 0.0,
            beats: 0,
            seq: 2,
            bass: 0.0,
        }
    }
}

impl Effect for Lyapunov {
    fn name(&self) -> &'static str {
        "Lyapunov"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.pan += dt * (0.03 + 0.05 * f.mid);
        self.hue += dt * 0.01;
        self.bass += (f.bass - self.bass) * (1.0 - (-4.0 * dt).exp());
        if f.beat {
            self.beats += 1;
            if self.beats.is_multiple_of(8) {
                self.seq = (self.seq + 1) % SEQS.len();
            }
        }
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        if gw == 0 || gh == 0 {
            return;
        }
        self.colors.resize(gw * gh, [0.0; 3]);

        // Center wanders over the interesting part of [2, 4]².
        let ca = 3.25 + 0.45 * (self.pan * 0.9).sin();
        let cb = 3.25 + 0.45 * (self.pan * 0.67 + 1.0).cos();
        let span = (0.9 + 0.35 * (self.pan * 0.5).sin()) * (1.0 - 0.25 * self.bass);
        let step = span / gw.max(gh) as f32;
        let seq = SEQS[self.seq];
        let gold = 0.9 + 0.6 * f.pulse;

        for gy in 0..gh {
            let b = cb + (gh as f32 * 0.5 - gy as f32) * step;
            for gx in 0..gw {
                let a = ca + (gx as f32 - gw as f32 * 0.5) * step;
                let mut x = 0.5f32;
                let mut k = 0;
                for _ in 0..WARMUP {
                    let r = if seq[k] { b } else { a };
                    k = if k + 1 == seq.len() { 0 } else { k + 1 };
                    x = r * x * (1.0 - x);
                }
                // Multiply derivatives and take the log every 8 steps only.
                let (mut sum, mut prod) = (0.0f32, 1.0f32);
                for i in 0..ITER {
                    let r = if seq[k] { b } else { a };
                    k = if k + 1 == seq.len() { 0 } else { k + 1 };
                    x = r * x * (1.0 - x);
                    prod *= (r * (1.0 - 2.0 * x)).abs();
                    if i % 8 == 7 {
                        sum += prod.max(1e-30).ln();
                        prod = 1.0;
                    }
                }
                let l = sum / ITER as f32;
                let col = if l < 0.0 {
                    let v = (-l * 0.9).min(1.0);
                    hsv(self.hue + 0.12 - 0.08 * v, 0.95 - 0.5 * v * v, v.sqrt() * gold)
                } else {
                    let v = (l * 1.5).min(1.0);
                    hsv(self.hue + 0.62, 0.8, 0.35 * (1.0 - v) + 0.04)
                };
                self.colors[gy * gw + gx] = col.map(|v| v as f32 / 255.0);
            }
        }
        upscale(c, &self.colors, gw, gh);
    }
}
