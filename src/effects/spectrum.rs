use super::Effect;
use crate::analysis::{Features, SPECTRUM_BINS};
use crate::gfx::{Canvas, hsv};

/// Classic spectrum analyzer bars with falling peak caps.
pub struct Spectrum {
    peaks: Vec<f32>,
}

impl Spectrum {
    pub fn new() -> Self {
        Self {
            peaks: vec![0.0; SPECTRUM_BINS],
        }
    }
}

impl Effect for Spectrum {
    fn name(&self) -> &'static str {
        "Spectrum"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fill(hsv(0.72, 0.7, 0.04 + 0.12 * f.pulse));
        let n = f.spectrum.len();
        if n == 0 || c.w == 0 || c.h == 0 {
            return;
        }
        for (p, &v) in self.peaks.iter_mut().zip(&f.spectrum) {
            *p = if v > *p { v } else { (*p - dt * 0.35).max(0.0) };
        }

        let hf = c.h as f32;
        for bin in 0..n {
            let x0 = bin * c.w / n;
            let x1 = (bin + 1) * c.w / n;
            // The last column of a wide bar stays empty as the gap to the next one.
            let x_end = if x1 - x0 >= 3 { x1 - 1 } else { x1 };
            let height = ((f.spectrum[bin] * hf) as usize).min(c.h);
            let peak_y = (((1.0 - self.peaks[bin]) * (hf - 1.0)) as usize).min(c.h - 1);
            for x in x0..x_end {
                for i in 0..height {
                    let frac = i as f32 / hf;
                    c.set(
                        x,
                        c.h - 1 - i,
                        hsv(0.62 - frac * 0.62 + t * 0.03, 0.85, 0.35 + 0.65 * frac),
                    );
                }
                c.set(x, peak_y, [240, 240, 255]);
            }
        }
    }
}
