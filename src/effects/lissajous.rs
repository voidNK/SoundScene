use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

const STEPS: usize = 700;

/// A slowly morphing Lissajous figure with trails. Loudness sets its size,
/// bass the speed.
pub struct Lissajous {
    phase: f32,
    hue: f32,
}

impl Lissajous {
    pub fn new() -> Self {
        Self { phase: 0.0, hue: 0.0 }
    }
}

impl Effect for Lissajous {
    fn name(&self) -> &'static str {
        "Lissajous"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.82);
        self.phase += dt * (0.4 + f.bass * 2.0);
        self.hue += dt * 0.05 + if f.beat { 0.1 } else { 0.0 };

        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let ax = c.w as f32 * 0.45 * (0.5 + 0.5 * f.level);
        let ay = c.h as f32 * 0.45 * (0.5 + 0.5 * f.level);
        let a = 3.0;
        let b = 4.0 + 0.5 * (t * 0.07).sin();

        let mut prev: Option<(f32, f32)> = None;
        for i in 0..=STEPS {
            let s = i as f32 / STEPS as f32 * TAU;
            let x = cx + ax * (a * s + self.phase).sin();
            let y = cy + ay * (b * s).sin();
            if let Some((px, py)) = prev {
                c.line_add(px, py, x, y, hsv(self.hue + s / TAU * 0.5, 0.7, 0.15));
            }
            prev = Some((x, y));
        }
    }
}
