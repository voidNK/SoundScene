use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, add, hsv, scale};
use std::f32::consts::TAU;

const RAYS: usize = 32;

/// The spectrum as rays around a glowing sun, mirrored so both halves run
/// from bass to treble. The wheel turns with the mids, peak caps fall back
/// slowly, the core swells with bass and beats, and rays leave an afterglow.
pub struct RadialSpectrum {
    level: [f32; RAYS],
    peak: [f32; RAYS],
    rot: f32,
    hue: f32,
}

impl RadialSpectrum {
    pub fn new() -> Self {
        Self {
            level: [0.0; RAYS],
            peak: [0.0; RAYS],
            rot: 0.0,
            hue: 0.0,
        }
    }
}

impl Effect for RadialSpectrum {
    fn name(&self) -> &'static str {
        "Radial Spectrum"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.rot += dt * (0.15 + 0.6 * f.mid);
        self.hue += dt * 0.05 + if f.beat { 0.07 } else { 0.0 };
        let bins = f.spectrum.len();
        for i in 0..RAYS {
            let v = if bins == 0 {
                0.0
            } else {
                let a = i * bins / RAYS;
                let b = ((i + 1) * bins / RAYS).clamp(a + 1, bins);
                let v = f.spectrum[a.min(bins - 1)..b].iter().fold(0.0f32, |m, &x| m.max(x));
                // Tilt up the highs, which are usually much weaker than the bass.
                (v * (0.7 + 0.8 * i as f32 / RAYS as f32)).min(1.0)
            };
            let l = &mut self.level[i];
            *l += (v - *l) * (1.0 - (-18.0 * dt).exp());
            self.peak[i] = (self.peak[i] - dt * 0.35).max(*l);
        }

        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let rmax = c.w.min(c.h) as f32 * 0.5;
        let core = rmax * (0.2 + 0.08 * f.bass + 0.1 * f.pulse);
        // Rays may reach past the short edge of a wide canvas.
        let span = (c.w.max(c.h) as f32 * 0.42 - core).max(0.0);
        let core_col = hsv(self.hue + 0.1, 0.35, 1.0);

        for y in 0..c.h {
            let dy = y as f32 + 0.5 - cy;
            for x in 0..c.w {
                let dx = x as f32 + 0.5 - cx;
                let r = (dx * dx + dy * dy).sqrt();
                let mut col = if r < core {
                    let k = r / core.max(0.01);
                    scale(core_col, 1.0 - 0.5 * k * k)
                } else {
                    // Angle 0 at the top, mirrored left/right.
                    let a = ((dx.atan2(-dy) / TAU + self.rot).rem_euclid(1.0) - 0.5).abs() * 2.0;
                    let pos = a * RAYS as f32;
                    let i = (pos as usize).min(RAYS - 1);
                    let len = core + span * (0.04 + 0.96 * self.level[i]);
                    let cap = core + span * (0.04 + 0.96 * self.peak[i]);
                    let gap = pos.fract();
                    let hue = self.hue + i as f32 / RAYS as f32 * 0.5;
                    let halo = (core * 1.8 / r.max(1.0)).powi(2) * (0.25 + 0.5 * f.pulse);
                    let glow = scale(core_col, halo.min(1.0) * 0.6);
                    if !(0.2..=0.8).contains(&gap) {
                        glow
                    } else if r < len {
                        let k = (r - core) / span.max(0.01);
                        add(hsv(hue + k * 0.15, 0.85 - 0.4 * (1.0 - k), 0.45 + 0.55 * k), glow)
                    } else if (r - cap).abs() < 1.0 {
                        hsv(hue, 0.3, 1.0)
                    } else {
                        glow
                    }
                };
                // Afterglow: keep a dimmed copy of the previous frame.
                let old = scale(c.get(x, y), 0.55);
                col = [col[0].max(old[0]), col[1].max(old[1]), col[2].max(old[2])];
                c.set(x, y, col);
            }
        }
    }
}
