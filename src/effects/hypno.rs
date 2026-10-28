use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

/// Hypnotic logarithmic spiral bands that endlessly pour into the center.
/// Bass speeds the rotation, the level widens the bright bands, beats flash
/// them and every eighth beat changes the number of arms.
pub struct HypnoSpiral {
    phase: f32,
    arms: f32,
    beats: usize,
    hue: f32,
}

impl HypnoSpiral {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            arms: 3.0,
            beats: 0,
            hue: 0.75,
        }
    }
}

impl Effect for HypnoSpiral {
    fn name(&self) -> &'static str {
        "Hypno Spiral"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.beats += 1;
            // Whole numbers only, a fractional arm count leaves a seam.
            if self.beats.is_multiple_of(8) {
                self.arms = 2.0 + ((self.arms as usize - 1) % 4) as f32;
            }
        }
        self.phase += dt * (0.4 + 2.6 * f.bass + 1.0 * f.pulse);
        self.hue += dt * 0.02;

        let (w, h) = (c.w as f32, c.h as f32);
        let m = w.min(h).max(1.0);
        let (cx, cy) = (
            w * 0.5 + m * 0.04 * (t * 0.7).sin(),
            h * 0.5 + m * 0.04 * (t * 0.9).cos(),
        );
        let tight = 1.6 + 0.6 * (t * 0.1).sin();
        let duty = 0.3 + 0.45 * f.level;
        let light = hsv(self.hue, 0.25 + 0.5 * f.pulse, 1.0);
        let dark = hsv(self.hue + 0.5, 0.8, 0.1);
        let arms = self.arms;
        let center_glow = 0.35 + 0.65 * f.pulse;

        for y in 0..c.h {
            let dy = y as f32 + 0.5 - cy;
            for x in 0..c.w {
                let dx = x as f32 + 0.5 - cx;
                let r = (dx * dx + dy * dy).sqrt().max(0.5);
                let a = dy.atan2(dx);
                let v = arms * a / TAU + tight * (r / m).ln() + self.phase;
                let fr = v.rem_euclid(1.0);
                // Band edges are antialiased with the pattern's change per pixel.
                let fw = ((arms / TAU).powi(2) + tight * tight).sqrt() / r;
                let s = if fr < duty {
                    fr.min(duty - fr)
                } else {
                    -(fr - duty).min(1.0 - fr)
                };
                let tone = (s / fw.max(1e-4) + 0.5).clamp(0.0, 1.0);
                let fade = (r / (m * 0.06)).min(1.0);
                let tone = tone * fade + center_glow * (1.0 - fade);
                c.set(x, y, crate::gfx::mix(dark, light, tone));
            }
        }
    }
}
