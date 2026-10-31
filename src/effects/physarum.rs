use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, rgb};
use std::f32::consts::TAU;

const MAX_AGENTS: usize = 9000;
const STEPS_PER_SEC: f32 = 60.0;
const MAX_STEPS: usize = 4;

struct Agent {
    x: f32,
    y: f32,
    a: f32,
}

/// Physarum slime mold: thousands of agents sense the trail map ahead,
/// steer towards the strongest scent, move and deposit, while the map
/// diffuses and decays into glowing vein networks. Bass boosts the
/// deposit, treble widens the sensors, beats scatter the agents' headings.
pub struct Physarum {
    agents: Vec<Agent>,
    trail: Vec<f32>,
    tmp: Vec<f32>,
    w: usize,
    h: usize,
    rng: Rng,
    acc: f32,
    hue: f32,
}

impl Physarum {
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            trail: Vec::new(),
            tmp: Vec::new(),
            w: 0,
            h: 0,
            rng: Rng::new(0x5713),
            acc: 0.0,
            hue: 0.0,
        }
    }

    fn sense(&self, x: f32, y: f32) -> f32 {
        let xi = (x as i32).rem_euclid(self.w as i32) as usize;
        let yi = (y as i32).rem_euclid(self.h as i32) as usize;
        self.trail[yi * self.w + xi]
    }

    fn step(&mut self, sensor: f32, angle: f32, deposit: f32, decay: f32) {
        let (w, h) = (self.w as f32, self.h as f32);
        let turn = 0.45;
        let (s_ang, c_ang) = angle.sin_cos();
        for i in 0..self.agents.len() {
            let Agent { x, y, a } = self.agents[i];
            // Left and right sensor directions by rotating the heading.
            let (sa, ca) = a.sin_cos();
            let m = self.sense(x + ca * sensor, y + sa * sensor);
            let l = self.sense(
                x + (ca * c_ang - sa * s_ang) * sensor,
                y + (sa * c_ang + ca * s_ang) * sensor,
            );
            let r = self.sense(
                x + (ca * c_ang + sa * s_ang) * sensor,
                y + (sa * c_ang - ca * s_ang) * sensor,
            );
            let mut a = a + self.rng.range(-0.1, 0.1);
            if m < l || m < r {
                if l > r || (l == r && self.rng.f32() < 0.5) {
                    a += turn;
                } else {
                    a -= turn;
                }
            }
            let (sa, ca) = a.sin_cos();
            let wrap = |v: f32, len: f32| {
                if v < 0.0 {
                    v + len
                } else if v >= len {
                    v - len
                } else {
                    v
                }
            };
            let (x, y) = (wrap(x + ca, w), wrap(y + sa, h));
            self.agents[i] = Agent { x, y, a };
            let idx = (y as usize).min(self.h - 1) * self.w + (x as usize).min(self.w - 1);
            self.trail[idx] = (self.trail[idx] + deposit).min(6.0);
        }

        // Separable 3×3 box blur mixed with the original, then decay.
        let (w, h) = (self.w, self.h);
        for y in 0..h {
            let row = &self.trail[y * w..(y + 1) * w];
            let out = &mut self.tmp[y * w..(y + 1) * w];
            for x in 0..w {
                let l = if x == 0 { w - 1 } else { x - 1 };
                let r = if x + 1 == w { 0 } else { x + 1 };
                out[x] = row[l] + row[x] + row[r];
            }
        }
        for y in 0..h {
            let up = if y == 0 { h - 1 } else { y - 1 } * w;
            let down = if y + 1 == h { 0 } else { y + 1 } * w;
            let row = y * w;
            for x in 0..w {
                let sum = self.tmp[up + x] + self.tmp[row + x] + self.tmp[down + x];
                let t = &mut self.trail[row + x];
                *t = (*t * 0.4 + sum / 9.0 * 0.6) * decay;
            }
        }
    }
}

impl Effect for Physarum {
    fn name(&self) -> &'static str {
        "Physarum"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if (self.w, self.h) != (c.w, c.h) {
            (self.w, self.h) = (c.w, c.h);
            self.trail = vec![0.0; c.w * c.h];
            self.tmp = vec![0.0; c.w * c.h];
            self.agents.clear();
            let n = (c.w * c.h / 5).clamp(1, MAX_AGENTS);
            for _ in 0..n {
                self.agents.push(Agent {
                    x: self.rng.f32() * c.w as f32,
                    y: self.rng.f32() * c.h as f32,
                    a: self.rng.range(0.0, TAU),
                });
            }
        }
        self.hue += dt * 0.015;
        if f.beat {
            self.hue += 0.04;
            let spread = 0.5 + 1.5 * f.bass;
            for ag in &mut self.agents {
                ag.a += self.rng.range(-spread, spread);
            }
        }

        let scale = c.w.min(c.h) as f32;
        let sensor = (scale * (0.05 + 0.04 * f.treble)).clamp(1.5, 12.0);
        let angle = 0.6 + 0.3 * f.mid;
        let deposit = 0.35 + 1.2 * f.bass + 0.6 * f.pulse;
        self.acc = (self.acc + dt * STEPS_PER_SEC).min(MAX_STEPS as f32);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.step(sensor, angle, deposit, 0.9);
        }

        // Palette: deep violet → magenta → amber → white, slowly rotating.
        let gain = 0.9 + 0.5 * f.pulse;
        let (sh, ch) = (self.hue * TAU).sin_cos();
        for (p, &v) in c.px.iter_mut().zip(&self.trail) {
            let k = 1.0 - (-v * 0.9 * gain).exp();
            let r = k * (1.2 + 0.3 * sh);
            let g = k * k * (1.1 + 0.3 * ch);
            let b = 0.05 + k.sqrt() * (0.75 - 0.25 * sh) * (1.0 - k * 0.5);
            *p = rgb(r, g, b + k * k * k);
        }
    }
}
