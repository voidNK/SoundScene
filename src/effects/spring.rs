use super::Effect;
use super::donut::{ZBuf, apply, backdrop, dot, norm};
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};
use std::f32::consts::TAU;

type V3 = [f32; 3];

const CAM: f32 = 7.0;
const TURNS: f32 = 9.0;
const COIL_R: f32 = 0.65;
const WIRE_R: f32 = 0.09;
const BALL_R: f32 = 0.42;
/// Y of the fixed top end (y grows downwards, like the screen).
const TOP: f32 = -1.5;

/// Rotation about the vertical axis by `yaw`, then a tilt about x.
fn view(yaw: f32, tilt: f32) -> [V3; 3] {
    let (sy, cy) = yaw.sin_cos();
    let (st, ct) = tilt.sin_cos();
    // Rx(tilt) · Ry(yaw)
    [[cy, 0.0, sy], [st * sy, ct, -st * cy], [-ct * sy, st, ct * cy]]
}

/// A metallic coil spring hanging from a bar with a weight at its end,
/// z-buffered and lit. Its length is a damped spring simulation pulled
/// shorter by bass and kicked on beats; the spectrum glows along the coil.
pub struct Spring {
    yaw: f32,
    len: f32,
    vel: f32,
    hue: f32,
    zb: ZBuf,
}

impl Spring {
    pub fn new() -> Self {
        Self {
            yaw: 0.0,
            len: 2.4,
            vel: 0.0,
            hue: 0.08,
            zb: ZBuf::new(),
        }
    }
}

impl Effect for Spring {
    fn name(&self) -> &'static str {
        "Spring"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.yaw += dt * (0.4 + 0.8 * f.mid);
        self.hue += dt * 0.02;
        // Damped spring towards a bass-dependent rest length, plus a gentle
        // idle bob so it never stands still.
        let target = 2.6 - 1.4 * f.bass + 0.15 * (t * 1.3).sin();
        let dt_sim = dt.min(0.05);
        self.vel += ((target - self.len) * 40.0 - self.vel * 4.0) * dt_sim;
        if f.beat {
            self.vel -= 3.5;
        }
        self.len = (self.len + self.vel * dt_sim).clamp(0.7, 3.4);
        if self.len <= 0.7 || self.len >= 3.4 {
            self.vel *= -0.3;
        }

        backdrop(c, [0.03, 0.03, 0.05], [0.08, 0.06, 0.05]);
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        self.zb.reset(w, h);
        let focal = (h as f32).min(w as f32 * 0.8) * 1.7;
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let m = view(self.yaw, 0.3);
        let light = norm([-0.5, -0.6, -0.6]);
        let half = norm([light[0], light[1], light[2] - 1.0]);
        let len = self.len;

        // Coil: walk along the helix and splat a ring of tube points.
        let ppu = focal / (CAM - 1.0);
        let path_len = TURNS * TAU * COIL_R;
        let n_s = (path_len * ppu * 1.3).clamp(40.0, 4000.0) as usize;
        let n_r = (TAU * WIRE_R * ppu * 1.8).clamp(4.0, 64.0) as usize;
        let pitch = len / (TURNS * TAU);
        let tlen = (COIL_R * COIL_R + pitch * pitch).sqrt();
        for i in 0..=n_s {
            let s = i as f32 / n_s as f32;
            let a = s * TURNS * TAU;
            let (sa, ca) = a.sin_cos();
            let center = [COIL_R * ca, TOP + s * len, COIL_R * sa];
            let tan = [-COIL_R * sa / tlen, pitch / tlen, COIL_R * ca / tlen];
            let nrm = [-ca, 0.0, -sa];
            let bin = [
                tan[1] * nrm[2] - tan[2] * nrm[1],
                tan[2] * nrm[0] - tan[0] * nrm[2],
                tan[0] * nrm[1] - tan[1] * nrm[0],
            ];
            let lvl = super::donut::band_mirrored(&f.spectrum, s * 0.5);
            let base = hsv(self.hue + s * 0.25, 0.55 + 0.35 * lvl, 1.0).map(|x| x as f32 / 255.0);
            let glow = 0.45 + 0.55 * lvl;
            for k in 0..n_r {
                let (sb, cb) = (k as f32 / n_r as f32 * TAU).sin_cos();
                let n = [
                    nrm[0] * cb + bin[0] * sb,
                    nrm[1] * cb + bin[1] * sb,
                    nrm[2] * cb + bin[2] * sb,
                ];
                let p = [
                    center[0] + n[0] * WIRE_R,
                    center[1] + n[1] * WIRE_R,
                    center[2] + n[2] * WIRE_R,
                ];
                let q = apply(&m, p);
                let z = q[2] + CAM;
                if z < 0.2 {
                    continue;
                }
                let inv = focal / z;
                let Some(idx) = self.zb.test(cx + q[0] * inv, cy + q[1] * inv, z) else {
                    continue;
                };
                let nr = apply(&m, n);
                let diff = dot(nr, light).max(0.0);
                let spec = dot(nr, half).max(0.0).powi(30);
                let kd = glow * (0.12 + 0.88 * diff);
                c.px[idx] = rgb(base[0] * kd + spec, base[1] * kd + spec, base[2] * kd + spec);
            }
        }

        // The weight: an analytic sphere per pixel, depth-tested against the coil.
        let bc = apply(&m, [0.0, TOP + len + BALL_R * 0.9, 0.0]);
        let bz = bc[2] + CAM;
        if bz > BALL_R + 0.2 {
            let inv = focal / bz;
            let (px, py, pr) = (cx + bc[0] * inv, cy + bc[1] * inv, BALL_R * inv);
            let ball = hsv(self.hue + 0.5, 0.6, 1.0).map(|x| x as f32 / 255.0);
            let x0 = (px - pr).floor().max(0.0) as usize;
            let y0 = (py - pr).floor().max(0.0) as usize;
            let x1 = ((px + pr).ceil().max(0.0) as usize).min(w);
            let y1 = ((py + pr).ceil().max(0.0) as usize).min(h);
            for y in y0..y1 {
                for x in x0..x1 {
                    let dx = (x as f32 + 0.5 - px) / pr.max(1e-3);
                    let dy = (y as f32 + 0.5 - py) / pr.max(1e-3);
                    let d2 = dx * dx + dy * dy;
                    if d2 > 1.0 {
                        continue;
                    }
                    let nz = -(1.0 - d2).sqrt();
                    let Some(idx) = self.zb.test(x as f32 + 0.5, y as f32 + 0.5, bz + nz * BALL_R) else {
                        continue;
                    };
                    let n = [dx, dy, nz];
                    let diff = dot(n, light).max(0.0);
                    let spec = dot(n, half).max(0.0).powi(40);
                    let k = 0.1 + 0.9 * diff + 0.4 * f.pulse;
                    c.px[idx] = rgb(ball[0] * k + spec, ball[1] * k + spec, ball[2] * k + spec);
                }
            }
        }

        // The bar the spring hangs from.
        let bar = apply(&m, [0.0, TOP, 0.0]);
        let inv = focal / (bar[2] + CAM).max(0.5);
        let by = cy + bar[1] * inv;
        let half_w = COIL_R * 2.2 * inv;
        let thick = (0.12 * inv).max(1.0);
        let (y0, y1) = ((by - thick).max(0.0) as usize, ((by + 0.5).max(0.0) as usize).min(h));
        let (x0, x1) = (
            (cx - half_w).max(0.0) as usize,
            ((cx + half_w).max(0.0) as usize).min(w),
        );
        for y in y0..y1 {
            let k = 0.35 + 0.25 * (y as f32 - by + thick) / thick;
            c.px[y * w + x0..y * w + x1].fill(rgb(k, k, k * 1.05));
        }
    }
}
