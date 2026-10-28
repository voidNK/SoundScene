use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, mix, rgb, scale};
use std::f32::consts::TAU;

const FLIES: usize = 150;
const CLUSTERS: usize = 5;
/// Meadow line as a fraction of the height.
const MEADOW: f32 = 0.74;

struct Fly {
    cluster: usize,
    /// Offset from the cluster center, fractions of the height.
    ox: f32,
    oy: f32,
    phase: f32,
    glow: f32,
    /// When a scheduled flash fires.
    flash_at: Option<f32>,
    /// Drawn behind the front grass.
    low: bool,
}

struct Tree {
    x: f32,
    height: f32,
    radius: f32,
    pine: bool,
}

fn hash(n: u32) -> f32 {
    let mut n = n.wrapping_mul(0x9E37_79B9);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 13;
    (n & 0xFFFF) as f32 / 65535.0
}

/// A meadow at the edge of a forest by moonlight, full of drifting fireflies
/// in loose swarms. Each beat makes one swarm flash together, the light
/// running through it from its center outwards; the level adds random blinks.
pub struct Fireflies {
    flies: Vec<Fly>,
    trees: Vec<Tree>,
    beats: usize,
    time: f32,
    rng: Rng,
}

impl Fireflies {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xF1_F1E5);
        let flies = (0..FLIES)
            .map(|i| {
                let (a, r) = (rng.range(0.0, TAU), rng.f32().sqrt() * 0.16);
                Fly {
                    cluster: i % CLUSTERS,
                    ox: a.cos() * r * 1.6,
                    oy: a.sin() * r,
                    phase: rng.range(0.0, TAU),
                    glow: 0.0,
                    flash_at: None,
                    low: rng.f32() < 0.3,
                }
            })
            .collect();
        let trees = (0..9)
            .map(|i| Tree {
                x: (i as f32 + rng.range(-0.3, 0.3)) / 8.0,
                height: rng.range(0.3, 0.55),
                radius: rng.range(0.06, 0.11),
                pine: rng.f32() < 0.5,
            })
            .collect();
        Self {
            flies,
            trees,
            beats: 0,
            time: 0.0,
            rng,
        }
    }

    /// Center of a swarm as fractions of the height (x) and height (y).
    fn cluster_center(&self, k: usize, aspect: f32) -> (f32, f32) {
        let t = self.time * 0.05;
        let kf = k as f32;
        let x = (kf + 0.5) / CLUSTERS as f32 * aspect + 0.12 * aspect * (t * (1.0 + 0.3 * kf) + kf * 2.0).sin();
        let y = 0.55 + 0.15 * (t * (0.7 + 0.2 * kf) + kf).cos() + 0.1 * (kf * 1.7).sin();
        (x, y)
    }
}

impl Effect for Fireflies {
    fn name(&self) -> &'static str {
        "Fireflies"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let now = self.time;
        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if c.h > 0 { w / h } else { 1.0 };

        if f.beat {
            let k = self.beats % CLUSTERS;
            self.beats += 1;
            for fl in self.flies.iter_mut().filter(|fl| fl.cluster == k) {
                let d = (fl.ox * fl.ox + fl.oy * fl.oy).sqrt();
                fl.flash_at = Some(now + d * 2.5 + self.rng.f32() * 0.05);
            }
        }
        let blink = (0.08 + 0.5 * f.level) * dt;
        for fl in &mut self.flies {
            fl.glow *= (-dt * 2.5).exp();
            if fl.flash_at.is_some_and(|at| now >= at) {
                fl.glow = 1.0;
                fl.flash_at = None;
            }
            if self.rng.f32() < blink {
                fl.glow = fl.glow.max(0.55);
            }
            // Slow wandering around the swarm center.
            fl.ox += (now * 0.7 + fl.phase).sin() * dt * 0.02;
            fl.oy += (now * 0.9 + fl.phase * 1.3).cos() * dt * 0.015;
            fl.ox = fl.ox.clamp(-0.35, 0.35);
            fl.oy = fl.oy.clamp(-0.25, 0.25);
        }

        if c.w == 0 || c.h == 0 {
            return;
        }

        // Sky and moon.
        let (mx, my, mr) = (w * 0.8, h * 0.16, (h * 0.06).max(1.0));
        for y in 0..c.h {
            let g = y as f32 / h;
            let sky = mix(rgb(0.01, 0.015, 0.05), rgb(0.03, 0.06, 0.1), g / MEADOW);
            for x in 0..c.w {
                let (dx, dy) = (x as f32 - mx, y as f32 - my);
                let d = (dx * dx + dy * dy).sqrt();
                let col = if d < mr {
                    rgb(0.75, 0.78, 0.7)
                } else {
                    mix(sky, rgb(0.2, 0.25, 0.3), (-(d - mr) / (mr * 2.0)).exp() * 0.5)
                };
                c.set(x, y, col);
            }
        }

        // Distant treeline, then near trees and the meadow.
        let far = rgb(0.02, 0.035, 0.05);
        let near = rgb(0.004, 0.008, 0.01);
        let meadow = (h * MEADOW) as usize;
        for x in 0..c.w {
            let u = x as f32 / w;
            let jag = hash(x as u32 / 2 + 7) * 0.025;
            let top = h * (0.56 + 0.04 * (u * 9.0).sin() + jag);
            for y in (top.max(0.0) as usize).min(c.h)..meadow.min(c.h) {
                c.set(x, y, far);
            }
        }
        for tr in &self.trees {
            let (tx, base) = (tr.x * w, h * MEADOW);
            let top = base - tr.height * h;
            let r = tr.radius * h;
            let x0 = ((tx - r - 1.0).max(0.0) as usize).min(c.w);
            let x1 = ((tx + r + 1.0).max(0.0) as usize).min(c.w);
            for x in x0..x1 {
                let dx = (x as f32 - tx).abs();
                for y in (top.max(0.0) as usize).min(c.h)..(base as usize).min(c.h) {
                    let fy = y as f32;
                    let inside = if tr.pine {
                        dx <= (fy - top) / (base - top).max(1.0) * r
                    } else {
                        let cyc = top + r;
                        let dy = fy - cyc;
                        dx * dx + dy * dy <= r * r || (dx < (r * 0.15).max(0.5) && fy > cyc)
                    };
                    if inside {
                        c.set(x, y, near);
                    }
                }
            }
        }
        for y in meadow.min(c.h)..c.h {
            let g = (y - meadow) as f32 / (h - meadow as f32).max(1.0);
            let col = mix(rgb(0.01, 0.03, 0.02), rgb(0.005, 0.012, 0.008), g);
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }

        let radius = (h / 30.0).max(1.0);
        let centers: Vec<(f32, f32)> = (0..CLUSTERS).map(|k| self.cluster_center(k, aspect)).collect();
        let draw = |c: &mut Canvas, fl: &Fly| {
            let (ccx, ccy) = centers[fl.cluster];
            let wob = (now * 1.7 + fl.phase).sin() * 0.01;
            let (x, y) = ((ccx + fl.ox + wob) * h, (ccy + fl.oy + wob * 0.7) * h);
            let b = 0.05 + 0.95 * fl.glow;
            let r = radius * (0.7 + 0.7 * fl.glow);
            let ri = r.ceil() as i32;
            let (xi, yi) = (x.round() as i32, y.round() as i32);
            let warm = rgb(0.75, 1.0, 0.25);
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let d2 = (dx * dx + dy * dy) as f32;
                    let k = b * (-d2 / (r * r * 0.35)).exp();
                    if k > 0.01 {
                        c.plot_add(xi + dx, yi + dy, scale(warm, k));
                    }
                }
            }
            if fl.glow > 0.5 {
                c.plot_add(xi, yi, scale(rgb(1.0, 1.0, 0.8), fl.glow - 0.5));
            }
        };
        for fl in self.flies.iter().filter(|fl| fl.low) {
            draw(c, fl);
        }

        // Grass blades in front, swaying with the bass.
        let sway = (now * 1.3).sin() * 0.6 + f.bass * 0.8;
        for x in 0..c.w {
            let tall = h * (0.03 + 0.07 * hash(x as u32 * 31 + 5));
            let lean = sway * tall / h.max(1.0) * 6.0;
            let top = h - tall;
            for y in (top.max(0.0) as usize).min(c.h)..c.h {
                let v = (h - y as f32) / tall.max(1.0);
                let sx = (x as f32 + lean * v * v).round() as i32;
                c.plot(sx, y as i32, near);
            }
        }

        for fl in self.flies.iter().filter(|fl| !fl.low) {
            draw(c, fl);
        }
    }
}
