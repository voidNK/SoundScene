use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix};
use std::f32::consts::TAU;

/// Classic scrolling, zooming and rotating XOR texture with palette cycling.
/// The spectrum tints the palette, bass pumps the zoom, beats jump the cycle.
pub struct XorPattern {
    cycle: f32,
    scroll: (f32, f32),
    angle: f32,
    palette: Vec<Rgb>,
}

impl XorPattern {
    pub fn new() -> Self {
        Self {
            cycle: 0.0,
            scroll: (0.0, 0.0),
            angle: 0.0,
            palette: vec![[0; 3]; 256],
        }
    }
}

impl Effect for XorPattern {
    fn name(&self) -> &'static str {
        "XOR Pattern"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.cycle += dt * (0.08 + f.level * 0.2) + if f.beat { 0.12 } else { 0.0 };
        self.scroll.0 = (self.scroll.0 + dt * 40.0 * (t * 0.11).cos()).rem_euclid(256.0);
        self.scroll.1 = (self.scroll.1 + dt * (30.0 + f.bass * 60.0)).rem_euclid(256.0);
        self.angle += dt * 0.1 * (t * 0.07).sin();
        if c.w == 0 || c.h == 0 {
            return;
        }

        // Palette: hue ramp cycling over time, each quarter-band lit by a spectrum bin.
        let bins = f.spectrum.len();
        for (i, p) in self.palette.iter_mut().enumerate() {
            let k = i as f32 / 256.0;
            let band = if bins > 0 { f.spectrum[i * bins / 256] } else { 0.0 };
            let v = 0.35 + 0.65 * (0.5 + 0.5 * ((k * 2.0 + self.cycle) * TAU).sin());
            let col = hsv(k + self.cycle, 0.8, v * (0.75 + 0.25 * band));
            *p = mix(col, hsv(k * 0.5 + 0.6, 0.3, 1.0), band * band * 0.4);
        }

        // Normalize so the pattern has about the same number of cells at any size.
        let zoom = (256.0 / c.h.max(1) as f32) * (0.6 + 0.25 * (t * 0.23).sin() + 0.25 * f.bass);
        let (s, co) = self.angle.sin_cos();
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        for y in 0..c.h {
            let dy = (y as f32 - cy) * zoom;
            for x in 0..c.w {
                let dx = (x as f32 - cx) * zoom;
                let u = (dx * co - dy * s + self.scroll.0).floor() as i32;
                let v = (dx * s + dy * co + self.scroll.1).floor() as i32;
                let idx = (u ^ v) & 255;
                c.set(x, y, self.palette[idx as usize]);
            }
        }
    }
}
