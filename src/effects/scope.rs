use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Circular oscilloscope with mirrored spectrum spikes around it.
pub struct Scope {
    rot: f32,
    hue: f32,
}

impl Scope {
    pub fn new() -> Self {
        Self { rot: 0.0, hue: 0.3 }
    }
}

impl Effect for Scope {
    fn name(&self) -> &'static str {
        "Scope"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fade(0.7);
        self.rot += dt * (0.2 + f.mid * 1.5);
        self.hue += dt * 0.05 + if f.beat { 0.1 } else { 0.0 };

        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let base = c.w.min(c.h) as f32 * 0.2 * (1.0 + 0.15 * f.pulse);

        let n = f.waveform.len();
        if n >= 2 {
            let point = |i: usize| {
                let a = i as f32 / n as f32 * TAU + self.rot;
                let r = base * (1.0 + f.waveform[i] * 0.5);
                (cx + a.cos() * r, cy + a.sin() * r)
            };
            let mut prev = point(n - 1);
            for i in 0..n {
                let p = point(i);
                let col = hsv(self.hue + i as f32 / n as f32 * 0.3, 0.7, 0.5);
                c.line_add(prev.0, prev.1, p.0, p.1, col);
                prev = p;
            }
        }

        let m = f.spectrum.len();
        let inner = base * 1.55;
        for (j, &v) in f.spectrum.iter().enumerate() {
            let offset = j as f32 / m as f32 * PI;
            let col = hsv(self.hue + 0.5 + v * 0.2, 0.8, 0.3 + 0.7 * v);
            let outer = inner + v * base * 0.8;
            for a in [FRAC_PI_2 + offset, FRAC_PI_2 - offset] {
                let (dx, dy) = (a.cos(), a.sin());
                c.line_add(cx + dx * inner, cy + dy * inner, cx + dx * outer, cy + dy * outer, col);
            }
        }
    }
}
