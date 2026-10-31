use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, mix, scale};

/// Endless perspective checkerboard floor and mirrored ceiling that scroll
/// and roll. Bass speeds it up, beats flash the horizon.
pub struct Checkerboard {
    travel: f32,
    side: f32,
    hue: f32,
}

impl Checkerboard {
    pub fn new() -> Self {
        Self {
            travel: 0.0,
            side: 0.0,
            hue: 0.75,
        }
    }
}

impl Effect for Checkerboard {
    fn name(&self) -> &'static str {
        "Checkerboard"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.travel += dt * (1.5 + f.bass * 7.0 + f.pulse * 3.0);
        self.side += dt * (0.4 * (t * 0.13).sin() + f.mid * 0.6);
        self.hue += dt * 0.015 + if f.beat { 0.04 } else { 0.0 };
        if c.w == 0 || c.h == 0 {
            return;
        }

        let roll = 0.25 * (t * 0.21).sin() + 0.1 * (t * 0.07).sin();
        let (sr, cr) = roll.sin_cos();
        let inv = 2.0 / c.h as f32;
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let floor_a = hsv(self.hue, 0.7, 0.95);
        let floor_b = hsv(self.hue + 0.5, 0.6, 0.25);
        let ceil_a = hsv(self.hue + 0.15, 0.7, 0.85);
        let ceil_b = hsv(self.hue + 0.65, 0.5, 0.2);
        // Below this view slope a check gets smaller than about two pixels.
        // Fewer, larger checks on small canvases.
        let density = 1.4 * (c.h as f32 / 120.0).sqrt().clamp(0.45, 1.2);
        let alias = (3.0 * density / c.h as f32).sqrt();
        let horizon = hsv(self.hue + 0.08, 0.5, 0.6 + 0.4 * f.pulse);

        for y in 0..c.h {
            let sy = (y as f32 + 0.5 - cy) * inv;
            for x in 0..c.w {
                let sx = (x as f32 + 0.5 - cx) * inv;
                // Roll the view, then intersect the ray with the plane y = ±1.
                let rx = sx * cr - sy * sr;
                let ry = sx * sr + sy * cr;
                let ay = ry.abs().max(1e-3);
                let z = 1.0 / ay;
                let u = rx * z * density + self.side;
                let v = z * density + self.travel;
                let check = ((u.floor() as i64 + v.floor() as i64) & 1) == 0;
                let (a, b) = if ry > 0.0 { (floor_a, floor_b) } else { (ceil_a, ceil_b) };
                // Fade the checks to their average with distance to hide aliasing.
                let contrast = ((ay - alias) / (alias * 1.5)).clamp(0.0, 1.0);
                let fog = (ay * 3.0).min(1.0);
                let col = scale(mix(mix(a, b, 0.5), if check { a } else { b }, contrast), fog);
                // Glow along the horizon.
                let glow = (1.0 - ay * 8.0).max(0.0);
                c.set(x, y, mix(col, horizon, glow * glow));
            }
        }
    }
}
