use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::{PI, TAU};

const POINTS: usize = 640;
const SHAPES: usize = 5;
/// Seconds before a morph may start, and before one starts without a beat.
const MIN_HOLD: f32 = 2.5;
const MAX_HOLD: f32 = 7.0;

/// Position of point `i` on shape `s`, roughly within the unit sphere.
fn shape(s: usize, i: usize) -> [f32; 3] {
    let k = (i as f32 + 0.5) / POINTS as f32;
    match s % SHAPES {
        0 => {
            // Fibonacci sphere.
            let y = 1.0 - 2.0 * k;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let a = PI * (3.0 - 5f32.sqrt()) * i as f32;
            [a.cos() * r, y, a.sin() * r]
        }
        1 => {
            // Cube surface: a grid on each of the six faces.
            let face = i % 6;
            let j = i / 6;
            let n = ((POINTS / 6) as f32).sqrt().ceil().max(1.0) as usize;
            let a = (j % n) as f32 / (n - 1).max(1) as f32 * 2.0 - 1.0;
            let b = (j / n % n) as f32 / (n - 1).max(1) as f32 * 2.0 - 1.0;
            let s = if face.is_multiple_of(2) { 0.62 } else { -0.62 };
            let (a, b) = (a * 0.62, b * 0.62);
            match face / 2 {
                0 => [s, a, b],
                1 => [a, s, b],
                _ => [a, b, s],
            }
        }
        2 => {
            // Torus.
            let rings = 32;
            let u = (i % rings) as f32 / rings as f32 * TAU;
            let v = (i / rings) as f32 / (POINTS / rings) as f32 * TAU;
            let r = 0.65 + 0.3 * u.cos();
            [r * v.cos(), 0.3 * u.sin(), r * v.sin()]
        }
        3 => {
            // Double spiral.
            let arm = if i.is_multiple_of(2) { 0.0 } else { PI };
            let a = k * TAU * 3.0 + arm;
            let r = 0.25 + 0.55 * (1.0 - (2.0 * k - 1.0).abs());
            [a.cos() * r, 1.8 * k - 0.9, a.sin() * r]
        }
        _ => {
            // Rippled disc.
            let r = k.sqrt() * 0.95;
            let a = PI * (3.0 - 5f32.sqrt()) * i as f32;
            [a.cos() * r, 0.2 * (r * 9.0).sin(), a.sin() * r]
        }
    }
}

/// A rotating 3D point cloud morphing between sphere, cube, torus, spiral
/// and disc. Beats trigger the next morph, bass makes it breathe.
pub struct DotMorph {
    from: usize,
    to: usize,
    progress: f32,
    hold: f32,
    rot: f32,
    hue: f32,
}

impl DotMorph {
    pub fn new() -> Self {
        Self {
            from: 0,
            to: 0,
            progress: 1.0,
            hold: 0.0,
            rot: 0.0,
            hue: 0.5,
        }
    }
}

impl Effect for DotMorph {
    fn name(&self) -> &'static str {
        "Dot Morph"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.3);
        self.rot += dt * (0.5 + f.mid * 1.2);
        self.hue += dt * 0.02;
        if self.progress < 1.0 {
            self.progress = (self.progress + dt * 0.9).min(1.0);
        } else {
            self.hold += dt;
            if (f.beat && self.hold > MIN_HOLD) || self.hold > MAX_HOLD {
                self.from = self.to;
                self.to = (self.to + 1) % SHAPES;
                self.progress = 0.0;
                self.hold = 0.0;
                self.hue += 0.15;
            }
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        let p = self.progress;
        let ease = p * p * (3.0 - 2.0 * p);
        let (sy, cy) = self.rot.sin_cos();
        let tilt = 0.5 + 0.3 * (t * 0.37).sin();
        let (sx, cx) = tilt.sin_cos();
        let (w, h) = (c.w as f32, c.h as f32);
        let radius = w.min(h) * 0.42 * (1.0 + 0.12 * f.pulse + 0.05 * f.bass);
        let big = radius > 30.0;
        for i in 0..POINTS {
            let a = shape(self.from, i);
            let b = shape(self.to, i);
            // Points bulge outward mid-morph so the transition reads as a burst.
            let lift = 1.0 + 0.35 * (p * PI).sin();
            let v: [f32; 3] = std::array::from_fn(|k| (a[k] + (b[k] - a[k]) * ease) * lift);
            let (x, z) = (v[0] * cy + v[2] * sy, -v[0] * sy + v[2] * cy);
            let (y, z) = (v[1] * cx - z * sx, v[1] * sx + z * cx);
            let persp = 2.8 / (z + 2.8);
            let px = (w * 0.5 + x * radius * persp).round() as i32;
            let py = (h * 0.5 + y * radius * persp).round() as i32;
            let depth = ((1.2 - z) * 0.5).clamp(0.2, 1.0);
            let col = hsv(self.hue + y * 0.15 + i as f32 * 0.0004, 0.65, depth);
            c.plot(px, py, col);
            if big && z < 0.0 {
                c.plot(px + 1, py, col);
                c.plot(px, py + 1, col);
                c.plot(px + 1, py + 1, col);
            }
        }
    }
}
