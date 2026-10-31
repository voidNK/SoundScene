use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

/// Pixels per accumulation cell along each axis.
const CELL: usize = 2;
const MAX_PUFFS: usize = 1600;
const LIFE: f32 = 6.0;

struct Puff {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    color: [f32; 3],
    size: f32,
}

/// Rising smoke: soft additive puffs climb from a few chimneys and get
/// twisted by a drifting curl-noise field into curls and wisps. Beats
/// blow out thick bursts, loudness feeds the stream, bass warms the color.
pub struct Smoke {
    puffs: Vec<Puff>,
    acc: Vec<[f32; 3]>,
    img: Vec<[f32; 3]>,
    gw: usize,
    gh: usize,
    rng: Rng,
    time: f32,
    emit: f32,
    hue: f32,
}

impl Smoke {
    pub fn new() -> Self {
        Self {
            puffs: Vec::new(),
            acc: Vec::new(),
            img: Vec::new(),
            gw: 0,
            gh: 0,
            rng: Rng::new(0x5310),
            time: 0.0,
            emit: 0.0,
            hue: 0.0,
        }
    }

    fn spawn(&mut self, aspect: f32, warmth: f32, burst: f32) {
        if self.puffs.len() >= MAX_PUFFS {
            return;
        }
        // Three chimneys along the bottom.
        let src = (self.rng.next_u32() % 3) as f32;
        let x = aspect * (0.22 + 0.28 * src) + self.rng.range(-0.02, 0.02);
        let hue = self.hue + src * 0.12 + self.rng.range(-0.03, 0.03) - warmth * 0.15;
        let c = hsv(hue, 0.35 + 0.35 * warmth, 1.0);
        self.puffs.push(Puff {
            x,
            y: 1.02,
            vx: self.rng.range(-0.05, 0.05) * (1.0 + burst),
            vy: -0.12 - self.rng.f32() * 0.08 - 0.25 * burst,
            age: 0.0,
            life: LIFE * self.rng.range(0.6, 1.0),
            color: c.map(|v| v as f32 / 255.0),
            size: self.rng.range(0.7, 1.3) * (1.0 + 0.6 * burst),
        });
    }
}

/// Divergence-free velocity from the curl of a sum-of-sines stream function.
fn curl(x: f32, y: f32, t: f32) -> (f32, f32) {
    // ψ = sin(3x + t)·cos(4y − 0.7t) / 4 + sin(7y + 1.3t + 2x) / 9
    let a = 3.0 * x + t;
    let b = 4.0 * y - 0.7 * t;
    let e = 7.0 * y + 1.3 * t + 2.0 * x;
    let dpsi_dx = 3.0 * a.cos() * b.cos() / 4.0 + 2.0 * e.cos() / 9.0;
    let dpsi_dy = -4.0 * a.sin() * b.sin() / 4.0 + 7.0 * e.cos() / 9.0;
    (dpsi_dy, -dpsi_dx)
}

impl Effect for Smoke {
    fn name(&self) -> &'static str {
        "Smoke"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        if (gw, gh) != (self.gw, self.gh) {
            (self.gw, self.gh) = (gw, gh);
            self.acc = vec![[0.0; 3]; gw * gh];
            self.img = vec![[0.0; 3]; gw * gh];
        }
        self.time += dt * (0.5 + 0.6 * f.level);
        self.hue += dt * 0.01;
        // World units: height 1, width = aspect ratio.
        let aspect = c.w as f32 / c.h as f32;

        self.emit += dt * (90.0 + 140.0 * f.level);
        while self.emit >= 1.0 {
            self.emit -= 1.0;
            self.spawn(aspect, f.bass, 0.0);
        }
        if f.beat {
            for _ in 0..(40.0 + 80.0 * f.bass) as usize {
                self.spawn(aspect, f.bass, 0.6 + f.bass);
            }
        }

        let t = self.time;
        let swirl = 0.18 + 0.25 * f.mid;
        for p in &mut self.puffs {
            p.age += dt;
            let (ux, uy) = curl(p.x * 3.2, p.y * 3.2, t);
            // Buoyancy fades as the smoke cools; the flow takes over.
            let lift = -0.22 * (1.0 - 0.5 * p.age / p.life);
            let drag = 1.0 - (-1.5 * dt).exp();
            p.vx += (ux * swirl * 2.4 - p.vx) * drag;
            p.vy += (uy * swirl * 2.4 + lift - p.vy) * drag;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
        }
        self.puffs
            .retain(|p| p.age < p.life && p.y > -0.3 && p.x > -0.3 && p.x < aspect + 0.3);

        // Splat each puff as a soft disc into the half-resolution buffer.
        // The density buffer persists briefly so puffs smear into plumes.
        let keep = (-9.0 * dt).exp();
        for o in &mut self.acc {
            *o = o.map(|v| v * keep);
        }
        let unit = gh as f32;
        for p in &self.puffs {
            let k = p.age / p.life;
            let r = (unit * (0.02 + 0.08 * k) * p.size).max(0.8);
            let a = (k * 8.0).min(1.0) * (1.0 - k) * (1.0 - k) * 0.05 * dt * 30.0 / (r * 0.15).max(1.0);
            let (px, py) = (p.x * unit, p.y * unit);
            let x0 = (px - r).floor().max(0.0) as usize;
            let y0 = (py - r).floor().max(0.0) as usize;
            let x1 = ((px + r).ceil().max(0.0) as usize).min(gw);
            let y1 = ((py + r).ceil().max(0.0) as usize).min(gh);
            let inv = 1.0 / (r * r);
            for y in y0..y1 {
                let dy = y as f32 + 0.5 - py;
                for x in x0..x1 {
                    let dx = x as f32 + 0.5 - px;
                    let d = 1.0 - (dx * dx + dy * dy) * inv;
                    if d > 0.0 {
                        let wgt = d * d * a;
                        let o = &mut self.acc[y * gw + x];
                        o[0] += p.color[0] * wgt;
                        o[1] += p.color[1] * wgt;
                        o[2] += p.color[2] * wgt;
                    }
                }
            }
        }
        // Soft tone curve over a dusky gradient.
        let gain = 1.0 + 0.5 * f.pulse;
        for y in 0..gh {
            let bg = 0.02 + 0.05 * (y as f32 / gh as f32);
            let row = y * gw..(y + 1) * gw;
            for (o, a) in self.img[row.clone()].iter_mut().zip(&self.acc[row]) {
                *o = a.map(|v| 1.0 - (-v * gain).exp() + bg * 0.6);
                o[2] += bg * 0.5;
            }
        }
        upscale(c, &self.img, gw, gh);
    }
}
