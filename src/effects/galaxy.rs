use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, rgb};
use std::f32::consts::TAU;

const STARS: usize = 9000;
/// How far the orbit ellipses turn per unit radius; this forms the arms.
const TWIST: f32 = 3.2;

struct Star {
    r: f32,
    phase: f32,
    height: f32,
    bri: f32,
    hue: f32,
}

/// Spiral galaxy built from thousands of stars on nested, slowly turning
/// ellipses (density-wave arms). Inner stars orbit faster, the core glows
/// with the bass and each radius brightens with its spectrum band.
pub struct Galaxy {
    stars: Vec<Star>,
    pattern: f32,
    view: f32,
    hue: f32,
}

impl Galaxy {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x6A1A);
        let stars = (0..STARS)
            .map(|_| {
                // A small bulge plus a disk whose density falls off as 1/r.
                let r = if rng.f32() < 0.12 {
                    0.02 + rng.f32() * rng.f32() * 0.25
                } else {
                    0.08 + 1.15 * rng.f32()
                };
                let g = rng.range(-1.0, 1.0) + rng.range(-1.0, 1.0);
                Star {
                    r,
                    phase: rng.range(0.0, TAU),
                    height: g * 0.04 * (1.0 + 2.0 * (-r * 6.0).exp()),
                    bri: rng.range(0.25, 1.0),
                    hue: rng.range(-0.06, 0.06),
                }
            })
            .collect();
        Self {
            stars,
            pattern: 0.0,
            view: 0.0,
            hue: 0.6,
        }
    }
}

impl Effect for Galaxy {
    fn name(&self) -> &'static str {
        "Galaxy"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.35);
        let speed = 0.25 + 0.8 * f.level + 0.6 * f.pulse;
        self.pattern += dt * 0.05 * speed;
        self.view += dt * (0.04 + 0.08 * f.mid);
        self.hue += dt * 0.01 + if f.beat { 0.03 } else { 0.0 };

        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let size = (w * 0.38).min(h * 0.62);
        let incl = 1.05 + 0.25 * (t * 0.07).sin();
        let (si, ci) = incl.sin_cos();
        let (sv, cv) = self.view.sin_cos();
        let bins = f.spectrum.len();

        for s in &mut self.stars {
            s.phase += dt * speed * 0.35 / (s.r + 0.12);
            let tilt = s.r * TWIST + self.pattern;
            let (st, ct) = tilt.sin_cos();
            let (sp, cp) = s.phase.sin_cos();
            let (ex, ey) = (s.r * cp, s.r * 0.55 * sp);
            let (gx, gz) = (ex * ct - ey * st, ex * st + ey * ct);
            // View: turn around the galaxy axis, then incline.
            let (vx, vz) = (gx * cv - gz * sv, gx * sv + gz * cv);
            let vy = s.height * si + vz * ci;
            let depth = -s.height * ci + vz * si;
            let p = 1.0 / (1.0 + depth * 0.25).max(0.3);
            let band = if bins > 0 {
                f.spectrum[((s.r / 1.27 * bins as f32) as usize).min(bins - 1)]
            } else {
                0.0
            };
            let core = (-s.r * 5.0).exp();
            let hue = self.hue + s.hue + 0.45 * core - 0.4 * s.r + 0.3 * band;
            // Stars near the ends of their ellipse crowd into the arms; brighten them a bit more.
            let arm = 0.35 + 0.65 * cp * cp;
            let v = s.bri * (0.22 + 0.6 * band) * (0.6 + 0.4 * p) * arm * (1.0 - 0.7 * core);
            c.plot_add(
                (cx + vx * size * p).round() as i32,
                (cy + vy * size * p).round() as i32,
                hsv(hue, 0.55 - 0.4 * core, v),
            );
        }

        // Core glow, flattened by the inclination.
        let rc = size * (0.2 + 0.1 * f.bass);
        if rc < 0.5 {
            return;
        }
        let k = 0.1 + 0.35 * f.bass + 0.25 * f.pulse;
        let flat = ci.abs().max(0.3);
        let (x0, x1) = (
            (cx - rc).max(0.0) as usize,
            ((cx + rc).ceil().max(0.0) as usize).min(c.w),
        );
        let ry = rc * flat;
        let (y0, y1) = (
            (cy - ry).max(0.0) as usize,
            ((cy + ry).ceil().max(0.0) as usize).min(c.h),
        );
        for y in y0..y1 {
            let dy = (y as f32 + 0.5 - cy) / flat;
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let d2 = (dx * dx + dy * dy) / (rc * rc);
                if d2 < 1.0 {
                    let g = (-d2 * 9.0).exp() * k;
                    let i = y * c.w + x;
                    c.px[i] = crate::gfx::add(c.px[i], rgb(g, g * 0.8, g * 0.55));
                }
            }
        }
    }
}
