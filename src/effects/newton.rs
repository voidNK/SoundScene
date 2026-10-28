use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

const MAX_ITER: u32 = 22;

/// Newton fractal of z^n − 1: every pixel is colored by the root Newton's
/// method converges to and shaded by how long it took. Beats switch the
/// degree n (3 to 7), bass relaxes the step size, which tangles the basin
/// borders, mids spin the view.
pub struct Newton {
    n: u32,
    cooldown: f32,
    relax: f32,
    rot: f32,
    zoom: f32,
    hue: f32,
    vals: Vec<f32>,
    prev: Vec<f32>,
    /// Previous frame's pixel size and rotation (sin, cos).
    view: Option<(f32, f32, f32)>,
    frame: usize,
    size: (usize, usize),
}

impl Newton {
    pub fn new() -> Self {
        Self {
            n: 3,
            cooldown: 0.0,
            relax: 1.0,
            rot: 0.0,
            zoom: 0.0,
            hue: 0.0,
            vals: Vec::new(),
            prev: Vec::new(),
            view: None,
            frame: 0,
            size: (0, 0),
        }
    }
}

impl Effect for Newton {
    fn name(&self) -> &'static str {
        "Newton"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.vals = vec![-1.0; c.w * c.h];
            self.prev = vec![-1.0; c.w * c.h];
            self.view = None;
        }
        self.cooldown -= dt;
        if f.beat && self.cooldown <= 0.0 {
            self.n = if self.n >= 7 { 3 } else { self.n + 1 };
            self.cooldown = 4.0;
            // The old frame shows a different fractal, nothing to reuse.
            self.view = None;
            self.hue += 0.17;
        }
        let target = 1.0 + 0.3 * f.bass;
        self.relax += (target - self.relax) * (1.0 - (-3.0 * dt).exp());
        self.rot += dt * (0.1 + 0.4 * f.mid);
        self.zoom += dt * 0.15;
        self.hue += dt * 0.02;

        let n = self.n;
        let nf = n as f32;
        let span = 1.3 + 0.9 * self.zoom.sin();
        let per = 2.0 * span / c.w.min(c.h).max(1) as f32 * (1.0 - 0.08 * f.pulse);
        let (sn, cs) = self.rot.sin_cos();
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let a = self.relax;
        let glow = 0.8 + 0.2 * f.pulse;

        // Every other row is solved per frame; the rows in between are
        // reprojected from the previous frame's results.
        std::mem::swap(&mut self.vals, &mut self.prev);
        let old = self.view;
        self.frame += 1;
        let parity = ((self.frame - 1) % 2) as f32;
        for y in 0..c.h {
            let fresh = old.is_none() || (y + self.frame).is_multiple_of(2);
            let dy = (y as f32 - cy) * per;
            for x in 0..c.w {
                let dx = (x as f32 - cx) * per;
                let (zr, zi) = (dx * cs - dy * sn, dx * sn + dy * cs);
                let reused = match old {
                    Some((qper, qs, qc)) if !fresh => {
                        let u = ((zr * qc + zi * qs) / qper + cx).round();
                        // Snap to the nearest row that was iterated last frame, so values are
                        // never more than one frame old.
                        let v = ((((zi * qc - zr * qs) / qper + cy) - parity) * 0.5).round() * 2.0 + parity;
                        if u >= 0.0 && v >= 0.0 && (u as usize) < c.w && (v as usize) < c.h {
                            Some(self.prev[v as usize * c.w + u as usize])
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                self.vals[y * c.w + x] = reused.unwrap_or_else(|| solve(zr, zi, n, a));
            }
        }
        self.view = Some((per, sn, cs));

        for (px, &v) in c.px.iter_mut().zip(&self.vals) {
            *px = if v < 0.0 {
                [0, 0, 0]
            } else {
                let (k, s) = ((v / 64.0).floor(), v.rem_euclid(64.0));
                let b = (1.0 - s / MAX_ITER as f32).max(0.0).powf(1.6) * glow;
                hsv(self.hue + k / nf, 0.9 - 0.5 * (1.0 - b), b)
            };
        }
    }
}

/// Runs Newton's method from z and returns root index * 64 + smooth
/// iteration count, or -1 if it did not converge.
fn solve(mut zr: f32, mut zi: f32, n: u32, a: f32) -> f32 {
    let nf = n as f32;
    let mut it = 0;
    let mut err = f32::MAX;
    while it < MAX_ITER {
        let m2 = zr * zr + zi * zi;
        if !m2.is_finite() {
            break;
        }
        if m2 > 4.0 {
            // Far from the roots a step is almost exactly z *= 1 - a/n, so
            // take all steps down to |z| = 2 at once.
            let shrink = 1.0 - a / nf;
            let k = (0.5 * (m2 / 4.0).ln() / -shrink.ln()).ceil().clamp(1.0, 1000.0);
            let s = shrink.powf(k);
            zr *= s;
            zi *= s;
            it += k as u32;
            continue;
        }
        // p = z^(n-1)
        let (mut pr, mut pi) = (zr, zi);
        for _ in 2..n {
            (pr, pi) = (pr * zr - pi * zi, pr * zi + pi * zr);
        }
        let fr = pr * zr - pi * zi - 1.0;
        let fi = pr * zi + pi * zr;
        err = fr * fr + fi * fi;
        if err < 1e-6 {
            break;
        }
        let (dr, di) = (nf * pr, nf * pi);
        let d = dr * dr + di * di;
        if d < 1e-12 {
            break;
        }
        // z -= a * f / f'
        let qr = (fr * dr + fi * di) / d;
        let qi = (fi * dr - fr * di) / d;
        zr -= a * qr;
        zi -= a * qi;
        it += 1;
    }
    if err < 1e-3 && zr.is_finite() && zi.is_finite() {
        let k = ((zi.atan2(zr) / TAU * nf).round() as i32).rem_euclid(n as i32) as f32;
        // Convergence is quadratic, so the final error lies between tol and
        // tol²; its log gives a fractional step that removes banding.
        let s = it as f32 + (2.0 - err.max(1e-30).ln() / 1e-6f32.ln()).clamp(0.0, 1.0);
        k * 64.0 + s.min(63.0)
    } else {
        -1.0
    }
}
