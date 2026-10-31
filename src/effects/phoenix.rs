use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

const MAX_ITER: u32 = 44;

/// Phoenix fractal, a Julia set that also feeds back the previous iterate:
/// z ← z² + c + p·z₋₁. Both constants drift on slow orbits around the
/// classic (0.5667, -0.5); loudness widens the orbit, bass zooms, beats
/// flash the plumage.
pub struct Phoenix {
    drift: f32,
    hue: f32,
    level: f32,
}

impl Phoenix {
    pub fn new() -> Self {
        Self {
            drift: 0.0,
            hue: 0.0,
            level: 0.0,
        }
    }
}

impl Effect for Phoenix {
    fn name(&self) -> &'static str {
        "Phoenix"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.level += (f.level - self.level) * (1.0 - (-3.0 * dt).exp());
        self.drift += dt * (0.12 + 0.25 * self.level);
        self.hue += dt * 0.025;
        if c.w == 0 || c.h == 0 {
            return;
        }

        let amp = 0.02 + 0.04 * self.level;
        let cr = 0.5667 + amp * (self.drift * 0.7).sin();
        let ci = amp * 0.6 * (self.drift * 1.1).cos();
        let pr = -0.5 + amp * 1.2 * (self.drift * 0.9 + 1.0).cos();
        let pi = amp * 0.5 * (self.drift * 0.5).sin();
        let zoom = 1.9 / c.w.min(c.h).max(1) as f32 * (1.0 - 0.12 * f.bass);
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let flash = 0.75 + 0.35 * f.pulse;

        for y in 0..c.h {
            for x in 0..c.w {
                // Classic orientation: the real axis runs vertically.
                let (mut zr, mut zi) = ((cy - y as f32) * zoom, (x as f32 - cx) * zoom);
                let (mut yr, mut yi) = (0.0f32, 0.0f32);
                let mut n = 0;
                while n < MAX_ITER && zr * zr + zi * zi < 16.0 {
                    let nr = zr * zr - zi * zi + cr + pr * yr - pi * yi;
                    let ni = 2.0 * zr * zi + ci + pr * yi + pi * yr;
                    (yr, yi) = (zr, zi);
                    (zr, zi) = (nr, ni);
                    n += 1;
                }
                let col = if n == MAX_ITER {
                    // Inside: the final orbit angle gives the feathers a texture.
                    let m = (zr * zr + zi * zi).sqrt().min(1.0);
                    let a = zi.atan2(zr) / std::f32::consts::PI;
                    hsv(self.hue + 0.5 + 0.15 * a, 0.75, (0.2 + 0.5 * m * m) * flash)
                } else {
                    let s = n as f32 + 1.0 - (zr * zr + zi * zi).sqrt().max(1.0001).ln().log2();
                    let k = (s / MAX_ITER as f32 * 3.0).min(1.0);
                    hsv(self.hue + 0.05 + s * 0.03, 0.9 - 0.4 * k * k, k.powf(0.8) * flash)
                };
                c.set(x, y, col);
            }
        }
    }
}
