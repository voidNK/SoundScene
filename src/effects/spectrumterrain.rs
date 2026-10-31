use super::Effect;
use super::mirrorbars::sample;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, mix, scale};
use std::collections::VecDeque;

/// Height samples per terrain row.
const COLS: usize = 64;
const ROWS: usize = 40;
/// Seconds between two spectrum snapshots.
const STEP: f32 = 0.06;
const Z_NEAR: f32 = 1.0;
const DZ: f32 = 0.16;
/// World half-width of a row.
const X_HALF: f32 = 2.6;
/// Camera height above the ground plane.
const CAM: f32 = 1.6;

/// Spectrum history as a shaded mountain range that rolls in from the
/// horizon towards the viewer: bass forms the central peaks, highs the
/// foothills at the sides. Beats push the newest ridge higher.
pub struct SpectrumTerrain {
    rows: VecDeque<Vec<f32>>,
    acc: f32,
    phase: f32,
    ybuf: Vec<i32>,
    pts: Vec<(f32, f32)>,
}

impl SpectrumTerrain {
    pub fn new() -> Self {
        Self {
            rows: VecDeque::new(),
            acc: 0.0,
            phase: 0.0,
            ybuf: Vec::new(),
            pts: Vec::with_capacity(COLS),
        }
    }

    fn snapshot(&self, f: &Features) -> Vec<f32> {
        let punch = 1.0 + 0.35 * f.pulse;
        (0..COLS)
            .map(|i| {
                let p = i as f32 / (COLS - 1) as f32;
                let d = ((p - 0.5).abs() * 2.0).min(1.0);
                let spec = sample(&f.spectrum, d * 0.85);
                // Rolling hills keep the landscape alive in silence.
                let hill = 0.07 * (2.0 + (p * 9.0 + self.phase).sin() + 0.6 * (p * 23.0 - self.phase * 1.3).sin());
                let env = 1.0 - d * d * 0.55;
                (spec * spec * 0.9 * punch + hill) * env
            })
            .collect()
    }
}

impl Effect for SpectrumTerrain {
    fn name(&self) -> &'static str {
        "Spectrum Terrain"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.phase += dt * 0.7;
        self.acc += dt;
        if self.rows.is_empty() {
            let row = self.snapshot(f);
            self.rows.resize(ROWS + 1, row);
        }
        if self.acc >= STEP {
            self.acc = (self.acc - STEP).min(STEP);
            let mut row = self.snapshot(f);
            // Blend with the previous ridge so the surface stays smooth.
            if let Some(prev) = self.rows.front() {
                for (v, p) in row.iter_mut().zip(prev) {
                    *v = *v * 0.6 + p * 0.4;
                }
            }
            self.rows.push_front(row);
            self.rows.truncate(ROWS + 1);
        }
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        let (wf, hf) = (w as f32, h as f32);
        let horizon = hf * 0.28;
        let fy = (hf - horizon) / CAM;
        let fx = wf * 0.5;
        let cx = wf * 0.5;

        // Sky and ground.
        let sky_top = hsv(0.72, 0.8, 0.06);
        let sky_low = hsv(0.88 + 0.03 * (t * 0.2).sin(), 0.7, 0.3 + 0.25 * f.pulse);
        let fog = hsv(0.85, 0.6, 0.22 + 0.15 * f.pulse);
        for y in 0..h {
            let yf = y as f32;
            let col = if yf < horizon {
                mix(sky_top, sky_low, (yf / horizon.max(1.0)).powi(2))
            } else {
                scale(fog, (1.0 - (yf - horizon) / (hf - horizon).max(1.0)).powi(3))
            };
            for x in 0..w {
                c.set(x, y, col);
            }
        }

        // Front-to-back with a per-column horizon: each row only paints
        // above everything nearer to the viewer.
        self.ybuf.clear();
        self.ybuf.resize(w, h as i32);
        let frac = self.acc / STEP;
        let z_far = Z_NEAR + ROWS as f32 * DZ;
        for i in (0..self.rows.len()).rev() {
            let z = Z_NEAR + (ROWS as f32 - 1.0 - (i as f32 + frac)) * DZ;
            if z < Z_NEAR * 0.85 {
                continue;
            }
            let fogk = ((z - Z_NEAR) / (z_far - Z_NEAR)).clamp(0.0, 1.0);
            let row = &self.rows[i];
            let nearer = if i + 1 < self.rows.len() {
                &self.rows[i + 1]
            } else {
                row
            };
            self.pts.clear();
            // Rows flatten out as they reach the viewer, so no cliff fills the foreground.
            let rise = ((z - Z_NEAR * 0.85) / 0.7).clamp(0.0, 1.0);
            for (j, &v) in row.iter().enumerate() {
                let v = v * rise;
                let xw = (j as f32 / (COLS - 1) as f32 * 2.0 - 1.0) * X_HALF;
                let sx = cx + xw / z * fx;
                let sy = horizon + (CAM - v) / z * fy;
                self.pts.push((sx, sy));
            }
            for j in 0..COLS - 1 {
                let ((xa, ya), (xb, yb)) = (self.pts[j], self.pts[j + 1]);
                if xb <= xa {
                    continue;
                }
                let hv = (row[j] + row[j + 1]) * 0.5;
                // Light from behind the viewer: slopes facing us are bright.
                let facing = (row[j] - nearer[j]) * 4.0 + (row[j + 1] - row[j]) * 3.0;
                let shade = (0.45 + facing).clamp(0.12, 1.0);
                let base = hsv(0.75 - hv * 0.65, 0.75 - hv * 0.3, shade);
                let col = mix(base, fog, fogk * 0.9);
                let ridge = mix(hsv(0.5 - hv * 0.4, 0.4, 1.0), fog, fogk);
                let x_start = xa.ceil().max(0.0) as usize;
                let x_end = (xb.ceil().max(0.0) as usize).min(w);
                for x in x_start..x_end {
                    let y = ya + (yb - ya) * (x as f32 - xa) / (xb - xa);
                    let yi = (y as i32).max(0);
                    let lim = self.ybuf[x];
                    if yi >= lim {
                        continue;
                    }
                    c.set(x, yi as usize, ridge);
                    for yy in yi + 1..lim {
                        let k = 1.0 - (yy - yi) as f32 / (hf * 0.4).max(1.0);
                        c.set(x, yy as usize, scale(col, k.max(0.35)));
                    }
                    self.ybuf[x] = yi;
                }
            }
        }
    }
}
