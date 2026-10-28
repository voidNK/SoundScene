use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng};
use std::f32::consts::TAU;

/// Pixels per computed sample along each axis.
const CELL: usize = 2;
const ITERS: usize = 5;
const MAX_RIPPLES: usize = 4;

struct Ripple {
    x: f32,
    y: f32,
    age: f32,
}

/// Underwater light caustics from the classic iterated sin/cos warp, in blue
/// and teal. Brightness follows the loudness, mids speed up the water, and
/// every beat drops a ring ripple that distorts the light pattern.
pub struct Caustics {
    out: Vec<[f32; 3]>,
    ripples: Vec<Ripple>,
    rng: Rng,
    time: f32,
}

impl Caustics {
    pub fn new() -> Self {
        Self {
            out: Vec::new(),
            ripples: Vec::new(),
            rng: Rng::new(0xCA05),
            time: 23.0,
        }
    }
}

impl Effect for Caustics {
    fn name(&self) -> &'static str {
        "Caustics"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        self.out.resize(gw * gh, [0.0; 3]);

        self.time += dt * (0.35 + 0.6 * f.mid + 0.3 * f.bass);
        if f.beat {
            if self.ripples.len() >= MAX_RIPPLES {
                self.ripples.remove(0);
            }
            let aspect = gw as f32 / gh as f32;
            self.ripples.push(Ripple {
                x: self.rng.range(0.2, 0.8) * aspect,
                y: self.rng.range(0.2, 0.8),
                age: 0.0,
            });
        }
        for r in &mut self.ripples {
            r.age += dt;
        }
        self.ripples.retain(|r| r.age < 2.5);

        let t = self.time;
        let gain = 0.55 + 0.7 * f.level + 0.35 * f.pulse;
        let inv = 1.0 / gh as f32;
        let times: Vec<f32> = (0..ITERS).map(|n| t * (1.0 - 3.5 / (n + 1) as f32)).collect();
        for y in 0..gh {
            for x in 0..gw {
                let (mut u, mut v) = (x as f32 * inv, y as f32 * inv);
                for r in &self.ripples {
                    let (dx, dy) = (u - r.x, v - r.y);
                    let d = (dx * dx + dy * dy).sqrt().max(1e-4);
                    let front = d - r.age * 0.45;
                    let amp = 0.035 * (-r.age * 1.2).exp() * (-front * front * 60.0).exp();
                    let wave = (front * 40.0).sin() * amp;
                    u += dx / d * wave;
                    v += dy / d * wave;
                }
                let (px, py) = (u * TAU * 1.1 - 250.0, v * TAU * 1.1 - 250.0);
                let (mut ix, mut iy) = (px, py);
                let mut acc = 1.0;
                for &tn in &times {
                    let (nx, ny) = (
                        px + (tn - ix).cos() + (tn + iy).sin(),
                        py + (tn - iy).sin() + (tn + ix).cos(),
                    );
                    (ix, iy) = (nx, ny);
                    let a = px / ((ix + tn).sin() / 0.005);
                    let b = py / ((iy + tn).cos() / 0.005);
                    let len = (a * a + b * b).sqrt();
                    if len.is_finite() && len > 1e-6 {
                        acc += 1.0 / len;
                    }
                }
                let k = (1.17 - (acc / ITERS as f32).powf(1.4)).abs().powi(8).min(4.0) * gain;
                let depth = 1.0 - 0.35 * y as f32 / gh as f32;
                self.out[y * gw + x] = [(k * 0.9) * depth, (0.25 + k) * depth, (0.42 + k * 0.9) * depth];
            }
        }
        upscale(c, &self.out, gw, gh);
    }
}
