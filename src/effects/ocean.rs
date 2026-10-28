use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, mix, rgb, scale};

/// Horizon as a fraction of the height.
const HORIZON: f32 = 0.42;
const CAM_HEIGHT: f32 = 1.0;
const Z_FAR: f32 = 60.0;
/// Wave components: direction x, direction z, wave number, angular speed, amplitude.
const WAVES: [(f32, f32, f32, f32, f32); 5] = [
    (0.3, 0.95, 0.9, 1.1, 0.5),
    (-0.6, 0.8, 1.4, 1.5, 0.3),
    (0.85, 0.5, 2.3, 2.0, 0.15),
    (-0.2, 0.98, 3.7, 2.7, 0.08),
    (0.6, 0.8, 7.1, 3.9, 0.04),
];

fn sky(g: f32) -> Rgb {
    // g: 0 = zenith, 1 = horizon.
    let g = g.clamp(0.0, 1.0);
    let top = rgb(0.06, 0.03, 0.18);
    let middle = rgb(0.45, 0.12, 0.35);
    let low = rgb(1.0, 0.5, 0.22);
    if g < 0.6 {
        mix(top, middle, g / 0.6)
    } else {
        mix(middle, low, (g - 0.6) / 0.4)
    }
}

/// Ocean at sunset: a perspective heightfield of summed sine waves, drawn
/// front to back per column so crests hide the water behind them. Each facet
/// reflects the sky at its tilt, with a glittering sun path. Bass raises the
/// swell, beats add an extra surge and foam, treble makes the glitter sparkle.
pub struct Ocean {
    amp: f32,
    time: f32,
}

impl Ocean {
    pub fn new() -> Self {
        Self { amp: 0.15, time: 0.0 }
    }
}

impl Effect for Ocean {
    fn name(&self) -> &'static str {
        "Ocean"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let target = 0.1 + 0.45 * f.bass + 0.15 * f.pulse;
        self.amp += (target - self.amp) * (1.0 - (-dt * 4.0).exp());
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (w, h) = (c.w as f32, c.h as f32);
        let (t, amp) = (self.time, self.amp);
        let hz = h * HORIZON;
        let horizon = (hz as usize).min(c.h);
        let horizon_col = sky(1.0);

        // Sky with sun and halo.
        let sr = (h * 0.1 * (1.0 + 0.15 * f.bass + 0.1 * f.pulse)).max(1.0);
        let (sx, sy) = (w * (0.5 + 0.12 * (t * 0.05).sin()), hz - sr * 0.25);
        for y in 0..horizon {
            let base = sky(y as f32 / hz.max(1.0));
            for x in 0..c.w {
                let (dx, dy) = (x as f32 - sx, y as f32 - sy);
                let d = (dx * dx + dy * dy).sqrt();
                let col = if d < sr {
                    mix(rgb(1.0, 0.95, 0.65), rgb(1.0, 0.65, 0.3), d / sr)
                } else {
                    let halo = (-(d - sr) / (sr * 1.2)).exp() * (0.35 + 0.3 * f.pulse);
                    mix(base, rgb(1.0, 0.7, 0.4), halo)
                };
                c.set(x, y, col);
            }
        }

        // Water: march away from the camera, filling each column upwards.
        let fy = (h - hz).max(1.0) * CAM_HEIGHT;
        let fx = h.max(1.0) * 0.8;
        let crest = amp * 0.8;
        let path_w = sr * 1.4;
        let sparkle = 0.6 + 0.8 * f.treble;
        for x in 0..c.w {
            let u = (x as f32 - w * 0.5) / fx;
            let path = (-((x as f32 - sx) / path_w).powi(2)).exp();
            let mut ymax = c.h;
            let (mut z, mut prev_z, mut prev_hgt) = (1.0f32, 0.9f32, 0.0f32);
            let mut first = true;
            while z < Z_FAR && ymax > horizon {
                let wx = u * z;
                let mut hgt = 0.0;
                for &(dx, dz, k, speed, a) in &WAVES {
                    hgt += a * (k * (dx * wx + dz * z) + speed * t).sin();
                }
                hgt *= amp;
                if first {
                    prev_hgt = hgt;
                    first = false;
                }
                let top = (hz + (CAM_HEIGHT - hgt) * fy / z).ceil().max(0.0) as usize;
                if top < ymax {
                    let slope = (hgt - prev_hgt) / (z - prev_z).max(1e-3);
                    let view = CAM_HEIGHT / z;
                    // Elevation of the reflected ray: facets facing away reflect the horizon.
                    let elev = view + 2.0 * slope;
                    let low = ((0.2 - elev) / 0.2).clamp(0.0, 1.0);
                    let refl = mix(rgb(0.04, 0.07, 0.18), rgb(1.0, 0.5, 0.24), low * low);
                    let graze = 1.0 - view.min(1.0);
                    let fresnel = 0.12 + 0.78 * graze * graze;
                    let mut col = mix(rgb(0.01, 0.04, 0.1), refl, fresnel);
                    // Facets facing the camera fall into shadow.
                    col = scale(col, 1.0 - 0.45 * (slope * 1.5).clamp(0.0, 1.0));
                    if hgt > crest {
                        col = mix(
                            col,
                            rgb(0.85, 0.8, 0.85),
                            (hgt - crest) / (amp * 0.4) * (0.4 + 0.6 * f.pulse),
                        );
                    }
                    col = mix(col, horizon_col, (z / Z_FAR).powf(0.8));
                    let glint = path * (-((elev - 0.03) / 0.1).powi(2)).exp() * sparkle;
                    col = mix(col, rgb(1.0, 0.88, 0.6), glint * 1.3);
                    for y in top..ymax {
                        c.set(x, y, col);
                    }
                    ymax = top;
                }
                prev_z = z;
                prev_hgt = hgt;
                z *= 1.035;
            }
            for y in horizon..ymax {
                c.set(x, y, horizon_col);
            }
        }
    }
}
