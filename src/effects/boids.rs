use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};
use std::f32::consts::TAU;

const MAX_BOIDS: usize = 220;
const VIEW: f32 = 0.12;
const PERSONAL: f32 = 0.04;

struct Boid {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

struct Shock {
    x: f32,
    y: f32,
    age: f32,
}

/// A flock (separation, alignment, cohesion) on a wrapping world, drawn as
/// little arrows with trails. Every beat scares the flock away from a point
/// with a visible shockwave; bass lets the birds fly faster.
pub struct Boids {
    boids: Vec<Boid>,
    accel: Vec<(f32, f32)>,
    shocks: Vec<Shock>,
    rng: Rng,
    hue: f32,
}

impl Boids {
    pub fn new() -> Self {
        Self {
            boids: Vec::new(),
            accel: Vec::new(),
            shocks: Vec::new(),
            rng: Rng::new(0xB01D),
            hue: 0.55,
        }
    }
}

/// Shortest signed distance on a ring of length `len`.
fn wrap_delta(d: f32, len: f32) -> f32 {
    d - len * (d / len).round()
}

impl Effect for Boids {
    fn name(&self) -> &'static str {
        "Boids"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        c.fade(0.72);
        // World units: height 1, width = aspect ratio.
        let unit = c.h as f32;
        let aspect = c.w as f32 / unit;

        let target = (c.w * c.h / 110).clamp(8, MAX_BOIDS);
        self.boids.truncate(target);
        while self.boids.len() < target {
            let a = self.rng.range(0.0, TAU);
            self.boids.push(Boid {
                x: self.rng.f32() * aspect,
                y: self.rng.f32(),
                vx: a.cos() * 0.2,
                vy: a.sin() * 0.2,
            });
        }

        self.hue += dt * 0.02;
        if f.beat {
            self.hue += 0.04;
            let (sx, sy) = (self.rng.f32() * aspect, self.rng.f32());
            let force = 0.8 + 1.2 * f.bass;
            for b in &mut self.boids {
                let (dx, dy) = (wrap_delta(b.x - sx, aspect), wrap_delta(b.y - sy, 1.0));
                let d = (dx * dx + dy * dy).sqrt().max(1e-4);
                if d < 0.45 {
                    let k = force * (1.0 - d / 0.45) / d;
                    b.vx += dx * k;
                    b.vy += dy * k;
                }
            }
            self.shocks.push(Shock { x: sx, y: sy, age: 0.0 });
        }

        self.accel.clear();
        for b in &self.boids {
            let (mut ax, mut ay, mut cx, mut cy, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            let mut n = 0;
            for o in &self.boids {
                let (dx, dy) = (wrap_delta(o.x - b.x, aspect), wrap_delta(o.y - b.y, 1.0));
                let d2 = dx * dx + dy * dy;
                if d2 < VIEW * VIEW && d2 > 1e-10 {
                    n += 1;
                    ax += o.vx;
                    ay += o.vy;
                    cx += dx;
                    cy += dy;
                    if d2 < PERSONAL * PERSONAL {
                        sx -= dx / d2;
                        sy -= dy / d2;
                    }
                }
            }
            let mut a = (0.0, 0.0);
            if n > 0 {
                let inv = 1.0 / n as f32;
                a.0 += (ax * inv - b.vx) * 1.8 + cx * inv * (1.5 + 2.0 * f.mid) + sx * 0.01;
                a.1 += (ay * inv - b.vy) * 1.8 + cy * inv * (1.5 + 2.0 * f.mid) + sy * 0.01;
            }
            self.accel.push(a);
        }

        let max_speed = 0.3 + 0.35 * f.bass + 0.8 * f.pulse;
        let min_speed = 0.12;
        for (b, &(ax, ay)) in self.boids.iter_mut().zip(&self.accel) {
            b.vx += ax.clamp(-20.0, 20.0) * dt + self.rng.range(-0.3, 0.3) * dt;
            b.vy += ay.clamp(-20.0, 20.0) * dt + self.rng.range(-0.3, 0.3) * dt;
            let s = (b.vx * b.vx + b.vy * b.vy).sqrt();
            let clamped = s.clamp(min_speed, max_speed);
            if s > 1e-6 {
                b.vx *= clamped / s;
                b.vy *= clamped / s;
            } else {
                b.vx = min_speed;
            }
            b.x = (b.x + b.vx * dt).rem_euclid(aspect.max(1e-3));
            b.y = (b.y + b.vy * dt).rem_euclid(1.0);
        }

        for s in &mut self.shocks {
            s.age += dt;
            let r = s.age * 0.9 * unit;
            let v = (1.0 - s.age / 0.6).max(0.0) * 0.5;
            let steps = (r * TAU).clamp(8.0, 600.0) as usize;
            for i in 0..steps {
                let a = i as f32 / steps as f32 * TAU;
                let col = hsv(self.hue + 0.5, 0.4, v);
                c.plot_add(
                    (s.x * unit + a.cos() * r) as i32,
                    (s.y * unit + a.sin() * r) as i32,
                    col,
                );
            }
        }
        self.shocks.retain(|s| s.age < 0.6);

        let len = (unit * 0.035).clamp(1.0, 5.0);
        for b in &self.boids {
            let s = (b.vx * b.vx + b.vy * b.vy).sqrt().max(1e-6);
            let (hx, hy) = (b.x * unit, b.y * unit);
            let (tx, ty) = (hx - b.vx / s * len, hy - b.vy / s * len);
            let heading = b.vy.atan2(b.vx) / TAU;
            let hue = self.hue + heading * 0.35;
            c.line_add(tx, ty, hx, hy, hsv(hue, 0.8, 0.2 + 0.25 * (s / max_speed).min(1.0)));
            c.plot(hx as i32, hy as i32, hsv(hue, 0.4, 1.0));
        }
    }
}
