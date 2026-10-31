use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::Canvas;
use std::f32::consts::TAU;

const CELL: usize = 2;
const MAX_ITER: u32 = 28;

/// Multibrot set z ← z^d + c whose exponent d morphs continuously between
/// 2 and 6, so the set grows and loses lobes. Mids push the morph, bass
/// pumps the zoom, beats brighten the smooth palette.
pub struct Multibrot {
    colors: Vec<[f32; 3]>,
    morph: f32,
    rot: f32,
    hue: f32,
}

impl Multibrot {
    pub fn new() -> Self {
        Self {
            colors: Vec::new(),
            morph: 0.0,
            rot: 0.0,
            hue: 0.0,
        }
    }
}

impl Effect for Multibrot {
    fn name(&self) -> &'static str {
        "Multibrot"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.morph += dt * (0.08 + 0.15 * f.mid);
        self.rot += dt * 0.06;
        self.hue += dt * (0.03 + 0.05 * f.treble);
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        if gw == 0 || gh == 0 {
            return;
        }
        self.colors.resize(gw * gh, [0.0; 3]);

        let d = 4.0 - 2.0 * self.morph.cos();
        let ln_d = d.ln();
        let step = 3.0 / (gw.min(gh).max(1) as f32) * (1.0 - 0.15 * f.bass);
        // The set has d - 1 lobes; for d near 2 it is offset to the left.
        let shift = -0.6 * (6.0 - d).max(0.0) / 4.0;
        let (s, co) = self.rot.sin_cos();
        let bright = 0.8 + 0.4 * f.pulse;

        for gy in 0..gh {
            let dy = (gy as f32 - gh as f32 * 0.5) * step;
            for gx in 0..gw {
                let dx = (gx as f32 - gw as f32 * 0.5) * step;
                let (cr, ci) = (shift + dx * co - dy * s, dx * s + dy * co);
                let (mut zr, mut zi) = (cr, ci);
                let mut n = 1;
                let mut m = zr * zr + zi * zi;
                while n < MAX_ITER && m < 16.0 {
                    // z^d in polar form; m > 0 is guaranteed unless z is exactly 0.
                    let r = (0.5 * d * m.max(1e-30).ln()).exp();
                    let (sa, ca) = (d * zi.atan2(zr)).sin_cos();
                    zr = r * ca + cr;
                    zi = r * sa + ci;
                    m = zr * zr + zi * zi;
                    n += 1;
                }
                self.colors[gy * gw + gx] = if n >= MAX_ITER {
                    [0.0, 0.0, 0.03]
                } else {
                    let sm = n as f32 + 1.0 - (0.5 * m.ln()).max(1e-6).ln() / ln_d;
                    let t = self.hue + sm.max(0.0).sqrt() * 0.3;
                    let v = (sm / 7.0).min(1.0).sqrt() * bright;
                    [
                        v * (0.5 + 0.5 * (TAU * t).cos()),
                        v * (0.5 + 0.5 * (TAU * (t + 0.2)).cos()),
                        v * (0.6 + 0.4 * (TAU * (t + 0.45)).cos()),
                    ]
                };
            }
        }
        upscale(c, &self.colors, gw, gh);
    }
}
