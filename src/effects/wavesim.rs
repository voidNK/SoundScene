use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

/// Pixels per simulation cell along each axis.
const CELL: usize = 2;
const STEPS_PER_SEC: f32 = 90.0;
const MAX_STEPS: usize = 6;
const SOURCES: usize = 3;

struct Source {
    phase: f32,
    /// Spectrum band that drives this emitter.
    band: usize,
    amp: f32,
}

/// 2D wave equation in a walled tank: a few emitters sing their spectrum
/// band and interfere into moiré fringes, beats and spectrum peaks drop
/// splashes in. Crests catch the light, troughs fall into deep water.
pub struct WaveSim {
    cur: Vec<f32>,
    prev: Vec<f32>,
    colors: Vec<[f32; 3]>,
    gw: usize,
    gh: usize,
    sources: Vec<Source>,
    rng: Rng,
    acc: f32,
    time: f32,
    hue: f32,
}

impl WaveSim {
    pub fn new() -> Self {
        Self {
            cur: Vec::new(),
            prev: Vec::new(),
            colors: Vec::new(),
            gw: 0,
            gh: 0,
            sources: Vec::new(),
            rng: Rng::new(0x3A7E),
            acc: 0.0,
            time: 0.0,
            hue: 0.0,
        }
    }

    /// Adds a smooth bump of height `amp` and radius `r` cells.
    fn splash(&mut self, x: f32, y: f32, r: f32, amp: f32) {
        let (gw, gh) = (self.gw as i32, self.gh as i32);
        let ri = r.ceil() as i32;
        let (cx, cy) = (x as i32, y as i32);
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let (px, py) = (cx + dx, cy + dy);
                if px <= 0 || py <= 0 || px >= gw - 1 || py >= gh - 1 {
                    continue;
                }
                let d2 = (dx * dx + dy * dy) as f32 / (r * r).max(0.25);
                if d2 < 1.0 {
                    let k = 1.0 - d2;
                    self.cur[py as usize * self.gw + px as usize] += amp * k * k;
                }
            }
        }
    }

    fn step(&mut self) {
        let (gw, gh) = (self.gw, self.gh);
        // `prev` becomes the next state; borders stay at 0 (reflecting walls).
        for y in 1..gh - 1 {
            for x in 1..gw - 1 {
                let i = y * gw + x;
                let n = self.cur[i - 1] + self.cur[i + 1] + self.cur[i - gw] + self.cur[i + gw];
                self.prev[i] = (n * 0.5 - self.prev[i]) * 0.993;
            }
        }
        std::mem::swap(&mut self.cur, &mut self.prev);
    }
}

impl Effect for WaveSim {
    fn name(&self) -> &'static str {
        "Wave Sim"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        if (gw, gh) != (self.gw, self.gh) {
            (self.gw, self.gh) = (gw, gh);
            self.cur = vec![0.0; gw * gh];
            self.prev = vec![0.0; gw * gh];
            self.colors = vec![[0.0; 3]; gw * gh];
            self.sources.clear();
            for i in 0..SOURCES {
                self.sources.push(Source {
                    phase: 0.0,
                    band: 2 + i * 9,
                    amp: 0.0,
                });
            }
        }
        if gw < 3 || gh < 3 {
            c.fill(hsv(0.58, 0.8, 0.2));
            return;
        }
        self.hue += dt * 0.01;
        self.time += dt;
        let min = gw.min(gh) as f32;

        if f.beat {
            self.hue += 0.03;
            let (x, y) = (
                self.rng.range(0.1, 0.9) * gw as f32,
                self.rng.range(0.1, 0.9) * gh as f32,
            );
            self.splash(x, y, (min * 0.06).max(1.0), 3.0 + 4.0 * f.bass);
        }
        // Strong spectrum bins drip in at their position along the x axis.
        let bins = f.spectrum.len();
        for (b, &v) in f.spectrum.iter().enumerate() {
            if v > 0.8 && self.rng.f32() < (v - 0.8) * 0.12 {
                let x = (b as f32 + 0.5) / bins as f32 * gw as f32;
                let y = self.rng.range(0.15, 0.85) * gh as f32;
                self.splash(x, y, (min * 0.03).max(1.0), 1.0 + v);
            }
        }
        // Gentle rain so silence still ripples.
        if self.rng.f32() < dt * 0.8 {
            let (x, y) = (self.rng.f32() * gw as f32, self.rng.f32() * gh as f32);
            self.splash(x, y, (min * 0.04).max(1.0), 1.5);
        }

        self.acc = (self.acc + dt * STEPS_PER_SEC).min(MAX_STEPS as f32);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let t = self.time;
            for i in 0..self.sources.len() {
                let s = &mut self.sources[i];
                let level = f.spectrum.get(s.band).copied().unwrap_or(0.0);
                s.amp += (0.25 + 0.9 * level - s.amp) * 0.05;
                s.phase += 0.35 + 0.08 * i as f32;
                // Emitters wander slowly around the tank.
                let k = i as f32 * 2.1;
                let x = (0.5 + 0.33 * (t * 0.07 + k).sin()) * gw as f32;
                let y = (0.5 + 0.33 * (t * 0.05 + k * 1.3).cos()) * gh as f32;
                let v = s.phase.sin() * s.amp * 0.6;
                let xi = (x as usize).clamp(1, gw - 2);
                let yi = (y as usize).clamp(1, gh - 2);
                self.cur[yi * gw + xi] += v;
            }
            self.step();
        }

        // Shade: slope as light from the upper left, height as depth.
        let bright = 0.85 + 0.3 * f.pulse;
        for y in 0..gh {
            for x in 0..gw {
                let i = y * gw + x;
                let h = self.cur[i];
                let sx = if x > 0 && x + 1 < gw {
                    self.cur[i + 1] - self.cur[i - 1]
                } else {
                    0.0
                };
                let sy = if y > 0 && y + 1 < gh {
                    self.cur[i + gw] - self.cur[i - gw]
                } else {
                    0.0
                };
                let light = (-(sx + sy) * 0.9).clamp(-1.0, 1.5);
                let crest = (h * 0.6).clamp(-1.0, 1.0);
                let base = hsv(
                    0.55 + 0.07 * (self.hue * 6.0).sin() + 0.05 * crest,
                    0.85 - 0.35 * crest.max(0.0),
                    0.25 + 0.2 * crest,
                );
                let spec = light.max(0.0).powi(2) * bright;
                self.colors[i] = [
                    base[0] as f32 / 255.0 + spec * 0.8 + light * 0.05,
                    base[1] as f32 / 255.0 + spec * 0.9 + light * 0.08,
                    base[2] as f32 / 255.0 + spec + light * 0.12,
                ];
            }
        }
        upscale(c, &self.colors, gw, gh);
    }
}
