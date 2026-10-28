use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, add, hsv, rgb, scale};
use std::f32::consts::TAU;

const MAX_SPARKS: usize = 6000;
const MAX_ROCKETS: usize = 12;

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Peony,
    Ring,
    Willow,
    Crossette,
}

#[derive(Clone, Copy)]
struct Rocket {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    hue: f32,
    shape: Shape,
    power: f32,
    /// Height (0 = top) at which the shell bursts.
    burst_y: f32,
}

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    decay: f32,
    drag: f32,
    hue: f32,
    sat: f32,
    /// Crossette stars split once more when their life drops below 0.6.
    splits: bool,
}

/// Fireworks over a dark skyline: beats launch rockets that climb with a
/// sparkling tail and burst into peonies, rings, golden willows or
/// crossettes. The dominant band picks the color (bass warm, mids green and
/// gold, treble blue and violet), bass sets the burst size.
pub struct Fireworks {
    rockets: Vec<Rocket>,
    sparks: Vec<Spark>,
    flashes: Vec<(f32, f32, f32, [u8; 3])>,
    rng: Rng,
    idle: f32,
}

impl Fireworks {
    pub fn new() -> Self {
        Self {
            rockets: Vec::new(),
            sparks: Vec::new(),
            flashes: Vec::new(),
            rng: Rng::new(0xF1AE),
            idle: 0.0,
        }
    }

    fn launch(&mut self, f: &Features) {
        if self.rockets.len() >= MAX_ROCKETS {
            return;
        }
        let r = &mut self.rng;
        let base = if f.bass >= f.mid && f.bass >= f.treble {
            0.02
        } else if f.mid >= f.treble {
            0.2
        } else {
            0.62
        };
        let shape = match r.next_u32() % 4 {
            0 => Shape::Peony,
            1 => Shape::Ring,
            2 => Shape::Willow,
            _ => Shape::Crossette,
        };
        self.rockets.push(Rocket {
            x: r.range(0.15, 0.85),
            y: 1.0,
            vx: r.range(-0.08, 0.08),
            vy: -r.range(0.95, 1.25),
            hue: base + r.range(-0.05, 0.08),
            shape,
            power: 0.6 + 0.6 * f.bass,
            burst_y: r.range(0.12, 0.45),
        });
    }

    fn burst(&mut self, rk: &Rocket) {
        let r = &mut self.rng;
        let n = match rk.shape {
            Shape::Ring => 90,
            Shape::Willow => 160,
            _ => 140,
        };
        let speed = 0.35 * rk.power;
        let tilt = r.range(0.5, 1.0);
        for i in 0..n {
            let a = if rk.shape == Shape::Ring {
                i as f32 / n as f32 * TAU
            } else {
                r.range(0.0, TAU)
            };
            let v = match rk.shape {
                Shape::Ring => speed,
                // sqrt gives an even fill of the disc.
                _ => speed * r.f32().sqrt(),
            };
            let (vx, vy) = (
                a.cos() * v,
                a.sin() * v * if rk.shape == Shape::Ring { tilt } else { 1.0 },
            );
            let willow = rk.shape == Shape::Willow;
            self.sparks.push(Spark {
                x: rk.x,
                y: rk.y,
                vx: vx + rk.vx * 0.3,
                vy: vy + rk.vy * 0.3,
                life: 1.0,
                decay: if willow { r.range(0.25, 0.4) } else { r.range(0.5, 0.9) },
                drag: if willow { 2.2 } else { 1.2 },
                hue: if willow { 0.11 } else { rk.hue + r.range(-0.03, 0.03) },
                sat: if willow { 0.75 } else { 0.85 },
                splits: rk.shape == Shape::Crossette,
            });
        }
        self.flashes.push((rk.x, rk.y, 1.0, hsv(rk.hue, 0.5, 1.0)));
    }
}

impl Effect for Fireworks {
    fn name(&self) -> &'static str {
        "Fireworks"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        let dt = dt.min(0.1);
        // Keep the sky busy during quiet passages too.
        self.idle += dt * (0.4 + 1.5 * f.level);
        if f.beat || self.idle > 1.5 {
            self.idle = 0.0;
            self.launch(f);
            if f.beat && f.bass > 0.7 {
                self.launch(f);
            }
        }

        // All motion runs in units of the canvas height, x is scaled by aspect.
        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if h > 0.0 { w / h } else { 1.0 };
        let gravity = 0.35;

        let mut bursting = Vec::new();
        for rk in &mut self.rockets {
            rk.vy += gravity * 0.6 * dt;
            rk.x += rk.vx * dt / aspect;
            rk.y += rk.vy * dt;
            // Sparkling tail.
            if self.rng.f32() < 0.8 {
                self.sparks.push(Spark {
                    x: rk.x,
                    y: rk.y,
                    vx: self.rng.range(-0.03, 0.03),
                    vy: self.rng.range(0.0, 0.08),
                    life: 0.5,
                    decay: 1.5,
                    drag: 3.0,
                    hue: 0.09,
                    sat: 0.5,
                    splits: false,
                });
            }
        }
        self.rockets.retain(|rk| {
            let apex = rk.vy > -0.1 || rk.y < rk.burst_y;
            if apex {
                bursting.push(*rk);
            }
            !apex
        });
        for rk in &bursting {
            self.burst(rk);
        }

        let mut children = Vec::new();
        for s in &mut self.sparks {
            let drag = (-s.drag * dt).exp();
            s.vx *= drag;
            s.vy = s.vy * drag + gravity * 0.3 * dt;
            s.x += s.vx * dt / aspect;
            s.y += s.vy * dt;
            s.life -= s.decay * dt;
            if s.splits && s.life < 0.6 {
                s.splits = false;
                for k in 0..4 {
                    let a = k as f32 * TAU / 4.0 + 0.4;
                    children.push(Spark {
                        x: s.x,
                        y: s.y,
                        vx: a.cos() * 0.12,
                        vy: a.sin() * 0.12,
                        life: 0.5,
                        decay: 1.2,
                        drag: 2.0,
                        hue: s.hue + 0.5,
                        sat: 0.6,
                        splits: false,
                    });
                }
            }
        }
        self.sparks.append(&mut children);
        self.sparks.retain(|s| s.life > 0.0 && s.y < 1.05);
        let excess = self.sparks.len().saturating_sub(MAX_SPARKS);
        self.sparks.drain(..excess);

        c.fade(0.78);
        if c.w == 0 || c.h == 0 {
            return;
        }
        // Soft glow where shells burst.
        for fl in &mut self.flashes {
            let (fx, fy, k, col) = *fl;
            let r = h * 0.12 * k;
            let (px, py) = (fx * w, fy * h);
            let (x0, x1) = ((px - r).max(0.0) as usize, ((px + r).max(0.0) as usize).min(c.w));
            let (y0, y1) = ((py - r).max(0.0) as usize, ((py + r).max(0.0) as usize).min(c.h));
            for y in y0..y1 {
                for x in x0..x1 {
                    let (dx, dy) = (x as f32 - px, y as f32 - py);
                    let d = (dx * dx + dy * dy).sqrt() / r.max(0.5);
                    if d < 1.0 {
                        let i = y * c.w + x;
                        c.px[i] = add(c.px[i], scale(col, 0.12 * k * (1.0 - d)));
                    }
                }
            }
            fl.2 -= dt * 4.0;
        }
        self.flashes.retain(|fl| fl.2 > 0.0);

        for s in &self.sparks {
            // Twinkle as they burn out.
            let flicker = if s.life < 0.35 && self.rng.f32() < 0.4 {
                0.2
            } else {
                1.0
            };
            let v = (s.life.min(1.0) * 1.3).min(1.0) * flicker;
            let col = hsv(s.hue, s.sat * (1.0 - 0.5 * s.life.min(1.0)), v);
            c.plot_add((s.x * w) as i32, (s.y * h) as i32, col);
        }
        for rk in &self.rockets {
            c.plot_add((rk.x * w) as i32, (rk.y * h) as i32, [255, 240, 200]);
        }

        // Skyline silhouette along the bottom.
        let hills = |x: f32| {
            let n = (x * 0.13).floor();
            let r = ((n * 12.9898).sin() * 43_758.547).fract().abs();
            h * (0.04 + 0.08 * r)
        };
        for x in 0..c.w {
            let top = (h - hills(x as f32 / w.max(1.0) * 240.0)).max(0.0) as usize;
            for y in top.min(c.h)..c.h {
                c.set(x, y, rgb(0.02, 0.02, 0.04));
            }
        }
    }
}
