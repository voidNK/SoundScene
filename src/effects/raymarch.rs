use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};
use std::f32::consts::TAU;

type V3 = [f32; 3];

const BOUND: f32 = 2.2;
const FLOOR_Y: f32 = -1.7;
const STEPS: usize = 40;

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-6 { mul(a, 1.0 / l) } else { [0.0, 1.0, 0.0] }
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

/// Distance along `ro + t * rd` to the bounding sphere, as (enter, exit).
fn bound_hit(ro: V3, rd: V3) -> Option<(f32, f32)> {
    let b = dot(ro, rd);
    let disc = b * b - (dot(ro, ro) - BOUND * BOUND);
    if disc <= 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let t1 = -b + s;
    (t1 > 0.0).then(|| ((-b - s).max(0.0), t1))
}

struct Scene {
    blobs: [(V3, f32); 3],
    hues: [f32; 3],
    torus_rot: (f32, f32),
    torus_r: (f32, f32),
}

impl Scene {
    fn dist(&self, p: V3) -> f32 {
        let mut d = f32::MAX;
        for (i, &(cen, r)) in self.blobs.iter().enumerate() {
            let q = sub(p, cen);
            let s = dot(q, q).sqrt() - r;
            d = if i == 0 { s } else { smin(d, s, 0.55) };
        }
        let (s, co) = self.torus_rot;
        let (y, z) = (p[1] * co - p[2] * s, p[1] * s + p[2] * co);
        let qx = (p[0] * p[0] + z * z).sqrt() - self.torus_r.0;
        let torus = (qx * qx + y * y).sqrt() - self.torus_r.1;
        smin(d, torus, 0.25)
    }

    fn normal(&self, p: V3) -> V3 {
        // Tetrahedron technique: four samples instead of six.
        let e = 0.004;
        let k = [[1.0, -1.0, -1.0], [-1.0, -1.0, 1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, 1.0]];
        let mut n = [0.0; 3];
        for d in k {
            n = add(n, mul(d, self.dist(add(p, mul(d, e)))));
        }
        norm(n)
    }

    /// Marches only inside the bounding sphere; returns the hit distance.
    fn march(&self, ro: V3, rd: V3) -> Option<f32> {
        let (mut t, t1) = bound_hit(ro, rd)?;
        let mut d = f32::MAX;
        for _ in 0..STEPS {
            d = self.dist(add(ro, mul(rd, t)));
            if d < 0.002 * t {
                return Some(t);
            }
            t += d * 0.9;
            if t > t1 {
                return None;
            }
        }
        // Out of steps while grazing a surface still counts as a hit.
        (d < 0.05).then_some(t)
    }

    /// Blob color weighted by proximity to each blob center.
    fn albedo(&self, p: V3) -> V3 {
        let mut col = [0.0; 3];
        let mut total = 0.0;
        for (&(cen, r), &hue) in self.blobs.iter().zip(&self.hues) {
            let q = sub(p, cen);
            let wgt = (-(dot(q, q).sqrt() - r).max(0.0) * 6.0).exp() + 1e-3;
            let c = hsv(hue, 0.75, 1.0);
            col = add(col, mul([c[0] as f32, c[1] as f32, c[2] as f32], wgt / 255.0));
            total += wgt;
        }
        mul(col, 1.0 / total)
    }
}

fn sky(rd: V3, hue: f32, flash: f32) -> V3 {
    let k = rd[1].clamp(-1.0, 1.0) * 0.5 + 0.5;
    let top = hsv(hue + 0.55, 0.7, 0.18);
    let bot = hsv(hue + 0.75, 0.6, 0.55);
    let m = |i: usize| (bot[i] as f32 + (top[i] as f32 - bot[i] as f32) * k.powf(0.6)) / 255.0 + flash * 0.15;
    [m(0), m(1), m(2)]
}

/// Raymarched scene: three blobs melt together with a spinning torus above a
/// glossy checkerboard floor. Spectrum bands inflate the blobs, bass speeds up
/// the camera orbit and beats thicken the torus.
pub struct Raymarch {
    low: Vec<V3>,
    orbit: f32,
    phase: f32,
    hue: f32,
}

impl Raymarch {
    pub fn new() -> Self {
        Self {
            low: Vec::new(),
            orbit: 0.0,
            phase: 0.0,
            hue: 0.8,
        }
    }
}

fn band(f: &Features, lo: usize, hi: usize) -> f32 {
    let s = f.spectrum.get(lo..hi.min(f.spectrum.len())).unwrap_or(&[]);
    if s.is_empty() {
        0.0
    } else {
        s.iter().sum::<f32>() / s.len() as f32
    }
}

impl Effect for Raymarch {
    fn name(&self) -> &'static str {
        "Raymarch"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.orbit += dt * (0.12 + 0.45 * f.bass);
        self.phase += dt * (0.5 + 1.0 * f.level);
        self.hue += dt * 0.02 + if f.beat { 0.06 } else { 0.0 };
        if c.w == 0 || c.h == 0 {
            return;
        }
        // Render at half resolution and upscale.
        let (lw, lh) = (c.w.div_ceil(2), c.h.div_ceil(2));
        self.low.resize(lw * lh, [0.0; 3]);

        let bands = [band(f, 0, 8), band(f, 8, 24), band(f, 24, 64)];
        let ph = self.phase;
        let mut blobs = [([0.0; 3], 0.0); 3];
        for (i, b) in blobs.iter_mut().enumerate() {
            let a = ph * (0.6 + 0.25 * i as f32) + i as f32 * TAU / 3.0;
            let cen = [
                0.85 * a.cos(),
                0.1 + 0.4 * (ph * 1.3 + i as f32 * 2.0).sin(),
                0.85 * a.sin(),
            ];
            *b = (cen, 0.4 + 0.35 * bands[i] + 0.08 * f.pulse);
        }
        let scene = Scene {
            blobs,
            hues: [self.hue, self.hue + 0.33, self.hue + 0.66],
            torus_rot: (ph * 0.45).sin_cos(),
            torus_r: (1.25 + 0.1 * f.bass, 0.1 + 0.1 * f.pulse),
        };

        let a = self.orbit;
        let ro = [4.6 * a.cos(), 1.0 + 0.5 * (a * 0.7).sin(), 4.6 * a.sin()];
        let fwd = norm(sub([0.0, -0.2, 0.0], ro));
        let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
        let up = cross(right, fwd);
        let light = norm([0.5, 0.8, -0.3]);
        let flash = f.pulse * f.pulse;

        let shade = |p: V3, rd: V3| -> V3 {
            let n = scene.normal(p);
            let base = scene.albedo(p);
            let diff = dot(n, light).max(0.0);
            let refl = sub(rd, mul(n, 2.0 * dot(rd, n)));
            let spec = dot(refl, light).max(0.0).powi(24);
            let rim = (1.0 + dot(rd, n)).clamp(0.0, 1.0).powi(3);
            let env = sky(refl, self.hue, flash);
            let mut col = mul(base, 0.12 + 0.88 * diff);
            col = add(col, [spec * 0.9; 3]);
            add(col, mul(env, rim * 0.8))
        };

        for y in 0..lh {
            let v = -(y as f32 + 0.5 - lh as f32 * 0.5) / lh as f32;
            for x in 0..lw {
                let u = (x as f32 + 0.5 - lw as f32 * 0.5) / lh as f32;
                let rd = norm(add(add(mul(fwd, 1.5), mul(right, u)), mul(up, v)));
                let t_floor = if rd[1] < -1e-4 {
                    (FLOOR_Y - ro[1]) / rd[1]
                } else {
                    f32::MAX
                };
                let col = match scene.march(ro, rd) {
                    Some(t) if t < t_floor => {
                        let fog = (-t * t * 0.004).exp();
                        let s = shade(add(ro, mul(rd, t)), rd);
                        let bg = sky(rd, self.hue, flash);
                        add(mul(s, fog), mul(bg, 1.0 - fog))
                    }
                    _ if t_floor < f32::MAX => {
                        let p = add(ro, mul(rd, t_floor));
                        let check = ((p[0].floor() + p[2].floor()) as i32 & 1) as f32;
                        let mut base = [0.05 + 0.1 * check, 0.05 + 0.08 * check, 0.08 + 0.14 * check];
                        // Blob shadows straight below each center.
                        let mut sh = 1.0;
                        for &(cen, r) in &scene.blobs {
                            let (dx, dz) = (p[0] - cen[0], p[2] - cen[2]);
                            sh *= 1.0 - 0.7 * (-(dx * dx + dz * dz) / (r * r * 1.5)).exp();
                        }
                        base = mul(base, sh);
                        let rr = [rd[0], -rd[1], rd[2]];
                        let fres = 0.25 + 0.5 * (1.0 + rd[1]).powi(4).min(1.0);
                        let refl = match scene.march(p, rr) {
                            Some(t2) => shade(add(p, mul(rr, t2)), rr),
                            None => sky(rr, self.hue, flash),
                        };
                        let col = add(base, mul(refl, fres * 0.6));
                        let fog = (-t_floor * 0.035).exp();
                        add(mul(col, fog), mul(sky(rd, self.hue, flash), 1.0 - fog))
                    }
                    _ => sky(rd, self.hue, flash),
                };
                self.low[y * lw + x] = col;
            }
        }

        // Bilinear upscale.
        for y in 0..c.h {
            let fy = ((y as f32 - 0.5) * 0.5).clamp(0.0, (lh - 1) as f32);
            let y0 = fy as usize;
            let y1 = (y0 + 1).min(lh - 1);
            let ty = fy - y0 as f32;
            for x in 0..c.w {
                let fx = ((x as f32 - 0.5) * 0.5).clamp(0.0, (lw - 1) as f32);
                let x0 = fx as usize;
                let x1 = (x0 + 1).min(lw - 1);
                let tx = fx - x0 as f32;
                let l = &self.low;
                let ch = |i: usize| {
                    let top = l[y0 * lw + x0][i] * (1.0 - tx) + l[y0 * lw + x1][i] * tx;
                    let bot = l[y1 * lw + x0][i] * (1.0 - tx) + l[y1 * lw + x1][i] * tx;
                    top * (1.0 - ty) + bot * ty
                };
                c.set(x, y, rgb(ch(0), ch(1), ch(2)));
            }
        }
    }
}
