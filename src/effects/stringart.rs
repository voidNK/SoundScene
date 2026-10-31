use super::Effect;
use super::polygons::stroke;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, scale};
use std::f32::consts::TAU;

/// Times-table string art: points on a circle, point i joined to point i·k.
/// The multiplier k drifts slowly through cardioids, nephroids and denser
/// rosettes, faster with bass. Each glowing additive string is lit by its own
/// spectrum band, and the pulse brightens the whole web.
pub struct StringArt {
    phase: f32,
    spin: f32,
    hue: f32,
}

impl StringArt {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            spin: 0.0,
            hue: 0.0,
        }
    }
}

impl Effect for StringArt {
    fn name(&self) -> &'static str {
        "String Art"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fill([0, 0, 0]);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.phase += dt * (0.025 + 0.06 * f.bass + 0.02 * f.pulse);
        self.spin += dt * 0.05;
        self.hue += dt * (0.02 + 0.06 * f.treble);
        // Ping-pong between k = 2 and k = 34 without ever jumping.
        let k = 2.0 + 32.0 * (0.5 - 0.5 * (self.phase).cos());

        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let r = c.w.min(c.h) as f32 * 0.5 * 0.95;
        let n = ((r * 2.4) as usize).clamp(24, 260);
        // Dense webs saturate quickly, so dim each string with the count.
        let dim = (90.0 / n as f32).sqrt().min(1.0);
        let bins = f.spectrum.len();
        let point = |i: f32| {
            let a = TAU * i / n as f32 + self.spin;
            (cx + a.cos() * r, cy + a.sin() * r)
        };
        for i in 0..n {
            let band = if bins > 0 {
                f.spectrum[(i * bins / n).min(bins - 1)]
            } else {
                0.0
            };
            let (x0, y0) = point(i as f32);
            let (x1, y1) = point(i as f32 * k);
            let v = (0.3 + 0.6 * band + 0.3 * f.pulse) * dim;
            let col = hsv(self.hue + i as f32 / n as f32, 0.75, v.min(1.0));
            stroke(c, x0, y0, x1, y1, 1.0, col);
        }
        // Pins around the rim.
        let pin = scale(hsv(self.hue, 0.3, 1.0), 0.5 + 0.5 * f.level);
        for i in 0..n {
            let (x, y) = point(i as f32);
            c.plot_add(x as i32, y as i32, pin);
        }
    }
}
