use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

const STARS: usize = 500;

struct Star {
    x: f32,
    y: f32,
    z: f32,
}

/// 3D starfield drawn as streaks. Bass and beats accelerate into warp.
pub struct Starfield {
    stars: Vec<Star>,
    rng: Rng,
}

impl Starfield {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xC0FFEE);
        let stars = (0..STARS)
            .map(|_| Star { x: rng.range(-1.0, 1.0), y: rng.range(-1.0, 1.0), z: rng.range(0.05, 1.0) })
            .collect();
        Self { stars, rng }
    }
}

impl Effect for Starfield {
    fn name(&self) -> &'static str {
        "Starfield"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fade(0.55 + 0.35 * f.pulse);
        let speed = 0.12 + f.bass * 0.9 + f.pulse * 1.2;
        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let scale = w.max(h) * 0.5;

        for (i, s) in self.stars.iter_mut().enumerate() {
            let z0 = s.z;
            s.z -= speed * dt;
            let x1 = cx + s.x / s.z * scale;
            let y1 = cy + s.y / s.z * scale;
            if s.z <= 0.02 || !(-w..2.0 * w).contains(&x1) || !(-h..2.0 * h).contains(&y1) {
                s.x = self.rng.range(-1.0, 1.0);
                s.y = self.rng.range(-1.0, 1.0);
                s.z = 1.0;
                continue;
            }
            let x0 = cx + s.x / z0 * scale;
            let y0 = cy + s.y / z0 * scale;
            let b = (1.0 - s.z).powi(2);
            let hue = 0.55 + f.treble * 0.4 + (i % 7) as f32 * 0.02;
            c.line_add(x0, y0, x1, y1, hsv(hue, 0.4 + 0.4 * f.treble, b));
        }
    }
}
