use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, mix};

/// Seconds between new glitch layouts while the glitch is running.
const RESEED: f32 = 1.0 / 12.0;

fn hash(a: u32, b: u32, c: u32) -> u32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// A bold, colorful poster (hue bands, a striped sun, a spinning frame) torn
/// apart by digital glitches: RGB split, displaced and frozen blocks,
/// inverted tiles and scanline tearing. Every beat sets off a burst that
/// decays again, and bands with more spectral energy break up more.
pub struct Glitch {
    buf: Vec<Rgb>,
    rng: Rng,
    intensity: f32,
    seed: u32,
    reseed: f32,
    hue: f32,
}

impl Glitch {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            rng: Rng::new(0x611C),
            intensity: 0.0,
            seed: 1,
            reseed: 0.0,
            hue: 0.0,
        }
    }

    fn poster(&mut self, w: usize, h: usize, t: f32, f: &Features) {
        self.buf.resize(w * h, [0; 3]);
        let (wf, hf) = (w as f32, h as f32);
        let (cx, cy) = (wf * 0.5, hf * 0.5);
        let r = hf.min(wf) * (0.33 + 0.06 * f.bass);
        let band_w = (wf / 7.0).max(1.0);
        let (sa, ca) = (t * 0.6).sin_cos();
        let frame = hf.min(wf) * 0.45;
        let thick = (hf / 30.0).max(1.0);
        for y in 0..h {
            let yf = y as f32;
            for x in 0..w {
                let xf = x as f32;
                let (dx, dy) = (xf - cx, yf - cy);
                let k = ((xf + yf * 0.6) / band_w + t * 0.8).floor();
                let mut col = hsv(self.hue + k * 0.13, 0.85, 0.45 + 0.1 * (k * 1.7).sin());
                if dx * dx + dy * dy < r * r {
                    let v = (dy / r + 1.0) * 0.5;
                    let stripe = ((v * 9.0 - t * 1.5).rem_euclid(1.0)) < 0.7 - v * 0.4;
                    if stripe {
                        col = mix(hsv(self.hue + 0.12, 0.8, 1.0), hsv(self.hue + 0.9, 0.9, 0.9), v);
                    }
                }
                // Rotated square outline.
                let (rx, ry) = ((dx * ca - dy * sa).abs(), (dx * sa + dy * ca).abs());
                let m = rx.max(ry);
                if (m - frame).abs() < thick {
                    col = [245, 245, 245];
                }
                self.buf[y * w + x] = col;
            }
        }
    }
}

impl Effect for Glitch {
    fn name(&self) -> &'static str {
        "Glitch"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        self.hue += dt * 0.02;
        self.intensity = (self.intensity - dt * 1.8).max(0.0);
        self.reseed -= dt;
        if f.beat {
            self.intensity = 1.0;
            self.hue += 0.07;
            self.reseed = 0.0;
        }
        if self.reseed <= 0.0 {
            self.reseed = RESEED;
            self.seed = self.rng.next_u32();
        }
        self.poster(w, h, t, f);

        let g = (self.intensity + f.level * 0.15).min(1.0);
        let split = (g * 6.0 + f.bass * 2.0).round() as i32;
        let bs = (h / 8).max(2);
        let bins = f.spectrum.len();
        let (wi, hi) = (w as i32, h as i32);
        let seed = self.seed;
        for y in 0..h {
            // Scanline tearing, in pairs of lines.
            let row = hash(seed, y as u32 / 2, 7);
            let tear = if unit(row) < g * 0.12 {
                (unit(row >> 3) - 0.5) * w as f32 * 0.5
            } else {
                0.0
            };
            let spec = if bins == 0 {
                0.5
            } else {
                f.spectrum[(y * bins / h).min(bins - 1)]
            };
            let by = (y / bs) as u32;
            for x in 0..w {
                let bx = (x / bs) as u32;
                let bh = hash(seed, bx, by);
                let r = unit(bh);
                let p = g * (0.2 + 0.4 * spec);
                if r < p * 0.15 {
                    // Frozen block: keep last frame's pixel.
                    continue;
                }
                let (mut sx, mut sy) = (x as i32 + tear as i32, y as i32);
                if r < p {
                    sx += ((unit(bh >> 4) - 0.5) * w as f32 * 0.4) as i32;
                    sy += ((unit(bh >> 9) - 0.5) * bs as f32 * 2.0) as i32;
                }
                let sy = sy.rem_euclid(hi) as usize * w;
                let at = |dx: i32| self.buf[sy + (sx + dx).rem_euclid(wi) as usize];
                let mut col = if split > 0 {
                    [at(split)[0], at(0)[1], at(-split)[2]]
                } else {
                    at(0)
                };
                if r < p * 0.4 && r > p * 0.25 {
                    col = [255 - col[0], 255 - col[1], 255 - col[2]];
                } else if r < p * 0.55 && r >= p * 0.4 {
                    col = [col[2], col[0], col[1]];
                }
                c.set(x, y, col);
            }
        }

        // Bright noise lines during a burst.
        if g > 0.3 {
            for _ in 0..(g * 4.0) as usize {
                let y = self.rng.next_u32() as usize % h;
                let x0 = self.rng.next_u32() as usize % w;
                let len = (self.rng.next_u32() as usize % w.max(2)).min(w - x0);
                for x in x0..x0 + len {
                    let n = (self.rng.next_u32() & 0xFF) as u8;
                    c.set(x, y, [n, n.saturating_add(40), n]);
                }
            }
        }
    }
}
