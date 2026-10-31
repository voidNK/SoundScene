use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, scale};
use std::f32::consts::TAU;

const RINGS: usize = 28;
const SPACING: f32 = 0.5;
const NEAR: f32 = 0.25;

struct Ring {
    z: f32,
    boost: f32,
    hue: f32,
}

/// Rings of dots flying out of a curving tunnel. Bass drives the speed,
/// beats spawn bright rings, the spectrum bulges each ring.
pub struct DotTunnel {
    rings: Vec<Ring>,
    travel: f32,
    spin: f32,
    hue: f32,
}

impl DotTunnel {
    pub fn new() -> Self {
        let rings = (0..RINGS)
            .map(|i| Ring {
                z: NEAR + i as f32 * SPACING,
                boost: 0.0,
                hue: 0.55,
            })
            .collect();
        Self {
            rings,
            travel: 0.0,
            spin: 0.0,
            hue: 0.55,
        }
    }
}

/// Tunnel center at world depth `z`.
fn path(z: f32) -> (f32, f32) {
    ((z * 0.23).sin() * 1.2 + (z * 0.11).cos() * 0.8, (z * 0.17).cos() * 0.9)
}

impl Effect for DotTunnel {
    fn name(&self) -> &'static str {
        "Dot Tunnel"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fade(0.35);
        let speed = 1.2 + f.bass * 4.0 + f.pulse * 2.0;
        self.travel += dt * speed;
        self.spin += dt * (0.2 + f.mid * 0.8);
        self.hue += dt * 0.02;

        let depth = RINGS as f32 * SPACING;
        for r in &mut self.rings {
            r.z -= dt * speed;
            r.boost = (r.boost - dt * 0.6).max(0.0);
            if r.z < NEAR {
                r.z += depth;
                r.hue = self.hue;
                r.boost = 0.0;
            }
        }
        if f.beat
            && let Some(r) = self.rings.iter_mut().max_by(|a, b| a.z.total_cmp(&b.z))
        {
            r.boost = 1.0;
            r.hue = self.hue + 0.5;
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let focal = w.min(h * 1.6) * 0.45;
        let (camx, camy) = path(self.travel);
        let dots = (w.max(h) * 0.15).clamp(10.0, 36.0) as usize;
        let bins = f.spectrum.len();

        // Far rings first so near ones paint over them.
        let mut order: Vec<usize> = (0..self.rings.len()).collect();
        order.sort_by(|&a, &b| self.rings[b].z.total_cmp(&self.rings[a].z));
        for i in order {
            let r = &self.rings[i];
            let (px, py) = path(self.travel + r.z);
            let fog = (1.0 - r.z / (depth + NEAR)).clamp(0.0, 1.0);
            let bright = (fog * fog * (0.7 + 0.3 * f.level) + r.boost * fog).min(1.0);
            let col = hsv(r.hue + r.z * 0.03, 0.75 - 0.5 * r.boost, bright);
            let size = (focal * 0.025 / r.z).clamp(0.0, 4.0);
            for d in 0..dots {
                let a = d as f32 / dots as f32 * TAU + self.spin + r.z * 0.2;
                let bulge = if bins > 0 {
                    1.0 + 0.3 * f.spectrum[d * bins / dots]
                } else {
                    1.0
                };
                let rad = (1.0 + 0.4 * r.boost) * bulge;
                let x = w * 0.5 + (px - camx + a.cos() * rad) / r.z * focal;
                let y = h * 0.5 + (py - camy + a.sin() * rad) / r.z * focal;
                blob(c, x, y, size, col);
            }
        }
    }
}

/// Filled disc of radius `r` with a dimmer rim; a single pixel when tiny.
fn blob(c: &mut Canvas, x: f32, y: f32, r: f32, col: Rgb) {
    let (w, h) = (c.w as f32, c.h as f32);
    if !x.is_finite() || !y.is_finite() || x < -r - 1.0 || y < -r - 1.0 || x > w + r || y > h + r {
        return;
    }
    if r < 0.7 {
        c.plot_add(x as i32, y as i32, col);
        return;
    }
    let ri = r.ceil() as i32;
    let inner = (r - 1.0).max(0.0).powi(2);
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let d2 = (dx * dx + dy * dy) as f32;
            if d2 <= r * r {
                let k = if d2 > inner { 0.55 } else { 1.0 };
                c.plot(x as i32 + dx, y as i32 + dy, scale(col, k));
            }
        }
    }
}
