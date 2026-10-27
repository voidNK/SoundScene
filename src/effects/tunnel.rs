use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::PI;

/// Checkerboard tunnel. Bass pushes the camera forward, mid spins and twists it.
pub struct Tunnel {
    z: f32,
    rot: f32,
    hue: f32,
}

impl Tunnel {
    pub fn new() -> Self {
        Self { z: 0.0, rot: 0.0, hue: 0.6 }
    }
}

impl Effect for Tunnel {
    fn name(&self) -> &'static str {
        "Tunnel"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.z += dt * (0.8 + f.bass * 5.0 + f.pulse * 3.0);
        self.rot += dt * (0.15 + f.mid * 1.2);
        self.hue += dt * 0.02 + if f.beat { 0.05 } else { 0.0 };

        let (w, h) = (c.w as f32, c.h as f32);
        let cx = w * 0.5 + (t * 0.6).sin() * w * 0.12;
        let cy = h * 0.5 + (t * 0.45).cos() * h * 0.12;
        let radius = w.min(h) * 0.5;
        let twist = 0.3 * (t * 0.3).sin() + f.mid * 0.5;
        let flash = 0.7 + 0.3 * f.pulse;

        for y in 0..c.h {
            let dy = y as f32 - cy;
            for x in 0..c.w {
                let dx = x as f32 - cx;
                let d = (dx * dx + dy * dy).sqrt().max(0.5);
                let depth = radius / d;
                // a*4 spans 8 units around the circle, so the texture has no seam.
                let a = dy.atan2(dx) / PI;
                let u = depth * 2.0 + self.z;
                let v = a * 4.0 + self.rot + depth * twist;
                let checker = (u.floor() as i64 + v.floor() as i64) & 1 == 0;
                let fog = (d / radius).min(1.0).powf(0.7);
                let (hue, base) = if checker { (0.0, 1.0) } else { (0.5, 0.3) };
                c.set(x, y, hsv(self.hue + hue + depth * 0.03, 0.8, base * fog * flash));
            }
        }
    }
}
