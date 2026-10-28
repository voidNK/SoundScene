use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};
use std::f32::consts::TAU;

const MAX_PARTICLES: usize = 4000;

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    hue: f32,
}

/// A fountain fed by the loudness, plus a firework burst on every beat.
pub struct Particles {
    particles: Vec<Particle>,
    rng: Rng,
    hue: f32,
    emit: f32,
}

impl Particles {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            rng: Rng::new(0xB00),
            hue: 0.0,
            emit: 0.0,
        }
    }
}

impl Effect for Particles {
    fn name(&self) -> &'static str {
        "Particles"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fade(0.75);
        let (w, h) = (c.w as f32, c.h as f32);
        // Velocities scale with the canvas so mosaic tiles look the same.
        let unit = h.max(1.0);

        if f.beat {
            self.hue += 0.13;
            let (bx, by) = (self.rng.range(0.2, 0.8) * w, self.rng.range(0.15, 0.5) * h);
            for _ in 0..150 {
                let a = self.rng.range(0.0, TAU);
                let speed = self.rng.range(0.2, 1.0) * unit * (0.6 + f.bass);
                self.particles.push(Particle {
                    x: bx,
                    y: by,
                    vx: a.cos() * speed,
                    vy: a.sin() * speed,
                    life: 1.0,
                    hue: self.hue + self.rng.range(0.0, 0.1),
                });
            }
        }

        self.emit += dt * (30.0 + 300.0 * f.level);
        while self.emit >= 1.0 {
            self.emit -= 1.0;
            self.particles.push(Particle {
                x: w * 0.5,
                y: h - 1.0,
                vx: self.rng.range(-0.25, 0.25) * unit,
                vy: -self.rng.range(0.8, 1.3) * unit * (0.8 + 0.4 * f.bass),
                life: 1.0,
                hue: self.hue + 0.5,
            });
        }

        for p in &mut self.particles {
            p.vy += unit * 0.9 * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt * 0.5;
            c.plot_add(p.x as i32, p.y as i32, hsv(p.hue, 0.6, p.life.max(0.0) * 0.6));
        }
        self.particles
            .retain(|p| p.life > 0.0 && p.y < h + 2.0 && p.x > -2.0 && p.x < w + 2.0);
        let excess = self.particles.len().saturating_sub(MAX_PARTICLES);
        self.particles.drain(..excess);
    }
}
