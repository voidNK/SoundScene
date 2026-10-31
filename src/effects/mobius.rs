use super::Effect;
use super::donut::{ZBuf, apply, backdrop, dot, norm, rotation};
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};
use std::f32::consts::TAU;

const CAM: f32 = 4.2;
/// Color bands along the strip, each one fed by its own spectrum bin.
const BANDS: f32 = 18.0;

/// A rotating, z-buffered Möbius strip lit from both sides. Color bands
/// travel along it, each band glowing with its own spectrum bin; bass widens
/// the strip and beats push the bands forward.
pub struct Mobius {
    a: f32,
    b: f32,
    hue: f32,
    flow: f32,
    width: f32,
    zb: ZBuf,
}

impl Mobius {
    pub fn new() -> Self {
        Self {
            a: 1.1,
            b: 0.3,
            hue: 0.55,
            flow: 0.0,
            width: 0.4,
            zb: ZBuf::new(),
        }
    }
}

impl Effect for Mobius {
    fn name(&self) -> &'static str {
        "Möbius"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.a += dt * (0.25 + 0.6 * f.mid);
        self.b += dt * (0.35 + 1.1 * f.bass);
        self.hue += dt * 0.025;
        self.flow += dt * (0.6 + 2.5 * f.level) + if f.beat { 0.5 } else { 0.0 };
        let target = 0.38 + 0.2 * f.bass + 0.08 * f.pulse;
        self.width += (target - self.width) * (dt * 8.0).min(1.0);

        backdrop(c, [0.0, 0.02, 0.05], [0.02, 0.06, 0.08]);
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        self.zb.reset(w, h);

        let focal = w.min(h) as f32 * 1.25;
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let half_w = self.width;
        let ppu = focal / (CAM - 1.0 - half_w).max(0.5);
        let n_u = (TAU * (1.0 + half_w) * ppu * 1.5).clamp(16.0, 2400.0) as usize;
        let n_v = (2.0 * half_w * ppu * 1.8).clamp(3.0, 400.0) as usize;
        let m = rotation(self.a, self.b);
        let light = norm([-0.4, -0.7, -0.6]);
        let halfv = norm([light[0], light[1], light[2] - 1.0]);
        let bins = f.spectrum.len();

        for i in 0..n_u {
            let s = i as f32 / n_u as f32;
            let u = s * TAU;
            let (su, cu) = u.sin_cos();
            let (sh, ch) = (u * 0.5).sin_cos();
            let phase = s * BANDS - self.flow;
            let band = phase.floor();
            let frac = phase - band;
            let lvl = if bins > 0 {
                f.spectrum[(band.rem_euclid(BANDS) as usize * 3).min(bins - 1)]
            } else {
                0.0
            };
            // Soft dark seam between bands.
            let seam = (frac * (1.0 - frac) * 8.0).min(1.0);
            let base = hsv(self.hue + band * 0.055, 0.8, 1.0).map(|x| x as f32 / 255.0);
            let glow = (0.3 + 0.7 * lvl) * seam;
            for j in 0..=n_v {
                let v = (j as f32 / n_v.max(1) as f32 * 2.0 - 1.0) * half_w;
                let rr = 1.0 + v * ch;
                let q = apply(&m, [rr * cu, rr * su, v * sh]);
                let z = q[2] + CAM;
                if z < 0.1 {
                    continue;
                }
                let inv = focal / z;
                let Some(idx) = self.zb.test(cx + q[0] * inv, cy + q[1] * inv, z) else {
                    continue;
                };
                // Partial derivatives of the parametrization give the normal.
                let du = [-0.5 * v * sh * cu - rr * su, -0.5 * v * sh * su + rr * cu, 0.5 * v * ch];
                let dv = [ch * cu, ch * su, sh];
                let n = norm(apply(
                    &m,
                    [
                        du[1] * dv[2] - du[2] * dv[1],
                        du[2] * dv[0] - du[0] * dv[2],
                        du[0] * dv[1] - du[1] * dv[0],
                    ],
                ));
                // One-sided surface: always shade the side facing the camera.
                let n = if n[2] > 0.0 { [-n[0], -n[1], -n[2]] } else { n };
                let diff = dot(n, light).max(0.0);
                let spec = dot(n, halfv).max(0.0).powi(20) * 0.7;
                let edge = if j == 0 || j == n_v { 0.5 } else { 0.0 };
                let k = glow * (0.15 + 0.85 * diff) + edge;
                let sparkle = 0.15 * (t * 3.0 + s * 40.0).sin().max(0.0) * f.treble;
                c.px[idx] = rgb(
                    base[0] * k + spec + sparkle,
                    base[1] * k + spec + sparkle,
                    base[2] * k + spec + sparkle,
                );
            }
        }
    }
}
