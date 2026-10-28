use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv};

const MAX_BOBS: usize = 10;
const FREQ: [(f32, f32); MAX_BOBS] = [
    (1.0, 2.0),
    (2.0, 3.0),
    (3.0, 2.0),
    (3.0, 4.0),
    (2.0, 5.0),
    (5.0, 3.0),
    (4.0, 5.0),
    (3.0, 1.0),
    (5.0, 4.0),
    (1.0, 3.0),
];

/// Classic shadebobs: soft bobs fly along Lissajous paths and add into a
/// heat buffer that wraps around a 256-entry palette, leaving banded,
/// slowly fading trails. Mids add bobs and speed them up, the level and
/// beats decide how hot they burn, bass makes them bigger.
pub struct Shadebobs {
    w: usize,
    h: usize,
    buf: Vec<f32>,
    phase: f32,
    hue: f32,
}

impl Shadebobs {
    pub fn new() -> Self {
        Self {
            w: 0,
            h: 0,
            buf: Vec::new(),
            phase: 0.0,
            hue: 0.6,
        }
    }
}

impl Effect for Shadebobs {
    fn name(&self) -> &'static str {
        "Shadebobs"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if (c.w, c.h) != (self.w, self.h) {
            (self.w, self.h) = (c.w, c.h);
            self.buf = vec![0.0; c.w * c.h];
        }
        if c.w == 0 || c.h == 0 {
            return;
        }
        self.phase += dt * (0.5 + 1.5 * f.mid);
        self.hue += dt * 0.03 + if f.beat { 0.04 } else { 0.0 };

        let decay = 0.99f32.powf(dt * 30.0);
        for v in &mut self.buf {
            *v *= decay;
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let r = (w.min(h) * (0.09 + 0.05 * f.bass + 0.04 * f.pulse)).max(1.0);
        let amount = (10.0 + 16.0 * f.level + if f.beat { 30.0 } else { 0.0 }) * (dt * 30.0).min(4.0);
        let bobs = (3 + (f.mid * 7.0).round() as usize).min(MAX_BOBS);
        let (ax, ay) = ((w * 0.5 - r).max(0.0), (h * 0.5 - r).max(0.0));
        for (i, &(fx, fy)) in FREQ.iter().enumerate().take(bobs) {
            let k = i as f32;
            let p = self.phase * (0.5 + k * 0.04);
            let bx = w * 0.5 + ax * (p * fx + k * 0.7).sin();
            let by = h * 0.5 + ay * (p * fy + k * 1.3).cos();
            // Bobs stay on screen: the path amplitude is the half size minus r.
            let x0 = (bx - r).floor().max(0.0) as usize;
            let x1 = ((bx + r).ceil().max(0.0) as usize).min(c.w - 1);
            let y0 = (by - r).floor().max(0.0) as usize;
            let y1 = ((by + r).ceil().max(0.0) as usize).min(c.h - 1);
            let inv = 1.0 / (r * r);
            for y in y0..=y1 {
                let dy = y as f32 + 0.5 - by;
                for x in x0..=x1 {
                    let dx = x as f32 + 0.5 - bx;
                    let q = 1.0 - (dx * dx + dy * dy) * inv;
                    if q > 0.0 {
                        self.buf[y * c.w + x] += amount * q;
                    }
                }
            }
        }

        // Dark at both ends so wrapping from 255 to 0 stays seamless.
        let palette: Vec<Rgb> = (0..256)
            .map(|i| {
                let v = i as f32 / 256.0;
                let tri = 1.0 - (2.0 * v - 1.0).abs();
                hsv(self.hue + v * 0.6, 1.0 - tri.powi(3) * 0.75, tri.powf(0.9))
            })
            .collect();
        for (p, &v) in c.px.iter_mut().zip(&self.buf) {
            *p = palette[(v as u32 & 255) as usize];
        }
    }
}
