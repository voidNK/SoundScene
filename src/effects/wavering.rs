use super::Effect;
use super::mirrorbars::sample;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, scale};
use std::f32::consts::TAU;

/// Maximum points per ring; small rings use fewer to avoid overdraw.
const POINTS: usize = 160;
const COPIES: usize = 3;

/// Waveform value at loop position `p` (0..1), crossfaded at the ends so a
/// closed curve has no seam.
fn loop_wave(wave: &[f32], p: f32) -> f32 {
    let q = 0.2 + p * 0.8;
    let v = sample(wave, q);
    if p > 0.75 {
        let s = (p - 0.75) / 0.25;
        let s = s * s * (3.0 - 2.0 * s);
        v * (1.0 - s) + sample(wave, q - 0.8) * s
    } else {
        v
    }
}

/// Line with a dim one-pixel halo around it.
pub(super) fn glow_line(c: &mut Canvas, a: (f32, f32), b: (f32, f32), col: Rgb) {
    let halo = scale(col, 0.3);
    for (ox, oy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        c.line_add(a.0 + ox, a.1 + oy, b.0 + ox, b.1 + oy, halo);
    }
    c.line_add(a.0, a.1, b.0, b.1, col);
}

/// The waveform wrapped around a circle, drawn as glowing nested echo rings
/// that spin in alternating directions and leave fading copies. Bass pumps
/// the radius, mids spin it, beats flash it.
pub struct WaveRing {
    spin: f32,
    hue: f32,
}

impl WaveRing {
    pub fn new() -> Self {
        Self { spin: 0.0, hue: 0.55 }
    }
}

impl Effect for WaveRing {
    fn name(&self) -> &'static str {
        "Wave Ring"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.72);
        self.spin += dt * (0.3 + f.mid * 1.5 + f.pulse * 2.0);
        self.hue += dt * 0.03;
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let m = c.w.min(c.h) as f32;
        let base = m * 0.22 * (1.0 + 0.3 * f.bass + 0.15 * f.pulse);
        let amp = m * 0.12 * (0.6 + 0.6 * f.level);
        // Gentle breathing so silence still shows a living ring.
        let breathe = 1.0 + 0.04 * (t * 1.3).sin();
        for k in 0..COPIES {
            let kf = k as f32;
            let r = base * breathe * (1.0 + kf * 0.32);
            let a_k = amp * (1.0 - 0.25 * kf);
            let dir = if k % 2 == 0 { 1.0 } else { -1.0 };
            let rot = self.spin * (1.0 - 0.3 * kf) * dir + kf * 0.7;
            let v = (1.0 - 0.28 * kf) * (0.55 + 0.25 * f.level + 0.2 * f.pulse);
            let col = hsv(self.hue + kf * 0.13 + f.treble * 0.1, 0.75 - 0.4 * f.pulse, v);
            let points = (r * 0.8).clamp(16.0, POINTS as f32) as usize;
            let point = |i: usize| {
                let p = i as f32 / points as f32;
                let a = p * TAU + rot;
                let rr = (r + loop_wave(&f.waveform, p) * a_k).max(0.0);
                (cx + a.cos() * rr, cy + a.sin() * rr)
            };
            let mut prev = point(0);
            for i in 1..=points {
                let cur = point(i % points);
                if m >= 40.0 {
                    glow_line(c, prev, cur, col);
                } else {
                    c.line_add(prev.0, prev.1, cur.0, cur.1, scale(col, 0.7));
                }
                prev = cur;
            }
        }
        // Hot core that swells on beats.
        let core = (m * 0.05 * (0.5 + f.bass + f.pulse)).max(0.5);
        let ci = core.ceil() as i32;
        for dy in -ci..=ci {
            for dx in -ci..=ci {
                let d = ((dx * dx + dy * dy) as f32).sqrt() / core;
                if d < 1.0 {
                    let col = hsv(self.hue + 0.5, 0.5, (1.0 - d) * 0.5);
                    c.plot_add(cx as i32 + dx, cy as i32 + dy, col);
                }
            }
        }
    }
}
