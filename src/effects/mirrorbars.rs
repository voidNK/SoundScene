use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, add, hsv, scale};
use std::f32::consts::PI;

/// Linear interpolation into `v` at position `p` (0..1). Returns 0 for an
/// empty slice, so effects work with default (empty) `Features`.
pub(super) fn sample(v: &[f32], p: f32) -> f32 {
    let n = v.len();
    if n == 0 {
        return 0.0;
    }
    let x = p.clamp(0.0, 1.0) * (n - 1) as f32;
    let i0 = (x as usize).min(n - 1);
    let i1 = (i0 + 1).min(n - 1);
    v[i0] + (v[i1] - v[i0]) * (x - i0 as f32)
}

/// Gapless spectrum bars growing up from the center line, mirrored below
/// as a dimmer rippling reflection, with falling peak caps. Beats light
/// the center line and the background.
pub struct MirrorBars {
    levels: Vec<f32>,
    peaks: Vec<f32>,
}

impl MirrorBars {
    pub fn new() -> Self {
        Self {
            levels: Vec::new(),
            peaks: Vec::new(),
        }
    }
}

impl Effect for MirrorBars {
    fn name(&self) -> &'static str {
        "Mirror Bars"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fill(hsv(0.7, 0.6, 0.03 + 0.07 * f.pulse));
        let (w, h) = (c.w, c.h);
        if w == 0 || h < 2 {
            return;
        }
        let n = (w / 4).clamp(1, 48);
        if self.levels.len() != n {
            self.levels = vec![0.0; n];
            self.peaks = vec![0.0; n];
        }
        let last = n.saturating_sub(1).max(1) as f32;
        for (i, (l, p)) in self.levels.iter_mut().zip(&mut self.peaks).enumerate() {
            let v = sample(&f.spectrum, i as f32 / last * 0.95);
            *l = if v > *l { v } else { (*l - dt * 1.4).max(v) };
            *p = if *l > *p { *l } else { (*p - dt * 0.3).max(0.0) };
        }

        let mid = h / 2;
        let half = mid as f32;
        for i in 0..n {
            let (x0, x1) = (i * w / n, ((i + 1) * w / n).min(w));
            let bw = (x1 - x0).max(1) as f32;
            // A little idle sway so silence is not a flat line.
            let idle = 0.03 * (1.0 + (t * 1.7 + i as f32 * 0.5).sin());
            let top = ((self.levels[i] * 0.94 + idle) * half).min(half) as usize;
            let peak = ((self.peaks[i] * 0.94 + idle) * half).clamp(0.0, (half - 1.0).max(0.0)) as usize;
            let hue = 0.62 - i as f32 / n as f32 * 0.62 + t * 0.02;
            for x in x0..x1 {
                // Rounded shading across the bar keeps neighbours apart without gaps.
                let u = (x - x0) as f32 + 0.5;
                let shade = 0.55 + 0.45 * (u / bw * PI).sin();
                for k in 0..top {
                    let frac = k as f32 / half.max(1.0);
                    let col = hsv(hue + frac * 0.12, 0.85 - frac * 0.4, (0.35 + 0.65 * frac) * shade);
                    if mid > k {
                        c.set(x, mid - 1 - k, col);
                    }
                    let ry = mid + k;
                    if ry < h {
                        let ripple = 0.85 + 0.15 * (k as f32 * 0.9 - t * 7.0 + x as f32 * 0.2).sin();
                        let fadeout = 1.0 - k as f32 / (top as f32 + 1.0) * 0.7;
                        c.set(x, ry, scale(col, 0.5 * ripple * fadeout));
                    }
                }
                if mid > peak {
                    c.set(x, mid - 1 - peak, [235, 240, 255]);
                }
                if mid + peak < h {
                    c.set(x, mid + peak, [70, 75, 95]);
                }
            }
        }
        // Glowing center line.
        let line = hsv(0.6, 0.3 - 0.3 * f.pulse, 0.25 + 0.6 * f.pulse);
        for x in 0..w {
            let p = c.get(x, mid);
            c.set(x, mid, add(p, line));
        }
    }
}
