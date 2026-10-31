use super::Effect;
use super::donut::{apply, rotation};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};
use std::f32::consts::PI;

type V3 = [f32; 3];

const COUNT: usize = 1600;
const CAM: f32 = 4.0;
/// Seconds without a beat before the sphere bursts on its own.
const IDLE_BURST: f32 = 4.0;

struct Particle {
    home: V3,
    off: V3,
    vel: V3,
    /// How hard this particle flies out, so the burst is not a perfect shell.
    kick: f32,
}

/// A rotating sphere of particles that bursts outward on every beat and is
/// pulled back together by springs. Particles are depth-shaded and leave
/// short trails; bass sets the burst strength, mids the spin.
pub struct Explode {
    parts: Vec<Particle>,
    rng: Rng,
    a: f32,
    b: f32,
    hue: f32,
    idle: f32,
}

impl Explode {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xE7);
        // Fibonacci sphere for an even distribution.
        let golden = PI * (3.0 - 5f32.sqrt());
        let parts = (0..COUNT)
            .map(|i| {
                let y = 1.0 - 2.0 * (i as f32 + 0.5) / COUNT as f32;
                let r = (1.0 - y * y).max(0.0).sqrt();
                let (s, c) = (golden * i as f32).sin_cos();
                Particle {
                    home: [r * c, y, r * s],
                    off: [0.0; 3],
                    vel: [0.0; 3],
                    kick: rng.range(0.4, 1.6),
                }
            })
            .collect();
        Self {
            parts,
            rng,
            a: 0.4,
            b: 0.0,
            hue: 0.0,
            idle: 0.0,
        }
    }

    fn burst(&mut self, strength: f32) {
        for p in &mut self.parts {
            let s = strength * p.kick;
            let j = 0.4 * strength;
            for k in 0..3 {
                p.vel[k] += p.home[k] * s + self.rng.range(-j, j);
            }
        }
    }
}

impl Effect for Explode {
    fn name(&self) -> &'static str {
        "Explode"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.5);
        self.a += dt * (0.3 + 0.8 * f.mid);
        self.b += dt * (0.4 + 0.6 * f.level);
        self.hue += dt * 0.03;
        self.idle += dt;
        if f.beat {
            self.burst(2.5 + 4.0 * f.bass);
            self.hue += 0.07;
            self.idle = 0.0;
        } else if self.idle > IDLE_BURST {
            self.burst(1.5);
            self.idle = 0.0;
        }
        // Damped springs pull every particle home; sub-steps keep it stable at dt = 0.1.
        let steps = (dt / 0.02).ceil().clamp(0.0, 8.0) as usize;
        let h = if steps > 0 { dt / steps as f32 } else { 0.0 };
        for _ in 0..steps {
            for p in &mut self.parts {
                for k in 0..3 {
                    p.vel[k] += (-p.off[k] * 12.0 - p.vel[k] * 2.8) * h;
                    p.off[k] += p.vel[k] * h;
                }
            }
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        let m = rotation(self.a, self.b);
        let focal = c.w.min(c.h) as f32 * 1.3;
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let breathe = 1.0 + 0.06 * (t * 1.7).sin() + 0.12 * f.pulse;
        let big = c.w.min(c.h) >= 60;
        // Fewer pixels means more particles per pixel: dim them so tiles do not saturate.
        let gain = ((c.w * c.h) as f32 / 25000.0).clamp(0.12, 1.0);
        for p in &self.parts {
            let world = [
                p.home[0] * breathe + p.off[0],
                p.home[1] * breathe + p.off[1],
                p.home[2] * breathe + p.off[2],
            ];
            let q = apply(&m, world);
            let z = q[2] + CAM;
            if z < 0.3 {
                continue;
            }
            let inv = focal / z;
            let (x, y) = ((cx + q[0] * inv) as i32, (cy + q[1] * inv) as i32);
            // Near particles bright and warm, far ones dim and cool.
            let depth = (1.0 - q[2] * 0.45).clamp(0.05, 1.6);
            let spread = (p.off[0].abs() + p.off[1].abs() + p.off[2].abs()).min(3.0);
            let col = hsv(
                self.hue + 0.5 - 0.25 * depth + 0.08 * spread + 0.2 * p.home[1],
                0.7,
                ((0.25 + 0.5 * depth) * gain).min(1.0),
            );
            c.plot_add(x, y, col);
            if big && depth > 1.0 {
                c.plot_add(x + 1, y, col);
                c.plot_add(x, y + 1, col);
            }
        }
    }
}
