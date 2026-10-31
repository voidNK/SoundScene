use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, scale};
use std::f32::consts::FRAC_PI_2;

/// Size ratio and rotation between one nested square and the next. With
/// Q * (cos A + sin A) < 1 each square fits inside its parent.
const Q: f32 = 0.78;
const A: f32 = 0.3;
/// Table of levels relative to the current zoom position.
const LO: i32 = -8;
const LEVELS: usize = 56;

/// Endless zoom into nested, rotated squares. Level drives the zoom speed,
/// every square glows with its own spectrum bin, beats flash the palette.
pub struct InfiniteZoom {
    /// Whole levels zoomed so far and the fraction of the current one.
    base: i64,
    frac: f32,
    spin: f32,
    hue: f32,
    flash: f32,
}

impl InfiniteZoom {
    pub fn new() -> Self {
        Self {
            base: 0,
            frac: 0.0,
            spin: 0.0,
            hue: 0.0,
            flash: 0.0,
        }
    }
}

impl Effect for InfiniteZoom {
    fn name(&self) -> &'static str {
        "Infinite Zoom"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.frac += dt * (0.4 + f.level * 2.2 + f.pulse * 0.6);
        let whole = self.frac.floor();
        self.frac -= whole;
        self.base = self.base.wrapping_add(whole as i64);
        self.spin += dt * (0.1 + f.mid * 0.4);
        self.hue += dt * 0.02;
        self.flash = if f.beat { 1.0 } else { (self.flash - dt * 3.0).max(0.0) };
        if c.w == 0 || c.h == 0 {
            return;
        }

        // Level n (relative) has half-size Q^(n - frac) and rotation (base + n) * A.
        let base_rot = (self.base.rem_euclid(10_000) as f32 * A).rem_euclid(FRAC_PI_2) - self.spin;
        let table: Vec<(f32, f32, f32)> = (0..LEVELS)
            .map(|k| {
                let n = k as i32 + LO;
                let (s, c) = (base_rot + n as f32 * A).sin_cos();
                (c, s, Q.powf(n as f32 - self.frac))
            })
            .collect();
        let bins = f.spectrum.len();
        let colors: Vec<_> = (0..LEVELS)
            .map(|k| {
                let n = self.base + k as i64 + LO as i64;
                let band = if bins > 0 {
                    f.spectrum[n.rem_euclid(bins as i64) as usize]
                } else {
                    0.0
                };
                let v = 0.45 + 0.4 * band + 0.15 * self.flash;
                hsv(
                    self.hue + n.rem_euclid(12) as f32 / 12.0,
                    0.75 - 0.3 * self.flash,
                    v.min(1.0),
                )
            })
            .collect();

        let norm = 2.0 / c.w.min(c.h * 2).max(1) as f32;
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let ln_q = Q.ln();
        for y in 0..c.h {
            let py = (y as f32 + 0.5 - cy) * norm;
            for x in 0..c.w {
                let px = (x as f32 + 0.5 - cx) * norm;
                let r = (px * px + py * py).sqrt().max(1e-4);
                // Start at a level whose square surely contains the pixel, then
                // walk inward while the next one still does.
                let start = ((r.ln() / ln_q + self.frac).floor() as i32 - 2 - LO).clamp(0, LEVELS as i32 - 1) as usize;
                let mut k = start;
                let mut edge = 1.0;
                while k + 1 < LEVELS {
                    let (c, s, half) = table[k + 1];
                    let m = (px * c + py * s).abs().max((py * c - px * s).abs());
                    if m > half {
                        break;
                    }
                    edge = m / half;
                    k += 1;
                }
                // Darken towards the inner border for a beveled look.
                let shade = 1.0 - 0.6 * edge * edge * edge;
                c.set(x, y, scale(colors[k], shade));
            }
        }
    }
}
