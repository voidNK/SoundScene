use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, scale};
use std::f32::consts::TAU;

const G: f32 = 0.5;
const MAX_HEAVY: usize = 6;
const LIGHT: usize = 180;
const STEP: f32 = 0.004;
const MAX_STEPS: usize = 30;
/// Squared softening lengths keep close encounters finite.
const SOFT_LIGHT: f32 = 0.003;
const SOFT_HEAVY: f32 = 0.01;
/// Bodies farther than this from the center of mass are recycled.
const ESCAPE: f32 = 2.6;

#[derive(Clone, Copy)]
struct Body {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    m: f32,
    hue: f32,
}

/// Gravitational N-body system: a few heavy suns dance around each other
/// while a swarm of light planets orbits them and gets flung around,
/// leaving trails. Every beat throws in a new sun, bass makes it heavier.
pub struct NBody {
    heavy: Vec<Body>,
    light: Vec<Body>,
    rng: Rng,
    hue: f32,
}

impl NBody {
    pub fn new() -> Self {
        let mut s = Self {
            heavy: Vec::new(),
            light: Vec::new(),
            rng: Rng::new(0x9B0D),
            hue: 0.0,
        };
        s.reset();
        s
    }

    /// Three suns on an equilateral triangle in a rotating Lagrange orbit.
    fn reset(&mut self) {
        self.heavy.clear();
        let r: f32 = 0.45;
        let v = (G / (3f32.sqrt() * r)).sqrt();
        for i in 0..3 {
            let a = i as f32 / 3.0 * TAU;
            self.heavy.push(Body {
                x: a.cos() * r,
                y: a.sin() * r,
                vx: -a.sin() * v,
                vy: a.cos() * v,
                m: 1.0,
                hue: i as f32 / 3.0,
            });
        }
        self.light.clear();
        for _ in 0..LIGHT {
            let b = self.planet();
            self.light.push(b);
        }
    }

    /// A light body on a roughly circular orbit around one sun or all of them.
    fn planet(&mut self) -> Body {
        let a = self.rng.range(0.0, TAU);
        let (cx, cy, cvx, cvy, m, r) = if self.rng.f32() < 0.6 && !self.heavy.is_empty() {
            let h = self.heavy[self.rng.next_u32() as usize % self.heavy.len()];
            (h.x, h.y, h.vx, h.vy, h.m, self.rng.range(0.08, 0.22))
        } else {
            let total: f32 = self.heavy.iter().map(|h| h.m).sum();
            (0.0, 0.0, 0.0, 0.0, total, self.rng.range(0.9, 1.5))
        };
        let v = (G * m / r).sqrt();
        Body {
            x: cx + a.cos() * r,
            y: cy + a.sin() * r,
            vx: cvx - a.sin() * v,
            vy: cvy + a.cos() * v,
            m: 0.0,
            hue: self.rng.f32(),
        }
    }

    fn step(&mut self, h: f32) {
        // Suns attract each other.
        let n = self.heavy.len();
        let mut acc = [(0.0f32, 0.0f32); MAX_HEAVY + 1];
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (self.heavy[i], self.heavy[j]);
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let d2 = dx * dx + dy * dy + SOFT_HEAVY;
                let inv = G / (d2 * d2.sqrt());
                acc[i].0 += dx * inv * b.m;
                acc[i].1 += dy * inv * b.m;
                acc[j].0 -= dx * inv * a.m;
                acc[j].1 -= dy * inv * a.m;
            }
        }
        for (b, a) in self.heavy.iter_mut().zip(&acc) {
            b.vx += a.0 * h;
            b.vy += a.1 * h;
            b.x += b.vx * h;
            b.y += b.vy * h;
        }
        // Planets only feel the suns.
        for p in &mut self.light {
            let (mut ax, mut ay) = (0.0, 0.0);
            for s in &self.heavy {
                let (dx, dy) = (s.x - p.x, s.y - p.y);
                let d2 = dx * dx + dy * dy + SOFT_LIGHT;
                let inv = G * s.m / (d2 * d2.sqrt());
                ax += dx * inv;
                ay += dy * inv;
            }
            p.vx += ax * h;
            p.vy += ay * h;
            p.x += p.vx * h;
            p.y += p.vy * h;
        }
    }
}

impl Effect for NBody {
    fn name(&self) -> &'static str {
        "N-Body"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.hue += dt * 0.02;

        if f.beat {
            self.hue += 0.03;
            let total: f32 = self.heavy.iter().map(|h| h.m).sum();
            let a = self.rng.range(0.0, TAU);
            let r = self.rng.range(0.7, 1.1);
            let v = (G * total / r).sqrt() * self.rng.range(0.6, 0.9);
            let m = 0.3 + 0.7 * f.bass;
            self.heavy.push(Body {
                x: a.cos() * r,
                y: a.sin() * r,
                vx: -a.sin() * v,
                vy: a.cos() * v,
                m,
                hue: self.rng.f32(),
            });
            if self.heavy.len() > MAX_HEAVY {
                self.heavy.remove(0);
            }
        }

        let unit = c.w.min(c.h) as f32 * 0.42;
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let old: Vec<(f32, f32)> = self.light.iter().map(|p| (p.x, p.y)).collect();
        let speed = 1.0 + 0.6 * f.level;
        let steps = ((dt * speed / STEP).ceil() as usize).min(MAX_STEPS);
        let h = if steps > 0 { dt * speed / steps as f32 } else { 0.0 };
        for _ in 0..steps {
            self.step(h);
        }

        // Keep the suns' center of mass at rest in the middle of the screen.
        let total: f32 = self.heavy.iter().map(|b| b.m).sum::<f32>().max(1e-6);
        let mut com = [0.0f32; 4];
        for b in &self.heavy {
            com[0] += b.x * b.m / total;
            com[1] += b.y * b.m / total;
            com[2] += b.vx * b.m / total;
            com[3] += b.vy * b.m / total;
        }
        for b in self.heavy.iter_mut().chain(self.light.iter_mut()) {
            b.x -= com[0];
            b.y -= com[1];
            b.vx -= com[2];
            b.vy -= com[3];
        }
        let far = |b: &Body| b.x * b.x + b.y * b.y > ESCAPE * ESCAPE || !b.x.is_finite() || !b.y.is_finite();
        if self.heavy.iter().any(far) {
            self.heavy.retain(|b| !far(b));
            if self.heavy.len() < 2 {
                self.reset();
            }
        }
        for i in 0..self.light.len() {
            if far(&self.light[i]) {
                self.light[i] = self.planet();
            }
        }

        c.fade(0.86 - 0.08 * f.pulse);
        let bright = 0.45 + 0.4 * f.treble;
        for (p, &(ox, oy)) in self.light.iter().zip(&old) {
            let sp = (p.vx * p.vx + p.vy * p.vy).sqrt();
            let col = hsv(self.hue + p.hue * 0.25 + sp * 0.15, 0.6, bright);
            let (x0, y0) = (cx + (ox - com[0]) * unit, cy + (oy - com[1]) * unit);
            let (x1, y1) = (cx + p.x * unit, cy + p.y * unit);
            if (x1 - x0).abs() + (y1 - y0).abs() < unit {
                c.line_add(x0, y0, x1, y1, scale(col, 0.6));
            }
            c.plot_add(x1 as i32, y1 as i32, col);
        }
        for s in &self.heavy {
            let r = (unit * 0.07 * s.m.sqrt()).max(1.0) * (1.0 + 0.3 * f.pulse);
            let (x, y) = (cx + s.x * unit, cy + s.y * unit);
            let ri = r.ceil() as i32;
            let col = hsv(self.hue + s.hue, 0.5, 1.0);
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let d = ((dx * dx + dy * dy) as f32).sqrt() / r;
                    if d < 1.0 {
                        let k = (1.0 - d) * (1.0 - d);
                        c.plot_add(x as i32 + dx, y as i32 + dy, scale(col, k));
                    }
                }
            }
        }
    }
}
