use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix, scale};

/// Amiga Kefrens/Alcatraz bars: a shaded bar is drawn into a single line
/// buffer once per scanline at a sine-wobbling position, and the buffer is
/// never cleared, so each row inherits the bars above it and they pile up
/// into a twisting wall. Bass widens the wobble, beats flash the bars, the
/// spectrum tints the rows.
pub struct KefrensBars {
    line: Vec<Rgb>,
    phase: f32,
    swing: f32,
}

impl KefrensBars {
    pub fn new() -> Self {
        Self {
            line: Vec::new(),
            phase: 0.0,
            swing: 0.3,
        }
    }
}

impl Effect for KefrensBars {
    fn name(&self) -> &'static str {
        "Kefrens Bars"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        let (w, h) = (c.w, c.h);
        self.phase += dt * (1.2 + 1.5 * f.mid + 2.0 * f.pulse);
        let target = 0.18 + 0.22 * f.bass;
        self.swing += (target - self.swing) * (1.0 - (-5.0 * dt).exp());

        let (wf, hf) = (w as f32, h as f32);
        let bar_w = (wf / 14.0).max(2.0);
        let amp = (wf - bar_w) * self.swing.min(0.5);
        let top = (hf * 0.08) as usize;
        let bins = f.spectrum.len();

        // Bar sprite: a lit cylinder with a highlight, as brightness 0..1.
        let n = bar_w.ceil() as usize;
        let sprite: Vec<f32> = (0..n)
            .map(|i| {
                let u = (i as f32 + 0.5) / bar_w * 2.0 - 1.0;
                let body = (1.0 - u * u).max(0.0).sqrt();
                let spec = (-((u + 0.35) * 5.0).powi(2)).exp();
                (0.15 + 0.7 * body + 0.5 * spec).min(1.3)
            })
            .collect();

        // Backdrop the bars are piled onto: a dim vertical-stripe gradient.
        self.line.clear();
        self.line
            .extend((0..w).map(|x| hsv(0.68, 0.8, 0.06 + 0.04 * (x as f32 / wf.max(1.0) * 12.0).sin().abs())));
        for y in 0..h {
            let yf = y as f32;
            if y < top {
                // Thin copper strip above the wall.
                let k = 1.0 - (yf / top.max(1) as f32 - 0.5).abs() * 2.0;
                let col = hsv(0.6 + t * 0.05, 0.7, 0.15 + 0.5 * k * k);
                for x in 0..w {
                    c.set(x, y, col);
                }
                continue;
            }
            let row = yf - top as f32;
            let band = if bins == 0 {
                0.0
            } else {
                f.spectrum[((row / hf.max(1.0)) * bins as f32) as usize % bins]
            };
            for k in 0..2 {
                let ph = self.phase * (1.0 + 0.3 * k as f32) + k as f32 * 2.1;
                // The second bar mirrors the first's motion so they weave around each other.
                let sign = if k == 0 { 1.0 } else { -1.0 };
                let pos = sign * ((row * 0.045 + ph).sin() * 0.65 + (row * 0.018 - ph * 0.7).sin() * 0.35);
                let x0 = wf * 0.5 + pos * amp - bar_w * 0.5;
                let hue = t * 0.04 + row / hf.max(1.0) * 0.6 + k as f32 * 0.5;
                let base = hsv(hue, 0.75, 0.6 + 0.4 * band);
                let flash = f.pulse * 0.3;
                for (i, &b) in sprite.iter().enumerate() {
                    let x = x0.floor() as i64 + i as i64;
                    if x >= 0 && (x as usize) < w {
                        let col = mix(
                            scale(base, b.min(1.0)),
                            [255, 255, 255],
                            (b - 1.0).max(0.0) * 2.0 + flash * b,
                        );
                        self.line[x as usize] = col;
                    }
                }
            }
            c.px[y * w..(y + 1) * w].copy_from_slice(&self.line);
        }
    }
}
