use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, mix};
use std::f32::consts::TAU;

const MODES: usize = 3;
const MODE_TIME: f32 = 9.0;
const FADE_TIME: f32 = 1.5;

/// Black-and-white op art with a faint tint: wavy stripes, a checkerboard
/// and off-center rings, all bulging out of the screen like a lens. Bass and
/// the pulse inflate the bulge, mid bends the stripes, a beat flashes the tint.
pub struct OpArt {
    time: f32,
    scroll: f32,
    hue: f32,
    flash: f32,
}

impl OpArt {
    pub fn new() -> Self {
        Self {
            time: 0.0,
            scroll: 0.0,
            hue: 0.55,
            flash: 0.0,
        }
    }

    /// Pattern value in -1..1 for mode `m` at (bulged) coordinates (u, v).
    fn pattern(&self, m: usize, u: f32, v: f32, freq: f32, bend: f32) -> f32 {
        let s = self.scroll;
        match m {
            0 => ((u + bend * (v * 2.3 + s * 0.7).sin() * 0.25) * freq + s).sin(),
            1 => {
                let (sn, cs) = (s * 0.1).sin_cos();
                let (ru, rv) = (u * cs - v * sn, u * sn + v * cs);
                (ru * freq + s * 0.5).sin() * (rv * freq).sin() * 3.0
            }
            _ => {
                let (ox, oy) = ((s * 0.21).sin() * 0.35, (s * 0.17).cos() * 0.25);
                let d = ((u - ox).powi(2) + (v - oy).powi(2)).sqrt();
                (d * freq * 1.2 - s * 1.5 + bend * (v * 3.0).sin() * 0.6).sin()
            }
        }
    }
}

impl Effect for OpArt {
    fn name(&self) -> &'static str {
        "Op Art"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        self.scroll += dt * (0.8 + 1.5 * f.bass);
        self.hue += dt * 0.02;
        self.flash = if f.beat { 1.0 } else { self.flash * (-dt * 4.0).exp() };

        let cycle = self.time / MODE_TIME;
        let mode = cycle as usize % MODES;
        let next = (mode + 1) % MODES;
        let into = self.time - cycle.floor() * MODE_TIME;
        let blend = ((into - (MODE_TIME - FADE_TIME)) / FADE_TIME).clamp(0.0, 1.0);

        let scale = c.w.min(c.h) as f32 * 0.5;
        let inv = 1.0 / scale.max(1.0);
        // Aim for ten periods across the radius, but never narrower than 5 px.
        let periods = (scale / 5.0).clamp(1.0, 10.0);
        let freq = TAU * periods;
        // Pattern value change per pixel decides how sharp an edge can be.
        let sharp = (scale / freq).max(0.3);
        let bulge = 0.4 + 1.1 * f.bass + 0.6 * f.pulse;
        let bend = 0.6 + 1.4 * f.mid;
        let (cx, cy) = (
            c.w as f32 * 0.5 + (self.time * 0.3).sin() * scale * 0.25,
            c.h as f32 * 0.5 + (self.time * 0.23).cos() * scale * 0.15,
        );
        let sat = 0.12 + 0.45 * self.flash;
        let dark = hsv(self.hue + 0.5, 0.6, 0.04);
        let light = hsv(self.hue, sat, 0.95);

        for y in 0..c.h {
            let v0 = (y as f32 + 0.5 - cy) * inv;
            for x in 0..c.w {
                let u0 = (x as f32 + 0.5 - cx) * inv;
                let r2 = u0 * u0 + v0 * v0;
                let k = 1.0 / (1.0 + bulge * (-r2 * 1.8).exp());
                let (u, v) = (u0 * k, v0 * k);
                let mut p = self.pattern(mode, u, v, freq, bend);
                if blend > 0.0 {
                    p += (self.pattern(next, u, v, freq, bend) - p) * blend;
                }
                // Edges get sharper where the bulge magnifies the pattern.
                let val = (0.5 + p * sharp / k.max(0.2)).clamp(0.0, 1.0);
                c.set(x, y, mix(dark, light, val));
            }
        }
    }
}
