use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

/// Rotating, zooming checkerboard with inset rings. Bass zooms in, mids spin it.
pub struct Rotozoom {
    angle: f32,
    zoom_phase: f32,
    hue: f32,
}

impl Rotozoom {
    pub fn new() -> Self {
        Self { angle: 0.0, zoom_phase: 0.0, hue: 0.1 }
    }
}

impl Effect for Rotozoom {
    fn name(&self) -> &'static str {
        "Rotozoom"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.angle += dt * (0.2 + f.mid * 1.5);
        self.zoom_phase += dt * 0.5;
        self.hue += dt * 0.02 + if f.beat { 0.05 } else { 0.0 };

        let zoom = ((1.0 + 0.5 * self.zoom_phase.sin()) * (1.0 - 0.4 * f.bass)).max(0.2);
        let scale = zoom * 5.0 / c.w.min(c.h).max(1) as f32;
        let (sin, cos) = self.angle.sin_cos();
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let flash = 0.7 + 0.3 * f.pulse;

        for y in 0..c.h {
            let dy = y as f32 - cy;
            for x in 0..c.w {
                let dx = x as f32 - cx;
                let u = (dx * cos - dy * sin) * scale + self.angle * 2.0;
                let v = (dx * sin + dy * cos) * scale;
                let checker = (u.floor() as i64 + v.floor() as i64) & 1 == 0;
                let ring = ((u.rem_euclid(1.0) - 0.5).powi(2) + (v.rem_euclid(1.0) - 0.5).powi(2)).sqrt();
                let col = if checker {
                    hsv(self.hue, 0.7, (0.95 - ring) * flash)
                } else {
                    hsv(self.hue + 0.5, 0.7, (0.25 + ring * 0.5) * flash)
                };
                c.set(x, y, col);
            }
        }
    }
}
