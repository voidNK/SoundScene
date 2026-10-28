use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

const RINGS: usize = 16;
const CORE: f32 = 0.12;

/// Circular equalizer: 16 concentric rings, each an arc whose length follows
/// one band. Neighboring rings spin in opposite directions.
pub struct Rings {
    rot: f32,
    hue: f32,
}

impl Rings {
    pub fn new() -> Self {
        Self { rot: 0.0, hue: 0.0 }
    }
}

impl Effect for Rings {
    fn name(&self) -> &'static str {
        "Rings"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.rot += dt * (0.3 + f.mid);
        self.hue += dt * 0.03 + if f.beat { 0.05 } else { 0.0 };

        let bins = f.spectrum.len();
        let levels: Vec<f32> = (0..RINGS)
            .map(|i| {
                let band = &f.spectrum[i * bins / RINGS..(i + 1) * bins / RINGS];
                band.iter().sum::<f32>() / band.len() as f32
            })
            .collect();
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let rmax = c.w.min(c.h) as f32 * 0.48;

        for y in 0..c.h {
            let dy = y as f32 - cy;
            for x in 0..c.w {
                let dx = x as f32 - cx;
                let r = (dx * dx + dy * dy).sqrt() / rmax.max(1.0);
                let col = if r < CORE {
                    hsv(self.hue, 0.5, 0.2 + 0.8 * f.pulse)
                } else if r >= 1.0 {
                    [0, 0, 0]
                } else {
                    let pos = (r - CORE) / (1.0 - CORE) * RINGS as f32;
                    let i = (pos as usize).min(RINGS - 1);
                    let ring_hue = self.hue + i as f32 / RINGS as f32 * 0.6;
                    if pos.fract() > 0.75 {
                        [0, 0, 0]
                    } else {
                        let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
                        let a = ((dy.atan2(dx) + self.rot * dir * (1.0 + i as f32 * 0.1)) / TAU).rem_euclid(1.0);
                        if a < levels[i] {
                            hsv(ring_hue, 0.8, 0.5 + 0.5 * levels[i])
                        } else {
                            hsv(ring_hue, 0.6, 0.07)
                        }
                    }
                };
                c.set(x, y, col);
            }
        }
    }
}
