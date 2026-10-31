use super::Effect;
use super::polygons::stroke;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

/// One ring of interlaced waves: `curves` copies of r(θ) = center + amp ·
/// sin(waves θ + phase) + amp2 · sin(waves2 θ), each shifted by a fraction of a wave.
struct Band {
    center: f32,
    amp: f32,
    waves: f32,
    amp2: f32,
    waves2: f32,
    curves: usize,
    hue: f32,
}

const BANDS: [Band; 4] = [
    Band {
        center: 0.2,
        amp: 0.13,
        waves: 5.0,
        amp2: 0.03,
        waves2: 10.0,
        curves: 7,
        hue: 0.0,
    },
    Band {
        center: 0.47,
        amp: 0.14,
        waves: 9.0,
        amp2: 0.04,
        waves2: 3.0,
        curves: 9,
        hue: 0.12,
    },
    Band {
        center: 0.74,
        amp: 0.1,
        waves: 14.0,
        amp2: 0.04,
        waves2: 4.0,
        curves: 8,
        hue: 0.3,
    },
    Band {
        center: 0.93,
        amp: 0.04,
        waves: 26.0,
        amp2: 0.01,
        waves2: 6.0,
        curves: 4,
        hue: 0.45,
    },
];

/// Banknote-style guilloché: a rosette of fine interlaced sine rings plus
/// wavy border ribbons, with phases and amplitudes slowly drifting. Mid
/// swells the main waves, treble the secondary ripple, bass the radius, and
/// the pulse makes the engraving glow.
pub struct Guilloche {
    phase: f32,
    drift: f32,
    hue: f32,
}

impl Guilloche {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            drift: 0.0,
            hue: 0.33,
        }
    }
}

impl Effect for Guilloche {
    fn name(&self) -> &'static str {
        "Guilloche"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fill([4, 6, 5]);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.phase += dt * (0.25 + 0.8 * f.mid);
        self.drift += dt * 0.07;
        self.hue += dt * 0.01 + if f.beat { 0.03 } else { 0.0 };

        let (w, h) = (c.w as f32, c.h as f32);
        let borders = h >= 40.0;
        let (cx, cy) = (w * 0.5, h * 0.5);
        let radius = w.min(h) * 0.5 * if borders { 0.8 } else { 0.97 } * (1.0 + 0.05 * f.bass);
        let width = (radius / 120.0).clamp(0.55, 1.0);
        // Small rosettes pack the lines tighter, so they get dimmer ink.
        let glow = (0.4 + 0.3 * f.level + 0.3 * f.pulse) * (radius / 50.0).clamp(0.35, 1.0);
        let steps = ((radius * TAU / 3.0) as usize).clamp(48, 300);

        for (bi, b) in BANDS.iter().enumerate() {
            let breathe = 1.0 + 0.3 * (self.drift * (1.0 + bi as f32 * 0.37)).sin();
            let amp = b.amp * breathe * (0.7 + 0.6 * f.mid);
            let amp2 = b.amp2 * (0.5 + 1.2 * f.treble) * (self.drift * 1.3 + bi as f32).cos();
            let dir = if bi % 2 == 0 { 1.0 } else { -1.0 };
            let steps = steps.max(b.waves as usize * 5);
            for k in 0..b.curves {
                let shift = TAU * k as f32 / b.curves as f32;
                let value = glow * (0.75 + 0.25 * (shift + t).sin());
                let col = hsv(self.hue + b.hue + 0.04 * (shift * 2.0).sin(), 0.65, value.min(1.0));
                let point = |s: usize| {
                    let th = TAU * s as f32 / steps as f32;
                    let r = b.center
                        + amp * (b.waves * th + shift + dir * self.phase).sin()
                        + amp2 * (b.waves2 * th - self.phase * 0.5).sin();
                    let a = th + dir * self.drift * 0.3;
                    (cx + a.cos() * r * radius, cy + a.sin() * r * radius)
                };
                let mut prev = point(0);
                for s in 1..=steps {
                    let p = point(s);
                    stroke(c, prev.0, prev.1, p.0, p.1, width, col);
                    prev = p;
                }
            }
        }

        if borders {
            // Interlaced ribbons along the top and bottom edges.
            let band_h = h * 0.06;
            let period = (h * 0.18).max(6.0);
            let xsteps = ((w / 1.5) as usize).max(8);
            for (side, yc) in [band_h + 1.0, h - band_h - 1.0].into_iter().enumerate() {
                for k in 0..7 {
                    let shift = TAU * k as f32 / 7.0;
                    let col = hsv(self.hue + 0.6, 0.6, (glow * 0.8).min(1.0));
                    let dir = if side == 0 { 1.0 } else { -1.0 };
                    let point = |s: usize| {
                        let x = w * s as f32 / xsteps as f32;
                        let y = yc + band_h * (0.8 + 0.2 * f.mid) * (x / period * TAU + shift + dir * self.phase).sin();
                        (x, y)
                    };
                    let mut prev = point(0);
                    for s in 1..=xsteps {
                        let p = point(s);
                        stroke(c, prev.0, prev.1, p.0, p.1, width, col);
                        prev = p;
                    }
                }
            }
        }
    }
}
