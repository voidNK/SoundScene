use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, mix, rgb, scale};
use std::f32::consts::TAU;

const STARS: usize = 500;
const MAX_METEORS: usize = 24;

struct Star {
    /// Fractions of the canvas.
    x: f32,
    y: f32,
    bright: f32,
    phase: f32,
    bin: usize,
}

/// Positions and speeds in units of the canvas height.
struct Meteor {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    tail: f32,
    bright: f32,
}

fn hash(n: u32) -> f32 {
    let mut n = n.wrapping_mul(0x9E37_79B9);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 13;
    (n & 0xFFFF) as f32 / 65535.0
}

/// Smooth 1D value noise.
fn noise(x: f32, seed: u32) -> f32 {
    let i = x.floor();
    let t = x - i;
    let t = t * t * (3.0 - 2.0 * t);
    let a = hash((i as i32 as u32).wrapping_add(seed.wrapping_mul(7919)));
    let b = hash((i as i32 as u32).wrapping_add(1).wrapping_add(seed.wrapping_mul(7919)));
    a + (b - a) * t
}

/// A clear night sky full of twinkling stars above a hilly horizon with
/// trees. Every beat sends shooting stars streaking across, more and brighter
/// with more bass; the stars twinkle with their own spectrum band.
pub struct Meteors {
    stars: Vec<Star>,
    meteors: Vec<Meteor>,
    flash: f32,
    time: f32,
    rng: Rng,
}

impl Meteors {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x3E7E_0125);
        let stars = (0..STARS)
            .map(|i| {
                let b = rng.f32();
                Star {
                    x: rng.f32(),
                    // Denser towards the top.
                    y: rng.f32().powf(1.4) * 0.85,
                    bright: 0.15 + 0.85 * b * b * b,
                    phase: rng.range(0.0, TAU),
                    bin: i % 64,
                }
            })
            .collect();
        Self {
            stars,
            meteors: Vec::new(),
            flash: 0.0,
            time: 0.0,
            rng,
        }
    }

    fn spawn(&mut self, aspect: f32, strength: f32) {
        if self.meteors.len() >= MAX_METEORS {
            return;
        }
        let r = &mut self.rng;
        let dir = if r.f32() < 0.5 { 1.0 } else { -1.0 };
        let angle = r.range(0.25, 0.7);
        let speed = r.range(0.9, 1.6) * (0.8 + 0.5 * strength);
        self.meteors.push(Meteor {
            x: r.range(0.1, 0.9) * aspect.max(0.1),
            y: r.range(0.02, 0.35),
            vx: angle.cos() * speed * dir,
            vy: angle.sin() * speed,
            age: 0.0,
            life: r.range(0.5, 1.0),
            tail: r.range(0.12, 0.25) * (0.8 + 0.4 * strength),
            bright: 0.6 + 0.4 * strength,
        });
    }
}

impl Effect for Meteors {
    fn name(&self) -> &'static str {
        "Meteors"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if c.h > 0 { w / h } else { 1.0 };
        if f.beat {
            let n = 1 + (f.bass * 2.5) as usize;
            for _ in 0..n {
                self.spawn(aspect, f.bass);
            }
            self.flash = 0.4 + 0.6 * f.bass;
        }
        // An occasional meteor even in silence.
        if self.rng.f32() < dt * 0.3 {
            self.spawn(aspect, 0.2);
        }
        self.flash *= (-dt * 4.0).exp();
        for m in &mut self.meteors {
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            m.age += dt;
        }
        self.meteors.retain(|m| m.age < m.life);

        if c.w == 0 || c.h == 0 {
            return;
        }

        let horizon = |x: usize| {
            let u = x as f32 / h.max(1.0);
            let hill = 0.8 - 0.1 * noise(u * 2.0, 1) - 0.05 * noise(u * 6.0, 2);
            // Trees poke up from the hills in clumps.
            let clump = noise(u * 4.0, 3);
            let tree = if clump > 0.55 {
                let cell = (u * 40.0).floor();
                let tx = (u * 40.0).fract();
                let th = hash(cell as i32 as u32 + 99) * 0.07 * (clump - 0.55) / 0.45;
                th * (1.0 - (tx - 0.5).abs() * 2.0).max(0.0)
            } else {
                0.0
            };
            (hill - tree) * h
        };

        let glow = 0.02 * self.flash;
        for y in 0..c.h {
            let g = y as f32 / h;
            let sky = mix(
                rgb(0.0, 0.0, 0.03 + glow),
                rgb(0.08 + glow, 0.06, 0.18 + glow),
                g * g / 0.64,
            );
            for x in 0..c.w {
                c.set(x, y, sky);
            }
        }

        // Faint band of the milky way.
        for x in 0..c.w {
            let u = x as f32 / w.max(1.0);
            let cy = h * (0.5 - 0.35 * u);
            let band = h * 0.08;
            for y in ((cy - band * 2.0).max(0.0) as usize).min(c.h)..((cy + band * 2.0).max(0.0) as usize).min(c.h) {
                let d = (y as f32 - cy) / band;
                let k = 0.06 * (-d * d).exp() * (0.6 + 0.4 * hash((x * 131 + y * 7) as u32));
                let p = c.get(x, y);
                c.set(x, y, mix(p, rgb(0.5, 0.5, 0.65), k));
            }
        }

        let spec = &f.spectrum;
        // Keep the density similar on small canvases.
        let visible = ((w * h / 50.0) as usize).clamp(30, STARS);
        for s in self.stars.iter().take(visible) {
            let band = spec.get(s.bin).copied().unwrap_or(0.0);
            let tw = 0.65 + 0.35 * (self.time * (2.0 + 3.0 * f.treble) + s.phase).sin();
            let b = (s.bright * tw * (0.7 + 0.6 * band)).min(1.0);
            let (x, y) = ((s.x * w) as i32, (s.y * h) as i32);
            let warm = (s.phase * 3.0).sin() * 0.15;
            c.plot_add(x, y, rgb(b * (0.9 + warm), b * 0.92, b * (0.95 - warm)));
            if s.bright > 0.7 && h > 30.0 {
                let soft = rgb(b * 0.25, b * 0.25, b * 0.3);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    c.plot_add(x + dx, y + dy, soft);
                }
            }
        }

        for m in &self.meteors {
            let fade = (1.0 - m.age / m.life).clamp(0.0, 1.0) * (m.age * 10.0).min(1.0);
            let speed = (m.vx * m.vx + m.vy * m.vy).sqrt().max(1e-3);
            let (ux, uy) = (m.vx / speed, m.vy / speed);
            let len = m.tail * h;
            let steps = (len * 1.5).ceil().clamp(2.0, 600.0) as i32;
            for i in 0..=steps {
                let k = i as f32 / steps as f32;
                let x = m.x * h - ux * len * k;
                let y = m.y * h - uy * len * k;
                let b = m.bright * fade * (1.0 - k).powf(1.5);
                let col = mix(rgb(1.0, 1.0, 0.95), rgb(0.4, 0.6, 1.0), k);
                let (xi, yi) = (x.round() as i32, y.round() as i32);
                c.plot_add(xi, yi, scale(col, b));
                // A softer second line makes the streak wider on big canvases.
                if h > 40.0 {
                    c.plot_add(xi, yi + 1, scale(col, b * 0.35));
                }
            }
            let (hx, hy) = ((m.x * h).round() as i32, (m.y * h).round() as i32);
            let r = (h / 60.0).clamp(1.0, 3.0) as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let d2 = (dx * dx + dy * dy) as f32;
                    let k = m.bright * fade * (-d2 / (r * r) as f32).exp() * 0.8;
                    c.plot_add(hx + dx, hy + dy, scale(rgb(1.0, 0.95, 0.85), k));
                }
            }
        }

        // Horizon silhouette in front of everything.
        let ground = rgb(0.005, 0.008, 0.015);
        for x in 0..c.w {
            let top = horizon(x);
            for y in (top.max(0.0) as usize).min(c.h)..c.h {
                c.set(x, y, ground);
            }
        }
    }
}
