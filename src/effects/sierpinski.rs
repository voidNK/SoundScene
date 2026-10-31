use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

const SQ3_2: f32 = 0.866_025_4;
/// Fixed point of corner maps 0∘1∘2 in lattice coordinates (a, b): zooming
/// 8× around it maps the gasket onto itself, so the dive loops seamlessly.
const FIX: (f32, f32) = (2.0 / 7.0, 1.0 / 7.0);
/// View radius at the start of a loop; stays inside the outer triangle.
const RADIUS: f32 = 0.115;
const MAX_DEPTH: i32 = 18;

/// Endless dive into the Sierpinski triangle: every removed triangle glows
/// along its edges in a color set by its depth. Bass speeds up the zoom,
/// beats flash the gasket and kick the palette.
pub struct Sierpinski {
    zoom: f32,
    rot: f32,
    hue: f32,
}

impl Sierpinski {
    pub fn new() -> Self {
        Self {
            zoom: 0.0,
            rot: 0.0,
            hue: 0.0,
        }
    }
}

impl Effect for Sierpinski {
    fn name(&self) -> &'static str {
        "Sierpinski"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.zoom += dt * (0.1 + 0.3 * f.bass);
        self.rot += dt * (0.04 + 0.08 * f.mid);
        self.hue += dt * 0.03 + if f.beat { 0.08 } else { 0.0 };
        if c.w == 0 || c.h == 0 {
            return;
        }

        let frac = self.zoom.fract();
        let half_diag = 0.5 * ((c.w * c.w + c.h * c.h) as f32).sqrt().max(1.0);
        let px = RADIUS / half_diag * 8f32.powf(-frac);
        // Finest level whose triangles are still about a pixel wide.
        let max_depth = ((1.0 / (1.2 * px)).log2().ceil() as i32).clamp(1, MAX_DEPTH);
        let (s, co) = self.rot.sin_cos();
        let (fx, fy) = (FIX.0 + 0.5 * FIX.1, SQ3_2 * FIX.1);
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let flash = f.pulse * 0.5;
        let glow_w = 1.2 + 1.5 * f.level;

        for y in 0..c.h {
            for x in 0..c.w {
                let (dx, dy) = ((x as f32 - cx) * px, (cy - y as f32) * px);
                let wx = fx + dx * co - dy * s;
                let wy = fy + dx * s + dy * co;
                let mut b = wy / SQ3_2;
                let mut a = wx - 0.5 * b;
                let mut hit = None;
                if a < 0.0 || b < 0.0 || a + b > 1.0 {
                    hit = Some((0, 1.0));
                } else {
                    for k in 0..max_depth {
                        if a >= 0.5 {
                            a = 2.0 * a - 1.0;
                            b *= 2.0;
                        } else if b >= 0.5 {
                            a *= 2.0;
                            b = 2.0 * b - 1.0;
                        } else if a + b < 0.5 {
                            a *= 2.0;
                            b *= 2.0;
                        } else {
                            // Distance to the edge of the removed middle triangle.
                            let d = (0.5 - a).min(0.5 - b).min(a + b - 0.5);
                            hit = Some((k, d));
                            break;
                        }
                    }
                }
                let col = match hit {
                    Some((k, d)) => {
                        let size = 0.5f32.powi(k) / px;
                        let glow = (-d * SQ3_2 * size / glow_w).exp();
                        let rel = k as f32 - 3.0 * frac;
                        hsv(
                            self.hue + rel * 0.09,
                            0.8 - 0.3 * glow,
                            (0.08 + 0.8 * glow) * (1.0 + flash) + 0.05 * f.level,
                        )
                    }
                    None => hsv(self.hue + 0.5, 0.25, 0.75 + flash),
                };
                c.set(x, y, col);
            }
        }
    }
}
