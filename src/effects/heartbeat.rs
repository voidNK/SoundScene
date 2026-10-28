use super::{Effect, digits};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, mix, scale};
use std::collections::VecDeque;
use std::f32::consts::PI;

/// Seconds the trace needs to cross the screen.
const SWEEP: f32 = 3.5;
/// No beat for this long and the monitor shows a flatline alarm.
const FLATLINE: f32 = 3.0;
const HEART: [u8; 5] = [0b01010, 0b11111, 0b11111, 0b01110, 0b00100];

/// One PQRST complex, `tau` seconds after the beat; `interval` places the
/// P wave just before the expected next beat.
fn ecg(tau: f32, interval: f32) -> f32 {
    let bump = |a: f32, len: f32, amp: f32| {
        if tau >= a && tau < a + len {
            amp * (PI * (tau - a) / len).sin()
        } else {
            0.0
        }
    };
    let r = if (0.025..0.065).contains(&tau) {
        1.0 - ((tau - 0.045) / 0.02).abs()
    } else {
        0.0
    };
    let p = if interval > 0.4 {
        bump(interval - 0.18, 0.1, 0.12)
    } else {
        0.0
    };
    bump(0.0, 0.025, -0.12) + r + bump(0.065, 0.035, -0.3) + bump(0.22, 0.18, 0.28) + p
}

/// Pulse oximeter wave: a smooth rise peaking 0.2 s after the beat.
fn pleth(tau: f32) -> f32 {
    let x = tau / 0.2;
    x * x * (2.0 * (1.0 - x)).exp() + 0.15 * (-(tau - 0.45).powi(2) * 200.0).exp()
}

/// A patient monitor: an ECG trace scrolls across a green grid with phosphor
/// glow and fires a QRS spike on every detected beat, a pulse wave below it
/// swells with the loudness, and the BPM readout is measured from the beat
/// intervals. Silence ends in a red flatline.
pub struct Heartbeat {
    ecg: VecDeque<f32>,
    pleth: VecDeque<f32>,
    width: usize,
    since: f32,
    intervals: VecDeque<f32>,
    bpm: u32,
    amp: f32,
    acc: f32,
    clock: f32,
    rng: Rng,
}

impl Heartbeat {
    pub fn new() -> Self {
        Self {
            ecg: VecDeque::new(),
            pleth: VecDeque::new(),
            width: 0,
            since: 10.0,
            intervals: VecDeque::new(),
            bpm: 0,
            amp: 1.0,
            acc: 0.0,
            clock: 0.0,
            rng: Rng::new(0xEC6),
        }
    }

    fn interval(&self) -> f32 {
        if self.bpm == 0 { 0.0 } else { 60.0 / self.bpm as f32 }
    }

    fn on_beat(&mut self, bass: f32) {
        if (0.25..2.0).contains(&self.since) {
            self.intervals.push_back(self.since);
            if self.intervals.len() > 8 {
                self.intervals.pop_front();
            }
            let mut sorted: Vec<f32> = self.intervals.iter().copied().collect();
            sorted.sort_by(f32::total_cmp);
            let median = sorted[sorted.len() / 2];
            self.bpm = (60.0 / median).round() as u32;
        }
        self.since = 0.0;
        self.amp = 0.6 + 0.4 * bass;
    }
}

/// Draws a polyline from per-column samples, older columns dimmer, with a
/// soft glow above and below.
fn trace(c: &mut Canvas, samples: &VecDeque<f32>, base: f32, gain: f32, col: Rgb) {
    let w = samples.len();
    let y = |v: f32| base - v * gain;
    for x in 1..w {
        let age = x as f32 / w as f32;
        let k = 0.15 + 0.85 * age * age;
        let (y0, y1) = (y(samples[x - 1]), y(samples[x]));
        let (x0, x1) = ((x - 1) as f32, x as f32);
        c.line_add(x0, y0 - 1.0, x1, y1 - 1.0, scale(col, k * 0.18));
        c.line_add(x0, y0 + 1.0, x1, y1 + 1.0, scale(col, k * 0.18));
        c.line_add(x0, y0, x1, y1, scale(col, k * 0.8));
    }
    if let Some(&v) = samples.back() {
        let (hx, hy) = ((w - 1) as i32, y(v).round() as i32);
        for dy in -2..=2i32 {
            for dx in -2..=2i32 {
                let d = (dx * dx + dy * dy) as f32;
                c.plot_add(hx + dx, hy + dy, scale(mix(col, [255, 255, 255], 0.6), 1.0 / (1.0 + d)));
            }
        }
    }
}

impl Effect for Heartbeat {
    fn name(&self) -> &'static str {
        "Heartbeat"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        let (w, h) = (c.w, c.h);
        if self.width != w {
            self.width = w;
            self.ecg = VecDeque::from(vec![0.0; w]);
            self.pleth = VecDeque::from(vec![0.0; w]);
            self.acc = 0.0;
        }
        self.clock += dt;
        self.since += dt;
        if f.beat {
            self.on_beat(f.bass);
        }
        let flat = self.since > FLATLINE;
        if flat {
            self.bpm = 0;
            self.intervals.clear();
        }

        // New columns for the time that passed, each sampled at its own moment.
        let speed = w as f32 / SWEEP;
        self.acc = (self.acc + dt * speed).min(w as f32);
        let n = self.acc as usize;
        self.acc -= n as f32;
        let interval = self.interval();
        for k in 0..n {
            let tau = self.since - (n - 1 - k) as f32 / speed.max(1e-3);
            let noise = self.rng.range(-1.0, 1.0) * (0.01 + 0.03 * f.treble);
            let wander = 0.05 * f.mid * (self.clock * 1.3).sin();
            let v = if tau < 0.0 { 0.0 } else { ecg(tau, interval) * self.amp };
            self.ecg.push_back(v + noise + wander);
            let p = if tau < 0.0 { 0.0 } else { pleth(tau) };
            self.pleth.push_back(p * (0.35 + 0.65 * f.level));
            if self.ecg.len() > w {
                self.ecg.pop_front();
                self.pleth.pop_front();
            }
        }

        // Grid.
        let minor = (h / 24).max(3);
        for y in 0..h {
            for x in 0..w {
                let (mx, my) = (x % minor == 0, y % minor == 0);
                let major = (x % (minor * 5) == 0) || (y % (minor * 5) == 0);
                let col = if major && (mx || my) {
                    [0, 46, 24]
                } else if mx || my {
                    [0, 22, 12]
                } else {
                    [0, 7, 4]
                };
                c.set(x, y, col);
            }
        }
        if w < 4 || h < 4 {
            return;
        }

        let hf = h as f32;
        let alarm = flat && (self.clock * 2.0).fract() < 0.5;
        let green = if flat { [255, 60, 50] } else { [70, 255, 120] };
        let split = h >= 30;
        let ecg_h = if split { hf * 0.62 } else { hf };
        trace(c, &self.ecg, ecg_h * 0.62, ecg_h * 0.5, green);
        if split {
            let base = hf * 0.95;
            trace(c, &self.pleth, base, hf * 0.27, [60, 200, 255]);
        }

        let ds = (h / 26).max(1) as i32;
        if w >= 40 {
            let x = w as i32 - digits::width(self.bpm, ds) - 2 * ds;
            let col = if alarm { [255, 255, 255] } else { green };
            digits::draw(c, x, 2 * ds, self.bpm, ds, col);
            let heart = scale([255, 60, 80], 0.35 + 0.65 * f.pulse);
            let hx = x - 7 * ds;
            for (row, bits) in HEART.iter().enumerate() {
                for bit in 0..5 {
                    if bits & (16 >> bit) != 0 {
                        for dy in 0..ds {
                            for dx in 0..ds {
                                c.plot(hx + bit * ds + dx, 2 * ds + row as i32 * ds + dy, heart);
                            }
                        }
                    }
                }
            }
        }
    }
}
