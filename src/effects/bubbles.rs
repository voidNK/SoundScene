use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, mix, rgb, scale};
use std::f32::consts::TAU;

const MAX_BUBBLES: usize = 320;
const MAX_POPS: usize = 40;
/// Sea floor line as a fraction of the height.
const FLOOR: f32 = 0.9;

/// Positions and radius in units of the canvas height.
struct Bubble {
    x: f32,
    y: f32,
    r: f32,
    phase: f32,
}

/// Expanding ring left by a popped bubble.
struct Pop {
    x: f32,
    y: f32,
    r: f32,
    age: f32,
}

fn hash(n: u32) -> f32 {
    let mut n = n.wrapping_mul(0x9E37_79B9);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 13;
    (n & 0xFFFF) as f32 / 65535.0
}

/// Underwater scene with shifting light rays and swaying weed: bubbles rise
/// from vents on the sea floor, wobbling and growing as they go. Each beat
/// pops the biggest bubbles and blasts a burst of new ones out of a vent;
/// bass speeds up the ascent and the level feeds the vents.
pub struct Bubbles {
    bubbles: Vec<Bubble>,
    pops: Vec<Pop>,
    time: f32,
    rng: Rng,
}

impl Bubbles {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xB0B_B1E5);
        let bubbles = (0..80)
            .map(|_| Bubble {
                x: rng.f32() * 2.0,
                y: rng.range(0.1, FLOOR),
                r: rng.range(0.006, 0.03),
                phase: rng.range(0.0, TAU),
            })
            .collect();
        Self {
            bubbles,
            pops: Vec::new(),
            time: 0.0,
            rng,
        }
    }

    /// Vent positions as fractions of the width.
    fn vent(i: usize) -> f32 {
        [0.18, 0.47, 0.71, 0.9][i % 4]
    }

    fn spawn(&mut self, aspect: f32, x: f32, r: f32) {
        if self.bubbles.len() < MAX_BUBBLES {
            let phase = self.rng.range(0.0, TAU);
            let jitter = self.rng.range(-0.03, 0.03);
            let lift = self.rng.range(0.0, 0.06);
            self.bubbles.push(Bubble {
                x: x * aspect + jitter,
                y: FLOOR - 0.01 - lift,
                r,
                phase,
            });
        }
    }
}

impl Effect for Bubbles {
    fn name(&self) -> &'static str {
        "Bubbles"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let now = self.time;
        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if c.h > 0 { w / h } else { 1.0 };

        if f.beat {
            // Pop the three biggest bubbles.
            for _ in 0..3 {
                let big = self.bubbles.iter().enumerate().max_by(|a, b| a.1.r.total_cmp(&b.1.r));
                if let Some((i, b)) = big
                    && b.r > 0.012
                {
                    if self.pops.len() < MAX_POPS {
                        self.pops.push(Pop {
                            x: b.x,
                            y: b.y,
                            r: b.r,
                            age: 0.0,
                        });
                    }
                    self.bubbles.swap_remove(i);
                }
            }
            let vent = Self::vent(self.rng.next_u32() as usize);
            for _ in 0..(10.0 + 25.0 * f.bass) as usize {
                let r = self.rng.range(0.004, 0.018);
                self.spawn(aspect, vent, r);
            }
        }
        let rate = (3.0 + 14.0 * f.level) * dt;
        let mut n = rate.floor() as usize + usize::from(self.rng.f32() < rate.fract());
        while n > 0 {
            let vent = Self::vent(self.rng.next_u32() as usize);
            let r = self.rng.range(0.005, 0.03);
            self.spawn(aspect, vent, r);
            n -= 1;
        }

        let lift = 0.12 + 0.3 * f.bass;
        for b in &mut self.bubbles {
            // Bigger bubbles rise faster and wobble more.
            b.y -= dt * lift * (0.6 + b.r * 25.0);
            b.x += dt * (now * 3.0 + b.phase).sin() * (0.02 + b.r * 1.5);
            b.r += dt * 0.002;
        }
        self.bubbles.retain(|b| b.y + b.r > 0.0);
        for p in &mut self.pops {
            p.age += dt;
        }
        self.pops.retain(|p| p.age < 0.4);

        if c.w == 0 || c.h == 0 {
            return;
        }

        // Water with light rays from the surface.
        let floor_y = h * FLOOR;
        for y in 0..c.h {
            let g = y as f32 / h;
            let water = mix(rgb(0.05, 0.35, 0.45), rgb(0.0, 0.05, 0.15), g.min(1.0));
            let fade = (1.0 - g).max(0.0).powi(2);
            for x in 0..c.w {
                let u = (x as f32 + y as f32 * 0.35) / h;
                let ray = ((u * 5.0 + now * 0.3).sin() * (u * 3.1 - now * 0.21).sin()).max(0.0);
                let col = mix(water, rgb(0.5, 0.85, 0.85), ray * ray * fade * (0.35 + 0.2 * f.mid));
                c.set(x, y, col);
            }
        }

        // Sandy floor with flickering caustics and swaying weed.
        for x in 0..c.w {
            let u = x as f32 / h;
            let top = floor_y - h * 0.03 * (u * 3.0).sin().abs();
            for y in (top.max(0.0) as usize).min(c.h)..c.h {
                let caustic = ((u * 17.0 + now).sin() * (y as f32 / h * 40.0 - now * 0.7).sin()).max(0.0);
                let col = mix(rgb(0.25, 0.25, 0.17), rgb(0.5, 0.55, 0.4), caustic * 0.5);
                c.set(x, y, col);
            }
        }
        let weeds = ((aspect * 7.0) as usize).max(1);
        for i in 0..weeds {
            let bx = (i as f32 + hash(i as u32) * 0.8) / weeds as f32 * w;
            let tall = h * (0.15 + 0.25 * hash(i as u32 + 50));
            let steps = tall.ceil().clamp(1.0, 400.0) as usize;
            for s in 0..steps {
                let v = s as f32 / steps as f32;
                let sway = (now * 1.2 + v * 3.0 + i as f32).sin() * v * h * (0.03 + 0.03 * f.bass);
                let x = bx + sway;
                let y = floor_y - v * tall;
                let col = mix(rgb(0.05, 0.2, 0.08), rgb(0.1, 0.4, 0.15), v);
                let thick = ((1.0 - v) * h / 60.0).max(0.0) as i32;
                for dx in -thick..=thick {
                    c.plot(x as i32 + dx, y as i32, col);
                }
            }
        }

        for b in &self.bubbles {
            let (bx, by) = (b.x * h, b.y * h);
            let r = b.r * h;
            if r < 1.2 {
                c.plot_add(
                    bx.round() as i32,
                    by.round() as i32,
                    scale(rgb(0.7, 0.95, 1.0), 0.25 + 0.3 * r),
                );
                continue;
            }
            let ri = r.ceil() as i32 + 1;
            let (cx, cy) = (bx.round() as i32, by.round() as i32);
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let (fx, fy) = (dx as f32 + cx as f32 - bx, dy as f32 + cy as f32 - by);
                    let d = (fx * fx + fy * fy).sqrt() / r;
                    if d > 1.15 {
                        continue;
                    }
                    // Bright rim, faint inside, highlight top left.
                    let rim = (-((d - 0.95) / 0.18).powi(2)).exp();
                    let hl = (-(((fx + r * 0.4).powi(2) + (fy + r * 0.4).powi(2)) / (r * r * 0.06))).exp();
                    let k = 0.05 + 0.4 * rim + 0.6 * hl;
                    c.plot_add(
                        cx + dx,
                        cy + dy,
                        scale(rgb(0.7, 0.95, 1.0), k * if d > 1.0 { 0.5 } else { 1.0 }),
                    );
                }
            }
        }

        for p in &self.pops {
            let k = 1.0 - p.age / 0.4;
            let r = (p.r * (1.0 + p.age * 6.0)) * h;
            let n = (r * 6.0).clamp(6.0, 120.0) as usize;
            for i in 0..n {
                if i % 2 == 1 {
                    continue;
                }
                let a = i as f32 / n as f32 * TAU;
                c.plot_add(
                    (p.x * h + a.cos() * r).round() as i32,
                    (p.y * h + a.sin() * r).round() as i32,
                    scale(rgb(0.8, 1.0, 1.0), k),
                );
            }
        }
    }
}
