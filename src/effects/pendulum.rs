use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, add, hsv, mix, scale};
use std::f32::consts::TAU;

const COUNT: usize = 18;
/// Pendulum `i` completes `CYCLES + i` swings per `PERIOD` seconds of
/// pattern time, so the row drifts through waves and realigns.
const CYCLES: f32 = 20.0;
const PERIOD: f32 = 45.0;
const DIST: f32 = 5.0;

/// Pendulum wave seen from above at an angle: a row of pendulums with
/// slightly different periods sweeps through snakes, braids and chaos and
/// back into line. Loudness speeds up the pattern, bass widens the swing,
/// and each ball glows with its own spectrum band.
pub struct PendulumWave {
    time: f32,
    swing: f32,
    glow: [f32; COUNT],
}

impl PendulumWave {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            swing: 0.4,
            glow: [0.0; COUNT],
        }
    }
}

impl Effect for PendulumWave {
    fn name(&self) -> &'static str {
        "Pendulum Wave"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.time += dt * (0.6 + 1.0 * f.level + 0.6 * f.pulse);
        let target = 0.35 + 0.4 * f.bass;
        self.swing += (target - self.swing) * (1.0 - (-3.0 * dt).exp());
        let bins = f.spectrum.len();
        for (i, g) in self.glow.iter_mut().enumerate() {
            let v = if bins == 0 {
                0.0
            } else {
                f.spectrum[(i * bins / COUNT).min(bins - 1)]
            };
            *g += (v - *g) * (1.0 - (-12.0 * dt).exp());
        }

        c.fade(0.6);
        let (w, h) = (c.w as f32, c.h as f32);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (cx, cy) = (w * 0.5, h * 0.5);
        let fpx = (w / 4.2).min(h / 2.1) * DIST;
        let (sb, cb) = 0.45f32.sin_cos();
        let (sa, ca) = 0.5f32.sin_cos();
        // World (x along the row, y up, z towards the viewer) to screen and depth.
        let project = |x: f32, y: f32, z: f32| -> (f32, f32, f32) {
            let y = y + 0.7;
            let (x1, z1) = (x * cb + z * sb, -x * sb + z * cb);
            let y2 = y * ca - z1 * sa;
            let depth = (DIST - (z1 * ca + y * sa)).max(0.5);
            (cx + x1 * fpx / depth, cy - y2 * fpx / depth, depth)
        };

        let pivot = |i: usize| (i as f32 / (COUNT - 1) as f32 * 3.2 - 1.6, 0.0, 0.0);
        let (a0, a1) = (project(-1.75, 0.02, 0.0), project(1.75, 0.02, 0.0));
        for k in 0..2 {
            let o = k as f32;
            c.line_add(a0.0, a0.1 - o, a1.0, a1.1 - o, [70, 75, 85]);
        }

        let mut balls: Vec<(f32, f32, f32, usize)> = Vec::with_capacity(COUNT);
        for i in 0..COUNT {
            let (px, py, pz) = pivot(i);
            let len = 1.35 - 0.45 * i as f32 / (COUNT - 1) as f32;
            let omega = TAU * (CYCLES + i as f32) / PERIOD;
            let theta = self.swing * (omega * self.time).cos();
            let (bx, by, bz) = (px, py - len * theta.cos(), pz + len * theta.sin());
            let p = project(px, py, pz);
            let b = project(bx, by, bz);
            c.line_add(p.0, p.1, b.0, b.1, [45, 45, 55]);
            balls.push((b.0, b.1, b.2, i));
        }
        balls.sort_by(|a, b| b.2.total_cmp(&a.2));

        for (bx, by, depth, i) in balls {
            let g = self.glow[i];
            let r = (0.075 + 0.025 * g) * fpx / depth;
            let hue = i as f32 / COUNT as f32 * 0.75 + t * 0.03;
            let base = hsv(hue, 0.75, 1.0);
            let halo = r * (2.0 + 1.5 * g + f.pulse);
            let (x0, x1) = (
                (bx - halo).max(0.0) as usize,
                ((bx + halo).max(0.0) as usize + 1).min(c.w),
            );
            let (y0, y1) = (
                (by - halo).max(0.0) as usize,
                ((by + halo).max(0.0) as usize + 1).min(c.h),
            );
            for y in y0..y1 {
                for x in x0..x1 {
                    let (dx, dy) = (x as f32 + 0.5 - bx, y as f32 + 0.5 - by);
                    let d = (dx * dx + dy * dy).sqrt();
                    let i = y * c.w + x;
                    if d < r {
                        // Lit from the upper left, with a specular spot.
                        let (nx, ny) = (dx / r.max(0.01), dy / r.max(0.01));
                        let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
                        let light = (-0.45 * nx - 0.55 * ny + 0.7 * nz).max(0.0);
                        let spec = light.powi(12);
                        let col = scale(base, 0.15 + 0.85 * light);
                        c.px[i] = mix(col, [255, 255, 255], spec * 0.8);
                    } else if d < halo {
                        let k = (1.0 - (d - r) / (halo - r).max(0.01)).powi(2);
                        c.px[i] = add(c.px[i], scale(base, k * (0.15 + 0.6 * g)));
                    }
                }
            }
        }
    }
}
