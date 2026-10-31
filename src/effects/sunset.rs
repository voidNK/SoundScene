use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, mix, rgb, scale};

const RIDGES: usize = 5;

fn hash(n: u32) -> f32 {
    let mut n = n.wrapping_mul(0x9E37_79B9);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 13;
    (n & 0xFFFF) as f32 / 65535.0
}

/// Smooth 1D value noise.
fn noise(x: f32, seed: u32) -> f32 {
    let i = x.floor();
    let t = x - i;
    let t = t * t * (3.0 - 2.0 * t);
    let n = (i as i64 as u32).wrapping_add(seed.wrapping_mul(10_007));
    let (a, b) = (hash(n), hash(n.wrapping_add(1)));
    a + (b - a) * t
}

/// Ridge line 0..1 with a few octaves; sharper peaks for far ranges.
fn ridge(x: f32, seed: u32) -> f32 {
    let mut v = 0.0;
    let mut amp = 0.55;
    let mut freq = 1.0;
    for o in 0..4 {
        // Folded noise gives sharp peaks and rounded valleys.
        v += amp * (1.0 - (2.0 * noise(x * freq, seed + o) - 1.0).abs());
        amp *= 0.5;
        freq *= 2.1;
    }
    v
}

/// Layered mountain ranges scrolling with parallax in front of a sky that
/// follows the sun as it slowly sinks and rises again, from day blue to
/// glowing orange and violet. The sun glows with the level, beats flare it
/// and bass speeds up the journey.
pub struct Sunset {
    /// Day cycle phase, 0 = sun high.
    day: f32,
    scroll: f32,
    flare: f32,
}

impl Sunset {
    pub fn new() -> Self {
        Self {
            day: 1.6,
            scroll: 0.0,
            flare: 0.0,
        }
    }
}

impl Effect for Sunset {
    fn name(&self) -> &'static str {
        "Sunset"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.day += dt * 0.04;
        self.scroll += dt * (0.04 + 0.12 * f.bass);
        if f.beat {
            self.flare = 1.0;
        }
        self.flare *= (-dt * 3.0).exp();
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (w, h) = (c.w as f32, c.h as f32);

        // Sun height: 0 = high, 1 = at the horizon, beyond = set.
        let low = 0.5 - 0.5 * self.day.cos();
        let horizon = h * 0.72;
        let (sx, sy) = (w * 0.6, h * 0.15 + low * (horizon - h * 0.05));
        let sr = (h * 0.08).max(1.0);
        let dusk = low.powf(1.5);
        let zenith = mix(rgb(0.15, 0.35, 0.75), rgb(0.12, 0.05, 0.25), dusk);
        let lower = mix(rgb(0.6, 0.8, 0.95), rgb(1.0, 0.45, 0.15), dusk);
        let sun_col = mix(rgb(1.0, 0.97, 0.85), rgb(1.0, 0.55, 0.2), dusk);
        let glow = 0.5 + 0.5 * f.level + 0.6 * self.flare;

        for y in 0..c.h {
            let g = (y as f32 / horizon).min(1.0);
            let sky = mix(zenith, lower, g.powf(1.6));
            for x in 0..c.w {
                let (dx, dy) = (x as f32 - sx, y as f32 - sy);
                let d = (dx * dx + dy * dy).sqrt();
                let col = if d < sr {
                    // Darker, redder rim on the disc.
                    mix(rgb(1.0, 1.0, 0.95), sun_col, (d / sr).powi(3))
                } else {
                    let halo = (-(d - sr) / (sr * (1.5 + 2.0 * glow))).exp() * (0.4 + 0.4 * glow);
                    let wide = (-(d - sr) / (h * 0.6)).exp() * 0.3 * dusk;
                    mix(sky, sun_col, (halo + wide).min(1.0))
                };
                c.set(x, y, col);
            }
        }

        // Ranges from far (hazy, slow) to near (dark, fast).
        let near_col = mix(rgb(0.08, 0.12, 0.15), rgb(0.06, 0.02, 0.08), dusk);
        for i in 0..RIDGES {
            let depth = i as f32 / (RIDGES - 1) as f32;
            let col: Rgb = mix(mix(lower, zenith, 0.25), near_col, 0.35 + 0.65 * depth);
            let speed = 0.3 + 2.5 * depth * depth;
            let freq = 3.5 - 1.5 * depth;
            let base = h * (0.62 + 0.1 * i as f32);
            let amp = h * (0.4 - 0.05 * i as f32);
            for x in 0..c.w {
                let u = x as f32 / h * freq * 0.5 + self.scroll * speed + i as f32 * 17.3;
                let top = base - amp * ridge(u, i as u32 * 3 + 1);
                let y0 = (top.max(0.0) as usize).min(c.h);
                for y in y0..c.h {
                    // Lighter rim catching the sun at the top of each range.
                    let rim = if y == y0 { 0.25 * (1.0 - depth) } else { 0.0 };
                    let fall = ((y as f32 - top) / h).min(1.0) * 0.4 * depth;
                    c.set(x, y, mix(scale(col, 1.0 - fall), sun_col, rim));
                }
            }
        }
    }
}
