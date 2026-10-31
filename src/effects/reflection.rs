use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, mix, rgb, scale};

/// A sunset sky with mountains mirrored in rippling water. Bass drives the
/// ripples, level the sun's glow, beats send a swell through the water.
pub struct Reflection {
    /// Sky rows rendered this frame, reused for the mirror image.
    sky: Vec<Rgb>,
    phase: f32,
    drift: f32,
    swell: f32,
}

impl Reflection {
    pub fn new() -> Self {
        Self {
            sky: Vec::new(),
            phase: 0.0,
            drift: 0.0,
            swell: 0.0,
        }
    }
}

/// Mountain ridge height (0..1 of the sky) at horizontal position `u`.
fn ridge(u: f32) -> f32 {
    0.18 + 0.1 * (u * 2.1).sin() + 0.06 * (u * 5.3 + 1.0).sin() + 0.03 * (u * 13.0).sin()
}

impl Effect for Reflection {
    fn name(&self) -> &'static str {
        "Reflection"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.phase += dt * (2.0 + f.bass * 4.0);
        self.drift += dt * 0.05;
        self.swell = if f.beat { 1.0 } else { (self.swell - dt * 1.2).max(0.0) };
        if c.w == 0 || c.h == 0 {
            return;
        }

        let (w, h) = (c.w, c.h);
        let horizon = (h * 3 / 5).max(1);
        if self.sky.len() != w * horizon {
            self.sky = vec![[0; 3]; w * horizon];
        }

        // Sky: vertical gradient, sun disc with a halo, mountain silhouettes.
        let hf = horizon as f32;
        let unit = 1.0 / h as f32;
        let (sun_x, sun_y) = (w as f32 * 0.5 + (t * 0.05).sin() * w as f32 * 0.15, hf * 0.8);
        let sun_r = h as f32 * (0.14 + 0.03 * f.level);
        let top = rgb(0.08, 0.04, 0.22);
        let mid = rgb(0.75, 0.25, 0.35);
        let low = rgb(1.0, 0.65, 0.25);
        for y in 0..horizon {
            let k = y as f32 / hf;
            let grad = if k < 0.6 {
                mix(top, mid, k / 0.6)
            } else {
                mix(mid, low, (k - 0.6) / 0.4)
            };
            for x in 0..w {
                let (dx, dy) = (x as f32 - sun_x, y as f32 - sun_y);
                let d = (dx * dx + dy * dy).sqrt() / sun_r.max(1.0);
                let mut col = grad;
                if d < 1.0 {
                    // Sun with the classic horizontal cut-outs in its lower half.
                    let cut = dy > 0.0 && ((dy / sun_r.max(1.0) * 6.0).fract() < dy / sun_r.max(1.0) * 0.6);
                    if !cut {
                        col = mix(
                            rgb(1.0, 0.95, 0.5),
                            rgb(1.0, 0.45, 0.3),
                            (dy / sun_r.max(1.0) + 1.0) * 0.5,
                        );
                    }
                } else {
                    let halo = (1.0 - (d - 1.0) * 0.6).max(0.0) * (0.25 + 0.35 * f.level);
                    col = mix(col, rgb(1.0, 0.8, 0.5), halo * halo);
                }
                let u = x as f32 * unit * 3.0;
                let far = ridge(u + self.drift + 7.0) * 0.8;
                let near = ridge(u * 1.6 + self.drift * 2.0) * 0.45;
                let height = (hf - y as f32) / hf;
                if height < near {
                    col = rgb(0.06, 0.02, 0.1);
                } else if height < far {
                    col = mix(col, rgb(0.25, 0.08, 0.3), 0.85);
                }
                self.sky[y * w + x] = col;
                c.set(x, y, col);
            }
        }

        // Water: mirror the sky with per-line wobble that grows towards the viewer.
        let amp = 0.6 + f.bass * 3.0 + self.swell * 2.0;
        for y in horizon..h {
            let d = (y - horizon) as f32 + 1.0;
            let depth = d / (h - horizon).max(1) as f32;
            // Waves get wider towards the viewer: phase follows the water's distance.
            let dist = 1.0 / (depth + 0.08);
            let wave = (dist * 2.5 - self.phase * 2.0).sin() + 0.5 * (dist * 1.3 + self.phase).sin();
            let shift = wave * amp * (0.3 + depth * 2.0) * (w as f32 / 120.0).max(0.3);
            let ry = horizon as f32 - d - wave * amp * 0.4 * depth;
            let sy = (ry.round().max(0.0) as usize).min(horizon - 1);
            let dark = 0.85 - 0.3 * depth;
            for x in 0..w {
                let sx = (x as f32 + shift + (y as f32 * 0.7 + x as f32 * 0.05 + self.phase).sin() * amp * 0.5)
                    .round()
                    .clamp(0.0, (w - 1) as f32) as usize;
                let src = self.sky[sy * w + sx];
                let tinted = mix(scale(src, dark), rgb(0.05, 0.1, 0.25), 0.25 + 0.3 * depth);
                c.set(x, y, tinted);
            }
        }
    }
}
