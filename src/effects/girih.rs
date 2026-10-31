use super::Effect;
use super::polygons::stroke;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, scale};
use std::f32::consts::{PI, TAU};

/// Islamic star-and-polygon lattice drawn with Hankin's method on the
/// octagon-and-square tiling: rays leave every edge midpoint at a contact
/// angle that slowly sweeps, so eight-pointed stars open and close. Lines
/// shimmer with waves and treble, colors jump on beats, bass pushes the zoom.
pub struct Girih {
    contact: f32,
    angle: f32,
    hue: f32,
    hue_target: f32,
    pan: f32,
}

impl Girih {
    pub fn new() -> Self {
        Self {
            contact: 0.0,
            angle: 0.0,
            hue: 0.08,
            hue_target: 0.08,
            pan: 0.0,
        }
    }
}

/// Hankin rays of a regular `n`-gon with apothem `apo` and first edge normal
/// at `phi0`, in lattice units around `center`, for contact angle `theta`.
fn rays(center: (f32, f32), n: usize, apo: f32, phi0: f32, theta: f32, out: &mut Vec<[(f32, f32); 2]>) {
    for j in 0..n {
        let phi = phi0 + TAU * j as f32 / n as f32;
        let (sn, cs) = phi.sin_cos();
        let m = (cs * apo, sn * apo);
        let tangent = (-sn, cs);
        let inward = (-cs, -sn);
        for side in [1.0f32, -1.0] {
            let d = (
                side * theta.cos() * tangent.0 + theta.sin() * inward.0,
                side * theta.cos() * tangent.1 + theta.sin() * inward.1,
            );
            // Meet the mirrored ray of the neighbouring edge on the bisector
            // through the shared vertex.
            let psi = phi + side * PI / n as f32;
            let u = (psi.cos(), psi.sin());
            let cross_du = d.0 * u.1 - d.1 * u.0;
            let cross_mu = m.0 * u.1 - m.1 * u.0;
            let mut len = if cross_du.abs() > 1e-6 {
                -cross_mu / cross_du
            } else {
                apo
            };
            let end = (m.0 + d.0 * len, m.1 + d.1 * len);
            // Past the center the rays would cross over; stop at the closest point instead.
            if end.0 * u.0 + end.1 * u.1 < 0.0 || len.is_nan() || len <= 0.0 {
                len = -(m.0 * d.0 + m.1 * d.1);
            }
            let len = len.max(0.0);
            out.push([
                (center.0 + m.0, center.1 + m.1),
                (center.0 + m.0 + d.0 * len, center.1 + m.1 + d.1 * len),
            ]);
        }
    }
}

impl Effect for Girih {
    fn name(&self) -> &'static str {
        "Girih"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.contact += dt * (0.12 + 0.2 * f.mid);
        self.angle += dt * 0.03;
        self.pan += dt * (0.05 + 0.15 * f.bass);
        if f.beat {
            self.hue_target += 0.13;
        }
        self.hue += (self.hue_target - self.hue) * (1.0 - (-dt * 5.0).exp());
        c.fill(hsv(self.hue + 0.5, 0.7, 0.05));

        // Contact angle sweeps between 30° and 67.5°, the classic star at the top.
        let theta = (30.0 + 37.5 * (0.5 - 0.5 * self.contact.cos())).to_radians();
        let (w, h) = (c.w as f32, c.h as f32);
        let cell = (w.min(h) / 2.6).max(9.0).max(w.max(h) / 10.0) * (1.0 + 0.06 * f.bass + 0.04 * f.pulse);
        let (sn, cs) = self.angle.sin_cos();
        // The lattice repeats every unit, so the vertical pan can wrap.
        let (ox, oy) = (self.pan.sin() * 0.7, (self.pan * 0.6).rem_euclid(1.0));
        let (cx, cy) = (w * 0.5, h * 0.5);
        let to_screen = |p: (f32, f32)| {
            let (x, y) = (p.0 - ox, p.1 - oy);
            (cx + (x * cs - y * sn) * cell, cy + (x * sn + y * cs) * cell)
        };

        // Lattice cells needed to cover the screen at any rotation.
        let reach = (0.5 * (w * w + h * h).sqrt() / cell).ceil() as i32 + 1;
        let (bx, by) = (ox.round() as i32, oy.round() as i32);
        let edge = 1.0 / (1.0 + 2f32.sqrt());
        let mut segs = Vec::new();
        for j in by - reach..=by + reach {
            for i in bx - reach..=bx + reach {
                let (x, y) = (i as f32, j as f32);
                rays((x, y), 8, 0.5, 0.0, theta, &mut segs);
                rays((x + 0.5, y + 0.5), 4, edge * 0.5, PI / 4.0, theta, &mut segs);
            }
        }

        let width = (cell * 0.05).max(0.8);
        let bins = f.spectrum.len();
        for (k, s) in segs.iter().enumerate() {
            let a = to_screen(s[0]);
            let b = to_screen(s[1]);
            if a.0.max(b.0) < -width || a.0.min(b.0) > w + width || a.1.max(b.1) < -width || a.1.min(b.1) > h + width {
                continue;
            }
            let (mx, my) = ((a.0 + b.0) * 0.5 - cx, (a.1 + b.1) * 0.5 - cy);
            let dist = (mx * mx + my * my).sqrt() / cell;
            let shimmer = 0.5 + 0.5 * (dist * 2.2 - t * 2.5).sin();
            let band = if bins > 0 { f.spectrum[(k * 7) % bins] } else { 0.0 };
            let value = (0.35 + 0.3 * shimmer + 0.35 * f.treble * band + 0.2 * f.pulse).min(1.0);
            let col = hsv(self.hue + 0.06 * dist, 0.75, value);
            stroke(c, a.0, a.1, b.0, b.1, width * 2.6, scale(col, 0.35));
            stroke(c, a.0, a.1, b.0, b.1, width, col);
        }
    }
}
