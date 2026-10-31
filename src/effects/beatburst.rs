use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};
use std::f32::consts::TAU;

const MAX_SPARKS: usize = 4000;
/// Gravity in units of the smaller canvas side per second².
const GRAVITY: f32 = 0.8;

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    hue: f32,
}

/// Every beat fires a radial particle explosion colored by the dominant
/// band (red bass, green mids, blue treble); sparks fall with gravity and
/// cool down from white-hot as they fade. Louder bass means bigger bursts.
pub struct BeatBurst {
    sparks: Vec<Spark>,
    rng: Rng,
    idle: f32,
}

impl BeatBurst {
    pub fn new() -> Self {
        Self {
            sparks: Vec::new(),
            rng: Rng::new(0xB00B5),
            idle: 10.0,
        }
    }

    /// Spawns one explosion; positions are in units of the smaller canvas
    /// side, relative to the canvas center.
    fn burst(&mut self, strength: f32, aspect: f32, density: f32, f: &Features) {
        let hue = if f.bass >= f.mid && f.bass >= f.treble {
            0.02
        } else if f.mid >= f.treble {
            0.3
        } else {
            0.6
        };
        let x = self.rng.range(-0.35, 0.35) * aspect;
        let y = self.rng.range(-0.25, 0.05);
        let count = ((60.0 + 160.0 * strength) * density) as usize;
        for _ in 0..count {
            let a = self.rng.range(0.0, TAU);
            let s = self.rng.range(0.15, 1.0).sqrt() * (0.35 + 0.6 * strength);
            let life = self.rng.range(0.7, 1.8);
            self.sparks.push(Spark {
                x,
                y,
                vx: a.cos() * s,
                vy: a.sin() * s,
                life,
                max_life: life,
                hue: hue + self.rng.range(-0.05, 0.05),
            });
        }
        if self.sparks.len() > MAX_SPARKS {
            let extra = self.sparks.len() - MAX_SPARKS;
            self.sparks.drain(..extra);
        }
    }
}

impl Effect for BeatBurst {
    fn name(&self) -> &'static str {
        "Beat Burst"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fade(0.7);
        let unit = c.w.min(c.h) as f32;
        let aspect = if unit > 0.0 { c.w as f32 / unit } else { 1.0 };
        // Fewer sparks on small tiles so bursts stay readable.
        let density = (unit / 100.0).clamp(0.3, 1.0);
        self.idle += dt;
        if f.beat {
            self.burst(0.5 + 0.5 * f.bass, aspect, density, f);
            self.idle = 0.0;
        } else if self.idle > 1.4 {
            // Small fireworks keep the screen alive without beats.
            self.burst(0.15 + 0.3 * f.level, aspect, density, f);
            self.idle = 0.0;
        }

        let drag = (1.0 - 1.2 * dt).max(0.0);
        let bottom = 0.5 * c.h as f32 / unit.max(1.0) + 0.05;
        for s in &mut self.sparks {
            s.vy += GRAVITY * dt;
            s.vx *= drag;
            s.vy *= drag;
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.life -= dt;
        }
        self.sparks.retain(|s| s.life > 0.0 && s.y < bottom);
        if c.w == 0 || c.h == 0 {
            return;
        }

        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let big = unit >= 60.0;
        for s in &self.sparks {
            let l = (s.life / s.max_life).clamp(0.0, 1.0);
            let col = hsv(s.hue + (1.0 - l) * 0.06, 0.25 + 0.7 * (1.0 - l), l.sqrt());
            let x1 = cx + s.x * unit;
            let y1 = cy + s.y * unit;
            // Short motion streak behind each spark.
            let x0 = x1 - s.vx * unit * 0.04;
            let y0 = y1 - s.vy * unit * 0.04;
            c.line_add(x0, y0, x1, y1, col);
            if big && l > 0.5 {
                c.plot_add(x1 as i32 + 1, y1 as i32, col);
                c.plot_add(x1 as i32, y1 as i32 + 1, col);
            }
        }
    }
}
