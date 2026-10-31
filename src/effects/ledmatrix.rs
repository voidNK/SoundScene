use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, add, scale};
use std::collections::VecDeque;

const MAX_COLS: usize = 256;
const MODES: u32 = 3;

/// A panel of round LEDs with soft glow, unlit LEDs faintly visible. It
/// cycles through three shows every eight beats: a scrolling mirrored level
/// envelope, a live spectrum analyzer with peak dots and an oscilloscope
/// trace of the waveform. The scroll speed follows the level, beats brighten the
/// whole panel.
pub struct LedMatrix {
    /// Envelope values, newest column first.
    history: VecDeque<f32>,
    scroll: f32,
    peaks: Vec<f32>,
    beats: u32,
    /// Lookup per pixel offset inside one LED cell: core and glow weight.
    pitch: usize,
    spot: Vec<(f32, f32)>,
    lit: Vec<(f32, Rgb)>,
}

impl LedMatrix {
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
            scroll: 0.0,
            peaks: Vec::new(),
            beats: 0,
            pitch: 0,
            spot: Vec::new(),
            lit: Vec::new(),
        }
    }
}

/// Green at the bottom of the scale, then amber, then red.
fn level_color(p: f32) -> Rgb {
    if p < 0.55 {
        [40, 255, 80]
    } else if p < 0.8 {
        [255, 190, 30]
    } else {
        [255, 50, 40]
    }
}

impl Effect for LedMatrix {
    fn name(&self) -> &'static str {
        "LED Matrix"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.beats += 1;
        }
        let mode = (self.beats / 8) % MODES;
        self.scroll += dt * (8.0 + 22.0 * f.level);
        let mut added = 0;
        while self.scroll >= 1.0 {
            self.scroll -= 1.0;
            if added < 16 {
                let env = (0.6 * f.level + 0.35 * f.pulse + 0.06 * (t * 3.0).sin()).clamp(0.0, 1.0);
                self.history.push_front(env);
                added += 1;
            }
        }
        self.history.truncate(MAX_COLS);
        if c.w == 0 || c.h == 0 {
            return;
        }

        let p = if c.h >= 18 && c.w >= 18 {
            (c.h / 24).clamp(3, 8)
        } else {
            2
        };
        if p != self.pitch {
            self.pitch = p;
            let r = p as f32 * 0.42;
            self.spot = (0..p * p)
                .map(|i| {
                    let (x, y) = ((i % p) as f32 + 0.5, (i / p) as f32 + 0.5);
                    let d = (x - p as f32 * 0.5).hypot(y - p as f32 * 0.5);
                    let core = ((r - d) * 1.5 + 0.5).clamp(0.0, 1.0);
                    let glow = (-(d * d) / (r * r * 1.2)).exp();
                    if p == 2 {
                        (if i == 0 { 1.0 } else { 0.0 }, 0.0)
                    } else {
                        (core, glow)
                    }
                })
                .collect();
        }
        let (cols, rows) = (c.w / p, c.h / p);
        if cols == 0 || rows == 0 {
            c.clear();
            return;
        }
        let (ox, oy) = ((c.w - cols * p) / 2, (c.h - rows * p) / 2);

        // Per LED: brightness and color.
        self.lit.clear();
        self.lit.resize(cols * rows, (0.0, [40, 255, 80]));
        self.peaks.resize(cols, 0.0);
        let bins = f.spectrum.len();
        let rh = rows as f32;
        for col in 0..cols {
            match mode {
                0 => {
                    let env = self.history.get(cols - 1 - col).copied().unwrap_or(0.0);
                    let half = env * rh * 0.5;
                    for row in 0..rows {
                        let d = (row as f32 + 0.5 - rh * 0.5).abs();
                        if d < half.max(0.5) {
                            self.lit[row * cols + col] = (1.0, level_color(d / (rh * 0.5)));
                        }
                    }
                }
                1 => {
                    let v = if bins == 0 {
                        0.0
                    } else {
                        f.spectrum[(col * bins / cols).min(bins - 1)]
                    };
                    let height = v * rh;
                    let pk = &mut self.peaks[col];
                    *pk = (*pk - dt * 0.6 * rh).max(height);
                    for row in 0..rows {
                        let from_bottom = (rows - 1 - row) as f32;
                        let colr = level_color(from_bottom / rh);
                        if from_bottom < height {
                            self.lit[row * cols + col] = (1.0, colr);
                        } else if *pk >= 1.0 && from_bottom as usize == *pk as usize {
                            self.lit[row * cols + col] = (0.9, [255, 80, 60]);
                        }
                    }
                }
                _ => {
                    let n = f.waveform.len();
                    let gain = 0.45 * (0.4 + 0.6 * f.level + 0.3 * f.pulse);
                    let at = |i: usize| {
                        if n == 0 {
                            0.0
                        } else {
                            f.waveform[(i * n / cols).min(n - 1)]
                        }
                    };
                    let row_of = |v: f32| ((0.5 - v * gain) * rh).clamp(0.0, rh - 1.0) as usize;
                    let (a, b) = (row_of(at(col)), row_of(at(col.saturating_sub(1))));
                    for row in a.min(b)..=a.max(b) {
                        self.lit[row * cols + col] = (1.0, [80, 200, 255]);
                    }
                }
            }
        }

        let gain = 0.8 + 0.3 * f.pulse;
        c.fill([5, 5, 7]);
        for row in 0..rows {
            for col in 0..cols {
                let (v, colr) = self.lit[row * cols + col];
                let off = scale(colr, 0.09);
                let on = scale(colr, v * gain);
                for dy in 0..p {
                    let base = (oy + row * p + dy) * c.w + ox + col * p;
                    for dx in 0..p {
                        let (core, glow) = self.spot[dy * p + dx];
                        let px = add(scale(off, core), scale(on, core.max(glow * 0.45)));
                        c.px[base + dx] = add(px, [5, 5, 7]);
                    }
                }
            }
        }
    }
}
