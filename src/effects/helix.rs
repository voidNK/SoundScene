use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

/// A DNA double helix scrolling sideways. The rungs light up with the
/// spectrum, bass widens it, mids spin it.
pub struct Helix {
    phase: f32,
    hue: f32,
}

impl Helix {
    pub fn new() -> Self {
        Self { phase: 0.0, hue: 0.5 }
    }
}

impl Effect for Helix {
    fn name(&self) -> &'static str {
        "Helix"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.4);
        self.phase += dt * (1.0 + f.mid * 3.0);
        self.hue += dt * 0.03 + if f.beat { 0.06 } else { 0.0 };

        let h = c.h as f32;
        let amp = h * 0.3 * (0.7 + 0.3 * f.bass + 0.2 * f.pulse);
        let bins = f.spectrum.len();

        for x in 0..c.w {
            let fx = x as f32;
            let a = fx * 0.09 + self.phase;
            let cy = h * 0.5 + (fx * 0.012 + t * 0.7).sin() * h * 0.1;
            let (y1, y2) = (cy + a.sin() * amp, cy - a.sin() * amp);
            if x % 5 == 0 {
                let v = f.spectrum[x * bins / c.w];
                c.line_add(fx, y1, fx, y2, hsv(self.hue + v * 0.3, 0.6, 0.15 + 0.5 * v));
            }
            // Draw the strand that is behind first.
            let front = a.cos();
            let strands = [(y1, self.hue, front), (y2, self.hue + 0.5, -front)];
            let order = if front > 0.0 { [1, 0] } else { [0, 1] };
            for i in order {
                let (y, hue, depth) = strands[i];
                let col = hsv(hue, 0.7, 0.55 + 0.45 * depth);
                c.plot(x as i32, y.round() as i32, col);
                c.plot(x as i32, y.round() as i32 + 1, col);
            }
        }
    }
}
