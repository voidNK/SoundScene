use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, add, hsv, scale};

const OFF: u8 = 0;
const FIRING: u8 = 1;
const REFRACTORY: u8 = 2;

/// Brian's Brain: an off cell fires when exactly two neighbors fire, then
/// rests for one step, which sends swarms of glowing gliders across a
/// wrapping board. Loudness sets the speed, bass drops fresh noise into the
/// board and firing cells leave fading trails.
pub struct BriansBrain {
    cells: Vec<u8>,
    next: Vec<u8>,
    glow: Vec<f32>,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    rng: Rng,
    acc: f32,
    hue: f32,
}

impl BriansBrain {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            next: Vec::new(),
            glow: Vec::new(),
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            rng: Rng::new(0xB4A1),
            acc: 0.0,
            hue: 0.5,
        }
    }

    /// Fills a disc of radius `r` around a random point with firing noise.
    fn seed(&mut self, r: i32, density: f32) {
        let (gw, gh) = (self.gw as i32, self.gh as i32);
        let cx = (self.rng.next_u32() % gw as u32) as i32;
        let cy = (self.rng.next_u32() % gh as u32) as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r && self.rng.f32() < density {
                    let x = (cx + dx).rem_euclid(gw) as usize;
                    let y = (cy + dy).rem_euclid(gh) as usize;
                    self.cells[y * self.gw + x] = FIRING;
                }
            }
        }
    }

    /// Returns the number of firing cells after the step.
    fn step(&mut self) -> usize {
        let (w, h) = (self.gw, self.gh);
        let mut firing = 0;
        for y in 0..h {
            let (up, down) = (((y + h - 1) % h) * w, ((y + 1) % h) * w);
            let row = y * w;
            for x in 0..w {
                let i = row + x;
                self.next[i] = match self.cells[i] {
                    FIRING => REFRACTORY,
                    REFRACTORY => OFF,
                    _ => {
                        let (l, r) = ((x + w - 1) % w, (x + 1) % w);
                        let cells = &self.cells;
                        let on = |j: usize| (cells[j] == FIRING) as u8;
                        let n = on(up + l)
                            + on(up + x)
                            + on(up + r)
                            + on(row + l)
                            + on(row + r)
                            + on(down + l)
                            + on(down + x)
                            + on(down + r);
                        if n == 2 {
                            firing += 1;
                            FIRING
                        } else {
                            OFF
                        }
                    }
                };
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        for (g, &s) in self.glow.iter_mut().zip(&self.cells) {
            if s == FIRING {
                *g = 1.0;
            }
        }
        firing
    }
}

impl Effect for BriansBrain {
    fn name(&self) -> &'static str {
        "Brian's Brain"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.scale = if c.w >= 160 && c.h >= 80 { 2 } else { 1 };
            self.gw = c.w.div_ceil(self.scale);
            self.gh = c.h.div_ceil(self.scale);
            let n = self.gw * self.gh;
            self.cells = vec![OFF; n];
            self.next = vec![OFF; n];
            self.glow = vec![0.0; n];
            if self.gw >= 3 && self.gh >= 3 {
                for _ in 0..4 {
                    let r = (self.gw.min(self.gh) / 5).max(2) as i32;
                    self.seed(r, 0.35);
                }
            }
        }
        if self.gw < 3 || self.gh < 3 {
            c.clear();
            return;
        }

        self.hue += dt * 0.02;
        let r = (self.gw.min(self.gh) as f32 * (0.06 + 0.12 * f.bass)) as i32;
        if f.beat {
            self.seed(r.max(2), 0.3 + 0.2 * f.bass);
        }
        let decay = (-dt * 4.0).exp();
        for g in &mut self.glow {
            *g *= decay;
        }
        self.acc = (self.acc + dt * (10.0 + f.level * 18.0 + f.bass * 10.0)).min(3.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            // Keep the board alive: a quiet brain gets a jolt.
            if self.step() < self.gw * self.gh / 300 + 2 {
                self.seed(r.max(2), 0.35);
            }
        }

        let fire = add(hsv(self.hue, 0.25, 0.8 + 0.2 * f.pulse), [40, 40, 40]);
        let rest = hsv(self.hue + 0.08, 0.8, 0.75);
        let s = self.scale;
        for y in 0..c.h {
            let row = (y / s) * self.gw;
            for x in 0..c.w {
                let i = row + x / s;
                let col = match self.cells[i] {
                    FIRING => fire,
                    REFRACTORY => rest,
                    _ => {
                        let g = self.glow[i];
                        scale(hsv(self.hue + 0.25 - 0.2 * g, 0.9, 1.0), g * (0.3 + 0.6 * g))
                    }
                };
                c.set(x, y, col);
            }
        }
    }
}
