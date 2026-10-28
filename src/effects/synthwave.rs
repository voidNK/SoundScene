use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, rgb};

/// Retro sunset: striped sun, spectrum mountains and a scrolling neon grid.
/// Bass drives the grid and pumps the sun, beats flash the grid.
pub struct Synthwave {
    offset: f32,
}

impl Synthwave {
    pub fn new() -> Self {
        Self { offset: 0.0 }
    }
}

impl Effect for Synthwave {
    fn name(&self) -> &'static str {
        "Synthwave"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.offset += dt * (0.8 + f.bass * 4.0);
        let (w, h) = (c.w as f32, c.h as f32);
        let horizon = ((h * 0.58) as usize).min(c.h);
        let hz = horizon as f32;
        let cx = w * 0.5;

        for y in 0..horizon {
            let g = y as f32 / hz.max(1.0);
            let col = rgb(0.10 + 0.55 * g * g, 0.02 + 0.12 * g * g, 0.18 + 0.12 * (1.0 - g));
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }

        let sr = h * 0.22 * (1.0 + 0.12 * f.bass + 0.08 * f.pulse);
        let sun_y = hz - sr * 0.35;
        for y in (sun_y - sr).max(0.0) as usize..horizon {
            let dy = y as f32 - sun_y;
            if dy.abs() > sr {
                continue;
            }
            // Gaps in the lower half that get wider towards the horizon.
            if dy > 0.0 && (dy / sr * 6.0 + self.offset * 0.5).rem_euclid(1.0) < dy / sr * 0.6 {
                continue;
            }
            let half = (sr * sr - dy * dy).sqrt();
            let g = (dy + sr) / (2.0 * sr);
            let col = rgb(1.0, 0.85 - 0.6 * g, 0.35 - 0.2 * g);
            for x in (cx - half).max(0.0) as usize..((cx + half) as usize).min(c.w) {
                c.set(x, y, col);
            }
        }

        // Mountains: the spectrum mirrored around the center, bass in the middle.
        let bins = f.spectrum.len();
        for x in 0..c.w {
            let d = ((x as f32 - cx).abs() / (w * 0.5)).min(1.0);
            let v = f.spectrum[(d * (bins - 1) as f32) as usize];
            let mountain = (v * h * 0.2) as usize;
            for y in horizon.saturating_sub(mountain)..horizon {
                c.set(x, y, rgb(0.15, 0.02, 0.25));
            }
        }

        let flash = 0.7 + 0.5 * f.pulse;
        for y in horizon..c.h {
            let z = (h - hz).max(1.0) / (y - horizon + 1) as f32;
            let glow = flash / (1.0 + z * 0.25);
            let row_line = (z + self.offset).rem_euclid(1.0) < 0.12;
            for x in 0..c.w {
                let col_line = ((x as f32 - cx) / w * 6.0 * z).rem_euclid(1.0) < 0.06;
                let col = if row_line || col_line {
                    rgb(glow, 0.2 * glow, 0.9 * glow)
                } else {
                    rgb(0.04, 0.0, 0.08)
                };
                c.set(x, y, col);
            }
        }
    }
}
