use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv};

/// Scrolling spectrogram: every frame the picture moves up one pixel and
/// the current spectrum is drawn along the bottom.
pub struct Waterfall;

impl Waterfall {
    pub fn new() -> Self {
        Self
    }
}

fn heat(v: f32) -> Rgb {
    hsv(0.7 - 0.75 * v, (1.0 - (v - 0.8).max(0.0) * 4.0).clamp(0.2, 1.0), v.powf(0.8))
}

impl Effect for Waterfall {
    fn name(&self) -> &'static str {
        "Waterfall"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, _dt: f32, f: &Features) {
        let (w, h) = (c.w, c.h);
        if w < 2 || h == 0 {
            return;
        }
        c.px.copy_within(w.., 0);
        let last = (f.spectrum.len() - 1) as f32;
        for x in 0..w {
            // Interpolate between bins so wide windows don't look blocky.
            let pos = x as f32 / (w - 1) as f32 * last;
            let i = pos as usize;
            let next = f.spectrum[(i + 1).min(last as usize)];
            let v = f.spectrum[i] + (next - f.spectrum[i]) * pos.fract();
            c.set(x, h - 1, heat(v));
        }
    }
}
