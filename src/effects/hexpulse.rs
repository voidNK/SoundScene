use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};

const MAX_WAVES: usize = 12;
const SQRT3: f32 = 1.732_050_8;

struct Wave {
    born: f32,
    speed: f32,
    strength: f32,
    hue: f32,
}

/// Honeycomb of beveled hexagons. Every ring of hexes around the center
/// shows one spectrum band (bass in the middle, treble outside), and each
/// beat launches a wave of light that rolls outward ring by ring.
pub struct HexPulse {
    waves: Vec<Wave>,
    time: f32,
    last_wave: f32,
    hue: f32,
}

impl HexPulse {
    pub fn new() -> Self {
        Self {
            waves: Vec::new(),
            time: 0.0,
            last_wave: 0.0,
            hue: 0.0,
        }
    }
}

impl Effect for HexPulse {
    fn name(&self) -> &'static str {
        "Hex Pulse"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        self.hue += dt * 0.02;
        let now = self.time;
        // A beat, or a soft heartbeat when the music has none.
        if (f.beat || now - self.last_wave > 2.0) && self.waves.len() < MAX_WAVES {
            self.hue += if f.beat { 0.07 } else { 0.0 };
            self.waves.push(Wave {
                born: now,
                speed: 7.0 + 8.0 * f.bass,
                strength: if f.beat { 0.6 + 0.6 * f.bass } else { 0.35 },
                hue: self.hue + 0.5,
            });
            self.last_wave = now;
        }
        self.waves.retain(|w| now - w.born < 4.0);

        if c.w == 0 || c.h == 0 {
            return;
        }
        let size = (c.w.min(c.h) as f32 / 14.0).max(2.0);
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let max_dist = ((c.w.max(c.h) as f32 / size) as usize + 2).min(512);
        let bins = f.spectrum.len();

        // Everything about a hex depends only on its ring, so precompute per ring.
        let rings: Vec<[u8; 3]> = (0..=max_dist)
            .map(|d| {
                let df = d as f32;
                let band = if bins > 0 {
                    f.spectrum[(d * bins / max_dist.max(1)).min(bins - 1)]
                } else {
                    0.0
                };
                let (mut light, mut hue_shift, mut weight) = (0.0f32, 0.0f32, 0.0f32);
                for w in &self.waves {
                    let age = now - w.born;
                    let front = age * w.speed;
                    let k = w.strength * (-((df - front) / 1.3).powi(2)).exp() * (-age * 0.5).exp();
                    light += k;
                    hue_shift += k * w.hue;
                    weight += k;
                }
                let base_hue = self.hue + df * 0.03;
                let hue = if weight > 0.01 {
                    base_hue + (hue_shift / weight - base_hue) * (weight * 2.0).min(1.0)
                } else {
                    base_hue
                };
                let v = 0.07 + 0.4 * band + 0.9 * light;
                hsv(hue, (0.85 - 0.5 * (light - 0.6).max(0.0)).max(0.2), v.min(1.0))
            })
            .collect();

        let apothem = size * SQRT3 * 0.5;
        for y in 0..c.h {
            let py = y as f32 - cy;
            for x in 0..c.w {
                let px = x as f32 - cx;
                // Pixel to axial coordinates (pointy-top), then cube rounding.
                let q = (SQRT3 / 3.0 * px - py / 3.0) / size;
                let r = (2.0 / 3.0 * py) / size;
                let s = -q - r;
                let (mut rq, mut rr, rs) = (q.round(), r.round(), s.round());
                let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
                if dq > dr && dq > ds {
                    rq = -rr - rs;
                } else if dr > ds {
                    rr = -rq - rs;
                }
                let lx = (px - size * SQRT3 * (rq + rr * 0.5)).abs();
                let ly = (py - size * 1.5 * rr).abs();
                let edge = lx.max(lx * 0.5 + ly * SQRT3 * 0.5) / apothem;
                let dist = ((rq.abs() + rr.abs() + (rq + rr).abs()) * 0.5) as usize;
                let col = rings[dist.min(max_dist)];
                let col = if edge > 0.88 {
                    rgb(0.01, 0.01, 0.02)
                } else if edge > 0.72 {
                    // Bevel: brighter on the upper rim, darker below.
                    let k = if py < size * 1.5 * rr { 1.35 } else { 0.6 };
                    [
                        (col[0] as f32 * k).min(255.0) as u8,
                        (col[1] as f32 * k).min(255.0) as u8,
                        (col[2] as f32 * k).min(255.0) as u8,
                    ]
                } else {
                    col
                };
                c.set(x, y, col);
            }
        }
    }
}
