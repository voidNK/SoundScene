use super::Effect;
use super::mirrorbars::sample;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

/// Maximum points per trace; small canvases use fewer to avoid overdraw.
const POINTS: usize = 360;
/// Waveform repeats around the circle; even, so the mirrored segments close.
const PETALS: usize = 6;

/// The waveform plotted in polar coordinates, repeated and mirrored into a
/// rotating rosette that leaves colored trails. Loudness opens the petals,
/// mids spin it, beats widen it; a second counter-rotating inner flower
/// follows the bass.
pub struct PolarScope {
    rot: f32,
    hue: f32,
}

impl PolarScope {
    pub fn new() -> Self {
        Self { rot: 0.0, hue: 0.0 }
    }
}

impl Effect for PolarScope {
    fn name(&self) -> &'static str {
        "Polar Scope"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.76);
        self.rot += dt * (0.25 + f.mid * 1.2 + f.pulse * 1.5);
        self.hue += dt * 0.04;
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let big_r = c.w.min(c.h) as f32 * 0.47 * (1.0 + 0.08 * f.pulse);
        let gain = 0.6 + 0.7 * f.level;
        // Idle wobble so the flower still breathes without input.
        let idle = 0.15 * (t * 0.9).sin();
        // Small tiles get fewer pixels per line, so keep them from saturating.
        let dim = (c.w.min(c.h) as f32 / 60.0).clamp(0.5, 1.0);

        for layer in 0..2 {
            let (radius, rot, gain, hue_off, off) = if layer == 0 {
                (big_r, self.rot, gain, 0.0, 0.0)
            } else {
                (big_r * (0.35 + 0.2 * f.bass), -self.rot * 1.6, gain * 0.8, 0.5, 0.37)
            };
            let points = (radius * 2.5).clamp(48.0, POINTS as f32) as usize;
            let point = |k: usize| {
                let theta = k as f32 / points as f32 * TAU;
                let x = k as f32 / points as f32 * PETALS as f32;
                let (s, p) = (x as usize, x.fract());
                let q = if s % 2 == 1 { 1.0 - p } else { p };
                let v = sample(&f.waveform, off + q * 0.3);
                let petal = (theta * PETALS as f32 * 0.5).sin().abs();
                let r = radius * (0.45 + 0.4 * (v * gain + idle * petal)).clamp(0.02, 1.0);
                let a = theta + rot;
                (cx + a.cos() * r, cy + a.sin() * r)
            };
            let mut prev = point(0);
            for k in 1..=points {
                let cur = point(k % points);
                let theta = k as f32 / points as f32;
                let col = hsv(
                    self.hue + hue_off + theta * 0.5,
                    0.85 - 0.3 * f.pulse,
                    dim * (0.4 + 0.25 * f.level + 0.2 * f.pulse),
                );
                c.line_add(prev.0, prev.1, cur.0, cur.1, col);
                prev = cur;
            }
        }
    }
}
