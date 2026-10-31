use super::Effect;
use super::phyllotaxis::disc;
use super::polygons::stroke;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, scale};
use std::f32::consts::TAU;

const MAX_CIRCLES: usize = 700;
const POP_TIME: f32 = 0.45;

struct Circle {
    x: f32,
    y: f32,
    r: f32,
    growing: bool,
    /// Seconds since this circle was popped by a beat.
    pop: Option<f32>,
    /// Delay before it shrinks away when the screen dissolves.
    delay: f32,
}

/// Circle packing: seeds appear in free space and grow until they touch a
/// neighbour or the edge, filling the screen; then the packing dissolves in
/// a sweep and starts over. Colors follow size, bass speeds up growth and
/// beats pop the biggest circles, leaving holes for new ones.
pub struct CirclePacking {
    circles: Vec<Circle>,
    size: (usize, usize),
    /// Seconds without room for a new seed.
    misses: f32,
    /// Seconds since the packing started; beats could keep it from ever filling up.
    age: f32,
    dissolve: Option<f32>,
    hue: f32,
    rng: Rng,
}

impl CirclePacking {
    pub fn new() -> Self {
        Self {
            circles: Vec::new(),
            size: (0, 0),
            misses: 0.0,
            age: 0.0,
            dissolve: None,
            hue: 0.0,
            rng: Rng::new(0xC1C1),
        }
    }

    fn restart(&mut self) {
        self.circles.clear();
        self.misses = 0.0;
        self.age = 0.0;
        self.dissolve = None;
    }

    /// Distance from (x, y) to the nearest circle rim or screen edge.
    fn free_space(&self, x: f32, y: f32, skip: usize) -> f32 {
        let (w, h) = (self.size.0 as f32, self.size.1 as f32);
        let mut free = x.min(y).min(w - x).min(h - y);
        for (i, o) in self.circles.iter().enumerate() {
            if i != skip && o.pop.is_none() {
                free = free.min(((x - o.x).powi(2) + (y - o.y).powi(2)).sqrt() - o.r);
            }
        }
        free
    }
}

impl Effect for CirclePacking {
    fn name(&self) -> &'static str {
        "Circle Packing"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fill([3, 3, 6]);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.restart();
        }
        self.hue += dt * 0.01;
        let (w, h) = (c.w as f32, c.h as f32);
        let span = w.min(h);
        let gap = (span / 120.0).max(0.6);
        let min_r = (span / 80.0).max(0.8);
        let max_r = span * 0.3;

        match self.dissolve {
            None => {
                self.age += dt;
                // Seed the roomiest of a batch of random spots, so holes fill
                // with large circles instead of a swarm of tiny ones.
                let mut best = None;
                if self.circles.len() < MAX_CIRCLES {
                    for _ in 0..(dt * 1200.0) as usize {
                        let (x, y) = (self.rng.f32() * w, self.rng.f32() * h);
                        let free = self.free_space(x, y, usize::MAX);
                        if free > min_r + gap && best.is_none_or(|(_, _, b)| free > b) {
                            best = Some((x, y, free));
                        }
                    }
                }
                match best {
                    Some((x, y, _)) => {
                        self.circles.push(Circle {
                            x,
                            y,
                            r: min_r * 0.5,
                            growing: true,
                            pop: None,
                            delay: 0.0,
                        });
                        self.misses = 0.0;
                    }
                    None => self.misses += dt,
                }
                let rate = span * (0.06 + 0.25 * f.bass);
                for i in 0..self.circles.len() {
                    if !self.circles[i].growing {
                        continue;
                    }
                    let (x, y, r) = (self.circles[i].x, self.circles[i].y, self.circles[i].r);
                    let room = self.free_space(x, y, i) - gap;
                    let circle = &mut self.circles[i];
                    if room <= r + 0.01 || r >= max_r {
                        circle.growing = false;
                        circle.r = r.min(room).max(0.3);
                    } else {
                        circle.r = (r + rate * dt).min(room).min(max_r);
                    }
                }
                // Beats pop the largest settled circles.
                if f.beat {
                    let pops = 1 + (f.bass > 0.8) as usize;
                    for _ in 0..pops {
                        let biggest = self
                            .circles
                            .iter_mut()
                            .filter(|o| o.pop.is_none() && !o.growing)
                            .max_by(|a, b| a.r.total_cmp(&b.r));
                        match biggest {
                            Some(o) => o.pop = Some(0.0),
                            None => break,
                        }
                    }
                }
                let busy = self.circles.iter().any(|o| o.growing);
                if (!busy && self.misses > 1.5) || self.age > 25.0 {
                    // Full: dissolve in a sweep from a random point.
                    let (ox, oy) = (self.rng.f32() * w, self.rng.f32() * h);
                    let diag = (w * w + h * h).sqrt().max(1.0);
                    for o in &mut self.circles {
                        o.delay = ((o.x - ox).powi(2) + (o.y - oy).powi(2)).sqrt() / diag * 2.0;
                    }
                    self.dissolve = Some(0.0);
                }
            }
            Some(time) => {
                let time = time + dt;
                self.dissolve = Some(time);
                if time > 3.5 {
                    self.restart();
                }
            }
        }
        for o in &mut self.circles {
            if let Some(p) = &mut o.pop {
                *p += dt;
            }
        }
        self.circles.retain(|o| o.pop.is_none_or(|p| p < POP_TIME));

        let shrink = self.dissolve.unwrap_or(0.0);
        let lw = (span / 100.0).max(0.8);
        for o in &self.circles {
            let fade = if self.dissolve.is_some() {
                (1.0 - (shrink - o.delay) * 1.5).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let size = (o.r / (span * 0.15).max(1.0)).min(1.0);
            let hue = self.hue + 0.62 - 0.62 * size;
            let value = (0.55 + 0.3 * size + 0.25 * f.pulse * size).min(1.0);
            if let Some(p) = o.pop {
                // A popped circle bursts into an expanding, fading ring.
                let k = p / POP_TIME;
                let rr = o.r * (1.0 + 0.6 * k);
                let col = scale(hsv(hue, 0.3, 1.0), 1.0 - k);
                let steps = ((rr * TAU / 2.0) as usize).clamp(8, 160);
                for s in 0..steps {
                    let a0 = TAU * s as f32 / steps as f32;
                    let a1 = TAU * (s + 1) as f32 / steps as f32;
                    stroke(
                        c,
                        o.x + a0.cos() * rr,
                        o.y + a0.sin() * rr,
                        o.x + a1.cos() * rr,
                        o.y + a1.sin() * rr,
                        lw * 1.5,
                        col,
                    );
                }
                continue;
            }
            let r = o.r * fade;
            let core = hsv(hue, 0.65, value * 0.45);
            let rim = hsv(hue + 0.04, 0.8, value);
            let edge = if r > 3.0 { (1.0 - lw * 1.5 / r).max(0.5) } else { 0.0 };
            disc(c, o.x, o.y, r, |d| {
                if d >= edge {
                    rim
                } else {
                    mix(core, rim, (d / edge.max(1e-3)).powi(3) * 0.6)
                }
            });
        }
    }
}
