use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng};
use std::collections::VecDeque;

/// Samples per stored line profile.
const RES: usize = 96;
const MAX_LINES: usize = 64;
/// Seconds between two snapshots.
const STEP: f32 = 0.07;

/// The Joy Division "Unknown Pleasures" cover: a stack of spectrum
/// snapshots receding upwards, white on black, each line hiding the ones
/// behind it. Low frequencies form the tall central peaks, beats brighten
/// the newest line.
pub struct UnknownPleasures {
    lines: VecDeque<(Vec<f32>, f32)>,
    acc: f32,
    rng: Rng,
    top: Vec<i32>,
}

impl UnknownPleasures {
    pub fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            acc: 0.0,
            rng: Rng::new(0x10D1),
            top: Vec::new(),
        }
    }

    fn snapshot(&mut self, f: &Features) -> Vec<f32> {
        let bins = f.spectrum.len();
        // A few random bumps per line so quiet passages still look organic.
        let bumps: Vec<(f32, f32)> = (0..4)
            .map(|_| (self.rng.range(0.25, 0.75), self.rng.range(0.0, 0.25)))
            .collect();
        (0..RES)
            .map(|i| {
                let p = i as f32 / (RES - 1) as f32;
                let d = ((p - 0.5).abs() * 2.0).min(1.0);
                let env = (1.0 - d * d * 1.6).max(0.0).powi(2);
                let spec = if bins == 0 {
                    0.0
                } else {
                    // Mirrored: bass in the middle, highs towards the edges.
                    let b = d * 0.7 * (bins - 1) as f32;
                    let (i0, fr) = (b as usize, b.fract());
                    let i1 = (i0 + 1).min(bins - 1);
                    f.spectrum[i0.min(bins - 1)] * (1.0 - fr) + f.spectrum[i1] * fr
                };
                let bump: f32 = bumps.iter().map(|&(c, a)| a * (-((p - c) * 30.0).powi(2)).exp()).sum();
                let noise = self.rng.range(0.0, 0.04);
                env * (spec * spec * 1.2 + bump) + noise * (0.3 + env)
            })
            .collect()
    }
}

impl Effect for UnknownPleasures {
    fn name(&self) -> &'static str {
        "Unknown Pleasures"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.acc += dt;
        if self.lines.is_empty() || self.acc >= STEP {
            self.acc = (self.acc - STEP).clamp(0.0, STEP);
            let line = self.snapshot(f);
            self.lines.push_front((line, f.pulse));
            self.lines.truncate(MAX_LINES);
        }
        c.clear();
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        let (wf, hf) = (w as f32, h as f32);
        let n = ((hf / 3.5) as usize).clamp(1, MAX_LINES).min(self.lines.len());
        let spacing = hf * 0.62 / n as f32;
        let amp = spacing * 7.0;
        let (x0, x1) = (wf * 0.2, wf * 0.8);
        let bottom = hf * 0.9;
        // Smooth scroll: lines move up continuously between snapshots.
        let shift = (self.acc / STEP).clamp(0.0, 1.0) * spacing;

        self.top.clear();
        self.top.resize(w, i32::MAX);
        let mut prev = vec![0i32; w];
        // Front to back: each line only draws above everything drawn before.
        for (k, (line, pulse)) in self.lines.iter().take(n).enumerate() {
            let base = bottom - k as f32 * spacing - shift;
            let fade = 1.0 - k as f32 / n as f32 * 0.5;
            let v = (fade * (0.85 + 0.15 * pulse) * 255.0) as u8;
            let col = [v, v, v];
            for (x, p) in prev.iter_mut().enumerate() {
                let xf = x as f32;
                let y = if xf < x0 || xf > x1 || x1 <= x0 {
                    base
                } else {
                    let s = (xf - x0) / (x1 - x0) * (RES - 1) as f32;
                    let (i0, fr) = (s as usize, s.fract());
                    let a = line[i0.min(RES - 1)];
                    let b = line[(i0 + 1).min(RES - 1)];
                    base - (a * (1.0 - fr) + b * fr) * amp
                };
                *p = y.round() as i32;
            }
            for x in 0..w {
                let y = prev[x];
                // Connect to the neighbors so steep slopes have no gaps.
                let lo = prev[x.saturating_sub(1)].min(prev[(x + 1).min(w - 1)]);
                let (ya, yb) = (y.min((lo + y) / 2), y);
                let limit = self.top[x];
                for yy in ya.max(0)..=yb.min(h as i32 - 1) {
                    if yy < limit {
                        c.set(x, yy as usize, col);
                    }
                }
                self.top[x] = limit.min(ya);
            }
        }
    }
}
