use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, mix};

const ROWS: usize = 56;
const COLS: usize = 64;
/// New spectrum rows per second.
const RATE: f32 = 24.0;
const NEAR: f32 = 1.5;
const DEPTH: f32 = 0.1;

/// A perspective landscape of dots: columns are frequency bands (bass in
/// the middle), rows are the spectrum history scrolling into the distance.
/// Height and color follow the band level, beats light up the front row.
pub struct DotWave {
    history: Vec<f32>,
    head: usize,
    acc: f32,
    hue: f32,
    sway: f32,
}

impl DotWave {
    pub fn new() -> Self {
        Self {
            history: vec![0.0; ROWS * COLS],
            head: 0,
            acc: 0.0,
            hue: 0.0,
            sway: 0.0,
        }
    }

    /// Spectrum value for column `k`, mirrored so bass sits in the center.
    fn column(f: &Features, k: usize) -> f32 {
        let bins = f.spectrum.len();
        if bins == 0 {
            return 0.0;
        }
        let off = (k as f32 - (COLS as f32 - 1.0) * 0.5).abs() * 2.0 / COLS as f32;
        f.spectrum[((off * bins as f32) as usize).min(bins - 1)]
    }
}

impl Effect for DotWave {
    fn name(&self) -> &'static str {
        "Dot Wave"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.acc += dt * RATE;
        let mut pushes = 0;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            pushes += 1;
            if pushes <= ROWS {
                self.head = (self.head + ROWS - 1) % ROWS;
                for k in 0..COLS {
                    self.history[self.head * COLS + k] = Self::column(f, k);
                }
            }
        }
        self.hue += dt * 0.02 + if f.beat { 0.04 } else { 0.0 };
        self.sway += dt * (0.2 + 0.3 * f.mid);

        c.clear();
        let (w, h) = (c.w as f32, c.h as f32);
        let cx = w * 0.5 + w * 0.06 * self.sway.sin();
        let horizon = h * 0.18;
        let s = h * 0.9;
        let cam = 1.2 + 0.1 * (t * 0.3).sin();
        let span = 0.9 * (w / h.max(1.0)).max(1.0);
        let amp = 1.3 + 0.5 * f.pulse;
        let frac = self.acc.min(1.0);

        // Back to front so near dots cover far ones.
        for row in (0..ROWS).rev() {
            let z = NEAR + (row as f32 + frac) * DEPTH;
            let fog = 1.0 - row as f32 / ROWS as f32;
            for k in 0..COLS {
                let v = if row == 0 {
                    Self::column(f, k)
                } else {
                    self.history[((self.head + row) % ROWS) * COLS + k]
                };
                let x = (k as f32 / (COLS - 1) as f32 * 2.0 - 1.0) * span;
                let y = v * amp;
                let sx = cx + x / z * s;
                let sy = horizon + (cam - y) / z * s;
                let mut col = hsv(
                    self.hue + 0.65 - 0.6 * v,
                    0.8 - 0.3 * v * v,
                    (0.2 + 0.8 * v) * fog * fog + 0.08,
                );
                if row == 0 {
                    col = mix(col, [255, 255, 255], 0.5 * f.pulse);
                }
                let (px, py) = (sx.round() as i32, sy.round() as i32);
                c.plot(px, py, col);
                if z < 2.4 {
                    c.plot(px + 1, py, col);
                    c.plot(px, py + 1, col);
                    c.plot(px + 1, py + 1, col);
                }
            }
        }
    }
}
