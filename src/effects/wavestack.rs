use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, mix, scale};
use std::collections::VecDeque;
use std::f32::consts::PI;

/// Samples per stored waveform line.
const RES: usize = 96;
const MAX_LINES: usize = 48;
/// Seconds between two snapshots.
const STEP: f32 = 0.06;

/// Waveform history stacked in perspective: the newest trace at the bottom,
/// older ones receding upwards and narrowing, each one hiding what is behind
/// it, colored from hot to cold by age. Loudness sets the amplitude, beats
/// brighten the newest line.
pub struct WaveStack {
    lines: VecDeque<(Vec<f32>, f32)>,
    acc: f32,
    ybuf: Vec<i32>,
}

impl WaveStack {
    pub fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            acc: 0.0,
            ybuf: Vec::new(),
        }
    }
}

fn snapshot(f: &Features, t: f32) -> Vec<f32> {
    let n = f.waveform.len();
    let gain = 0.35 + 0.65 * f.level;
    (0..RES)
        .map(|i| {
            let p = i as f32 / (RES - 1) as f32;
            // Averaging a window per sample keeps the trace readable.
            let v = if n == 0 {
                0.0
            } else {
                let (a, b) = (i * n / RES, ((i + 1) * n / RES).clamp(i * n / RES + 1, n));
                let s = &f.waveform[a.min(n - 1)..b];
                s.iter().sum::<f32>() / s.len().max(1) as f32
            };
            // A slow swell so silence still shows a moving surface.
            let idle = 0.12 * (p * 7.0 + t * 1.5).sin();
            (v * gain + idle) * (p * PI).sin()
        })
        .collect()
}

impl Effect for WaveStack {
    fn name(&self) -> &'static str {
        "Wave Stack"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.acc += dt;
        if self.lines.is_empty() || self.acc >= STEP {
            self.acc = (self.acc - STEP).clamp(0.0, STEP);
            self.lines.push_front((snapshot(f, t), f.pulse));
            self.lines.truncate(MAX_LINES + 1);
        }
        let (w, h) = (c.w, c.h);
        let bg = hsv(0.7, 0.7, 0.04);
        c.fill(bg);
        if w == 0 || h == 0 {
            return;
        }
        let (wf, hf) = (w as f32, h as f32);
        let n = ((hf / 3.0) as usize).clamp(1, MAX_LINES).min(self.lines.len());
        let frac = self.acc / STEP;
        let cx = wf * 0.5;
        let horizon = -hf * 0.15;
        let near = hf * 0.86;

        self.ybuf.clear();
        self.ybuf.resize(w, h as i32);
        // Front to back: nearer lines claim their columns first.
        for (i, (line, pulse)) in self.lines.iter().take(n).enumerate() {
            let d = ((i as f32 + frac) / n as f32).min(1.0);
            let z = 1.0 + d * 2.8;
            let base_y = horizon + (near - horizon) / z;
            let amp = hf * 0.2 / z;
            let half = wf * 0.48 / z.powf(0.6);
            let v = (1.0 - d * 0.8) * (0.75 + 0.25 * pulse);
            let fade = 1.0 - d.powi(3);
            let col = mix(bg, hsv(0.02 + d * 0.62, 0.8 - 0.4 * pulse * (1.0 - d), v), fade);
            let fill = scale(col, 0.18);
            let x0 = (cx - half).max(0.0) as usize;
            let x1 = ((cx + half).ceil().max(0.0) as usize).min(w);
            let mut prev: Option<i32> = None;
            for x in x0..x1 {
                let p = ((x as f32 - (cx - half)) / (2.0 * half).max(1.0)).clamp(0.0, 1.0);
                let s = p * (RES - 1) as f32;
                let (j0, fr) = ((s as usize).min(RES - 1), s.fract());
                let j1 = (j0 + 1).min(RES - 1);
                let val = line[j0] + (line[j1] - line[j0]) * fr;
                let y = (base_y - val * amp) as i32;
                let lim = self.ybuf[x];
                // Vertical span to the previous column keeps steep slopes closed.
                let (a, b) = match prev {
                    Some(py) => (y.min(py + 1), y.max(py - 1)),
                    None => (y, y),
                };
                prev = Some(y);
                if y < lim {
                    for yy in (y + 1).max(0)..lim {
                        c.set(x, yy as usize, fill);
                    }
                }
                for yy in a.max(0)..=b.min(lim - 1) {
                    c.set(x, yy as usize, col);
                }
                if y < lim {
                    self.ybuf[x] = y.max(0);
                }
            }
        }
    }
}
