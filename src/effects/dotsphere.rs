use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::PI;

const POINTS: usize = 700;

/// A rotating sphere of dots that grows spikes with the spectrum: bass at
/// the bottom pole, treble at the top.
pub struct DotSphere {
    rot: f32,
    hue: f32,
}

impl DotSphere {
    pub fn new() -> Self {
        Self { rot: 0.0, hue: 0.6 }
    }
}

impl Effect for DotSphere {
    fn name(&self) -> &'static str {
        "DotSphere"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.3);
        self.rot += dt * (0.4 + f.mid * 1.5);
        self.hue += dt * 0.03 + if f.beat { 0.06 } else { 0.0 };

        let tilt = 0.4 * (t * 0.3).sin();
        let (st, ct) = tilt.sin_cos();
        let (sr, cr) = self.rot.sin_cos();
        let golden = PI * (3.0 - 5f32.sqrt());
        let bins = f.spectrum.len();
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let radius = c.w.min(c.h) as f32 * 0.33 * (1.0 + 0.15 * f.pulse);

        for i in 0..POINTS {
            // Fibonacci sphere: evenly spread points.
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / POINTS as f32;
            let r = (1.0 - y * y).sqrt();
            let theta = golden * i as f32;
            let bin = ((1.0 - y) * 0.5 * (bins - 1) as f32) as usize;
            let lift = 1.0 + 0.35 * f.spectrum[bin];
            let (x, y, z) = (theta.cos() * r * lift, y * lift, theta.sin() * r * lift);
            let (x, z) = (x * cr + z * sr, -x * sr + z * cr);
            let (y, z) = (y * ct - z * st, y * st + z * ct);
            let p = 3.0 / (z + 3.0);
            let brightness = ((1.0 - z) * 0.5).clamp(0.15, 1.0);
            let col = hsv(self.hue + (y + 1.0) * 0.2, 0.6, brightness);
            c.plot_add((cx + x * radius * p).round() as i32, (cy + y * radius * p).round() as i32, col);
        }
    }
}
