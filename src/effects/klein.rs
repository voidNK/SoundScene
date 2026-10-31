use super::Effect;
use super::donut::{apply, dot, norm, rotation};
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

type V3 = [f32; 3];

const CAM: f32 = 9.0;
/// Wire lines of constant u and of constant v.
const LINES_U: usize = 40;
const LINES_V: usize = 14;

/// Figure-8 immersion of the Klein bottle.
fn klein(a: f32, u: f32, v: f32) -> V3 {
    let (su, cu) = u.sin_cos();
    let (sh, ch) = (u * 0.5).sin_cos();
    let (sv, s2v) = (v.sin(), (2.0 * v).sin());
    let r = a + ch * sv - sh * s2v;
    [r * cu, r * su, sh * sv + ch * s2v]
}

/// A rotating figure-8 Klein bottle drawn as a glowing wire mesh, colored
/// and dimmed by depth and shaded by its surface normal. Each u-line glows
/// with its own spectrum bin, bass swells the bottle, beats flash it.
pub struct Klein {
    a: f32,
    b: f32,
    hue: f32,
    swell: f32,
    flash: f32,
    /// Brightness scale so dense lines on small canvases do not saturate.
    dim: f32,
}

impl Klein {
    pub fn new() -> Self {
        Self {
            a: 0.9,
            b: 0.0,
            hue: 0.6,
            swell: 2.2,
            flash: 0.0,
            dim: 1.0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dot_at(&self, c: &mut Canvas, m: &[V3; 3], u: f32, v: f32, focal: f32, gain: f32, light: V3) {
        let e = 0.01;
        let p = klein(self.swell, u, v);
        let pu = klein(self.swell, u + e, v);
        let pv = klein(self.swell, u, v + e);
        let du = [pu[0] - p[0], pu[1] - p[1], pu[2] - p[2]];
        let dv = [pv[0] - p[0], pv[1] - p[1], pv[2] - p[2]];
        let n = norm(apply(
            m,
            [
                du[1] * dv[2] - du[2] * dv[1],
                du[2] * dv[0] - du[0] * dv[2],
                du[0] * dv[1] - du[1] * dv[0],
            ],
        ));
        let q = apply(m, p);
        let z = q[2] + CAM;
        if z < 0.5 {
            return;
        }
        // Depth 0 = nearest, 1 = farthest.
        let depth = ((q[2] + 4.0) / 8.0).clamp(0.0, 1.0);
        let shade = 0.35 + 0.65 * dot(n, light).abs();
        let bri = gain * self.dim * shade * (1.0 - 0.75 * depth) * (1.0 + self.flash);
        let col = hsv(self.hue + depth * 0.45, 0.75 - 0.3 * self.flash, bri.min(1.0));
        let inv = focal / z;
        c.plot_add(
            (c.w as f32 * 0.5 + q[0] * inv) as i32,
            (c.h as f32 * 0.5 + q[1] * inv) as i32,
            col,
        );
    }
}

impl Effect for Klein {
    fn name(&self) -> &'static str {
        "Klein Bottle"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.35);
        self.a += dt * (0.25 + 0.5 * f.mid);
        self.b += dt * (0.3 + 0.9 * f.bass);
        self.hue += dt * 0.03;
        self.flash = if f.beat { 0.8 } else { self.flash * (-dt * 5.0).exp() };
        let target = 2.2 + 0.5 * f.bass;
        self.swell += (target - self.swell) * (dt * 8.0).min(1.0);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let focal = c.w.min(c.h) as f32 * 1.6;
        self.dim = (c.w.min(c.h) as f32 / 110.0).clamp(0.25, 1.0);
        let m = rotation(self.a, self.b);
        let light = norm([-0.4, -0.6, -0.7]);
        // Sample density follows the canvas so lines stay continuous.
        let n = (c.w.max(c.h) as f32 * 1.6).clamp(24.0, 700.0) as usize;
        let bins = f.spectrum.len();
        let roll = t * 0.15;

        for i in 0..LINES_U {
            let u = (i as f32 / LINES_U as f32 + roll) * TAU;
            let lvl = if bins > 0 {
                f.spectrum[(i * bins / LINES_U).min(bins - 1)]
            } else {
                0.0
            };
            let gain = 0.25 + 0.6 * lvl;
            for k in 0..n {
                let v = k as f32 / n as f32 * TAU;
                self.dot_at(c, &m, u, v, focal, gain, light);
            }
        }
        let gain = 0.3 + 0.3 * f.level;
        for j in 0..LINES_V {
            let v = j as f32 / LINES_V as f32 * TAU;
            for k in 0..n * 2 {
                let u = k as f32 / (n * 2) as f32 * TAU;
                self.dot_at(c, &m, u, v, focal, gain, light);
            }
        }
    }
}
