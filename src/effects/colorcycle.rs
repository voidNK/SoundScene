use super::Effect;
use super::ink::{hash, noise};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, rgb};
use std::f32::consts::TAU;

/// Palette ranges: each scene pixel stores `range << 6 | index`.
const STATIC: u8 = 0;
const FALLS: u8 = 1;
const SUN: u8 = 2;
const POOL: u8 = 3;
/// 4×4 ordered-dither thresholds.
const BAYER: [[f32; 4]; 4] = [
    [0.03, 0.53, 0.16, 0.66],
    [0.78, 0.28, 0.91, 0.41],
    [0.22, 0.72, 0.09, 0.59],
    [0.97, 0.47, 0.84, 0.34],
];

/// Classic palette-cycling art: a still picture of a waterfall pouring
/// between two cliffs under a setting sun, animated only by rotating its
/// palette. The level speeds up the water, mids the sun rays, beats flash it.
pub struct ColorCycle {
    scene: Vec<u8>,
    w: usize,
    h: usize,
    falls: f32,
    pool: f32,
    sun: f32,
}

impl ColorCycle {
    pub fn new() -> Self {
        Self {
            scene: Vec::new(),
            w: 0,
            h: 0,
            falls: 0.0,
            pool: 0.0,
            sun: 0.0,
        }
    }

    /// Paints the palette indices of the picture; only done on resize.
    fn build(&mut self) {
        let (w, h) = (self.w, self.h);
        let (wf, hf) = (w as f32, h as f32);
        let s = wf.min(hf).max(1.0);
        let (sun_x, sun_y, sun_r) = (wf * 0.74, hf * 0.17, s * 0.075);
        let pool_top = 0.74;
        self.scene = vec![0; w * h];
        for y in 0..h {
            let v = (y as f32 + 0.5) / hf;
            for x in 0..w {
                let u = (x as f32 + 0.5) / wf;
                // Cliff faces left and right of the falls, ragged with noise.
                let rag = noise(v * 9.0, 3.0) * 0.06 + noise(v * 23.0, 8.0) * 0.025;
                let left = 0.40 - rag + 0.06 * ((v - 0.35) / 0.4).clamp(0.0, 1.0);
                let rag_r = noise(v * 9.0, 11.0) * 0.06 + noise(v * 23.0, 17.0) * 0.025;
                let right = 0.60 + rag_r - 0.06 * ((v - 0.35) / 0.4).clamp(0.0, 1.0);
                let ledge = 0.33 + 0.04 * noise(u * 7.0, 1.0);
                let top_l = ledge + 0.08 * (0.4 - u).max(0.0);
                let top_r = ledge + 0.1 * (u - 0.6).max(0.0);
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);

                let idx = if v >= pool_top {
                    // Ripples spreading from the plunge point, squashed by perspective.
                    let (dx, dy) = ((px - wf * 0.5) / s, (py - hf * pool_top) / s * 3.0);
                    let d = (dx * dx + dy * dy).sqrt();
                    POOL << 6 | ((d * 70.0 + noise(px * 0.3, py * 0.3) * 4.0) as u32 & 63) as u8
                } else if v > top_l.min(top_r) && (u < left || u > right) && v > if u < 0.5 { top_l } else { top_r } {
                    // Rock: shaded by noise and lit from the sun side.
                    let shade = noise(px * 0.25, py * 0.25) * 0.6 + noise(px * 0.9, py * 0.9) * 0.25;
                    let lit = if u > 0.5 { 0.15 } else { 0.0 };
                    STATIC << 6 | (32 + ((shade + lit) * 31.0).min(31.0) as u8)
                } else if v > ledge - 0.02 && u >= left && u <= right {
                    // The falls: vertical streaks running downwards.
                    let col = hash(x as i32 / 2, 5) * 22.0 + noise(u * 30.0, 0.0) * 12.0;
                    FALLS << 6 | ((py / s * 90.0 + col) as u32 & 63) as u8
                } else {
                    let (dx, dy) = (px - sun_x, py - sun_y);
                    let d = (dx * dx + dy * dy).sqrt();
                    // Rays fade into the sky through an ordered dither.
                    let fade = (1.0 - (d - sun_r) / (sun_r * 3.0)).clamp(0.0, 1.0).powf(1.5);
                    if d < sun_r || BAYER[y % 4][x % 4] < fade {
                        // Rings in the disc, swirling rays around it.
                        let ring = if d < sun_r {
                            d / sun_r * 16.0
                        } else {
                            (dy.atan2(dx) / TAU + 0.5) * 64.0 * 2.0 + d / sun_r * 6.0
                        };
                        SUN << 6 | (ring as u32 & 63) as u8
                    } else {
                        // Dusk sky gradient, never cycled.
                        STATIC << 6 | ((v / 0.75).min(1.0) * 31.0) as u8
                    }
                };
                self.scene[y * w + x] = idx;
            }
        }
    }
}

/// Water: deep blue with lighter swells and a band of white foam.
fn water(i: usize, flash: f32) -> Rgb {
    let p = i as f32 / 64.0 * TAU;
    let b = 0.35 + 0.22 * (p * 2.0).sin() + 0.12 * (p * 5.0).sin();
    let foam = if i % 32 < 3 { 0.5 } else { 0.0 };
    rgb(
        0.05 + 0.5 * b * b + foam + flash * 0.2,
        0.18 + 0.6 * b + foam * 0.9 + flash * 0.2,
        0.4 + 0.55 * b + foam * 0.7 + flash * 0.2,
    )
}

impl Effect for ColorCycle {
    fn name(&self) -> &'static str {
        "Color Cycle"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if (c.w, c.h) != (self.w, self.h) {
            (self.w, self.h) = (c.w, c.h);
            self.build();
        }
        self.falls += dt * (10.0 + 45.0 * f.level + 20.0 * f.pulse);
        self.pool += dt * (6.0 + 25.0 * f.level);
        self.sun += dt * (3.0 + 18.0 * f.mid);

        // Rebuild the four 64-entry palettes, rotated by their offsets.
        let flash = f.pulse * 0.6;
        let off = |v: f32| (v.rem_euclid(64.0) as usize) % 64;
        let (fo, po, so) = (off(self.falls), off(self.pool), off(self.sun));
        let mut pal = [[0u8; 3]; 256];
        for i in 0..64 {
            let k = i as f32 / 31.0;
            pal[i] = if i < 32 {
                rgb(0.06 + 0.85 * k * k, 0.04 + 0.4 * k * k, 0.22 + 0.15 * k)
            } else {
                let k = (i - 32) as f32 / 31.0;
                rgb(0.08 + 0.3 * k, 0.05 + 0.2 * k, 0.1 + 0.18 * k)
            };
            pal[64 + i] = water((i + 64 - fo) % 64, flash);
            pal[192 + i] = water((i + 64 - po) % 64, flash * 0.5);
            let p = ((i + 64 - so) % 64) as f32 / 64.0 * TAU;
            let b = 0.6 + 0.4 * (p * 4.0).sin();
            pal[128 + i] = rgb(1.0, 0.45 + 0.4 * b + flash * 0.3, 0.1 + 0.3 * b * b + flash * 0.3);
        }
        for (p, &i) in c.px.iter_mut().zip(&self.scene) {
            *p = pal[i as usize];
        }
    }
}
