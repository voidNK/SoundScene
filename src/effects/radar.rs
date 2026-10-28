use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, rgb};
use std::f32::consts::TAU;

const MAX_BLIPS: usize = 120;

struct Blip {
    /// Distance from the center as a fraction of the screen radius.
    r: f32,
    a: f32,
    drift: f32,
    strength: f32,
    age: f32,
}

fn hash(x: usize, y: usize) -> f32 {
    let mut n = (x as u32).wrapping_mul(0x27D4_EB2D) ^ (y as u32).wrapping_mul(0x1656_67B1);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 13;
    (n & 0xFFFF) as f32 / 65535.0
}

/// Green phosphor radar screen: a rotating sweep with a fading afterglow
/// over range rings and a crosshair. Spectrum peaks become contacts at a
/// range set by their frequency, beats add a burst of contacts, and bass
/// lights up ground clutter around the center. Contacts glow when the sweep
/// passes and fade until the next turn.
pub struct Radar {
    sweep: f32,
    blips: Vec<Blip>,
    rng: Rng,
}

impl Radar {
    pub fn new() -> Self {
        Self {
            sweep: 0.0,
            blips: Vec::new(),
            rng: Rng::new(0x5A_DA2),
        }
    }

    fn spawn(&mut self, r: f32, strength: f32) {
        if self.blips.len() < MAX_BLIPS {
            self.blips.push(Blip {
                r,
                a: self.sweep - self.rng.range(0.0, 0.15),
                drift: self.rng.range(-0.03, 0.03),
                strength,
                age: 0.0,
            });
        }
    }
}

fn phosphor(i: f32) -> [u8; 3] {
    rgb(0.1 * i + 0.5 * i * i * i, i, 0.3 * i + 0.2 * i * i)
}

impl Effect for Radar {
    fn name(&self) -> &'static str {
        "Radar"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        let speed = TAU / (2.6 - 1.2 * f.mid);
        self.sweep = (self.sweep + dt * speed).rem_euclid(TAU);

        let bins = f.spectrum.len();
        for b in 1..bins.saturating_sub(1) {
            let s = f.spectrum[b];
            if s > 0.7 && s >= f.spectrum[b - 1] && s >= f.spectrum[b + 1] && self.rng.f32() < 0.06 {
                self.spawn(0.12 + 0.82 * b as f32 / bins as f32, 0.5 + 0.5 * s);
            }
        }
        if f.beat {
            for _ in 0..2 + self.rng.next_u32() % 3 {
                let r = self.rng.range(0.15, 0.95);
                self.spawn(r, 1.0);
            }
        }
        let life = 2.0 * TAU / speed;
        for b in &mut self.blips {
            b.age += dt;
            b.a += b.drift * dt;
        }
        self.blips.retain(|b| b.age < life);

        if c.w == 0 || c.h == 0 {
            return;
        }
        let (cx, cy) = (c.w as f32 * 0.5 - 0.5, c.h as f32 * 0.5 - 0.5);
        let rad = (c.w.min(c.h) as f32 * 0.47).max(1.0);
        let sweep = self.sweep;
        let glow = 0.55 + 0.35 * f.level;

        for y in 0..c.h {
            let dy = y as f32 - cy;
            for x in 0..c.w {
                let dx = x as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt();
                if r > rad + 0.5 {
                    c.set(x, y, rgb(0.01, 0.025, 0.015));
                    continue;
                }
                let behind = (sweep - dy.atan2(dx)).rem_euclid(TAU);
                let mut i = 0.035 + glow * (-behind * 2.2).exp() * 0.5;
                // Sweep line: about one pixel wide at any radius.
                if behind * r < 1.0 || (TAU - behind) * r < 0.5 {
                    i += 0.6;
                }
                let rf = r / rad;
                if ((rf * 4.0).round() - rf * 4.0).abs() * rad < 0.6 || dx.abs() < 0.5 || dy.abs() < 0.5 {
                    i += 0.12;
                }
                if r > rad - 1.0 {
                    i += 0.3;
                }
                if rf < 0.35 && hash(x, y) > 0.82 {
                    i += f.bass * 0.7 * (1.0 - rf / 0.35) * (-behind * 1.2).exp();
                }
                c.set(x, y, phosphor(i.min(1.0)));
            }
        }

        for b in &self.blips {
            let behind = (sweep - b.a).rem_euclid(TAU);
            let fade = 1.0 - b.age / life;
            let v = b.strength * (-behind * 0.8).exp() * fade;
            if v < 0.03 {
                continue;
            }
            let (x, y) = (
                (cx + b.r * rad * b.a.cos()).round() as i32,
                (cy + b.r * rad * b.a.sin()).round() as i32,
            );
            let core = phosphor(v.min(1.0));
            let halo = phosphor((v * 0.45).min(1.0));
            c.plot_add(x, y, core);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                c.plot_add(x + dx, y + dy, halo);
            }
        }
    }
}
