use super::Effect;
use super::mirrorbars::sample;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, scale};
use std::collections::VecDeque;
use std::f32::consts::TAU;

/// Spectrum samples stored per ring.
const SHAPE: usize = 32;
/// Maximum points per drawn ring; small rings use fewer.
const SEGMENTS: usize = 72;
const Z_FAR: f32 = 6.0;
const Z_NEAR: f32 = 0.2;
/// Depth between two rings.
const GAP: f32 = 0.18;

struct Ring {
    z: f32,
    shape: Vec<f32>,
    hue: f32,
    punch: f32,
}

/// A curving tunnel of rings flying at the viewer; each ring is shaped by
/// the (mirrored) spectrum at the moment it was emitted. Bass and beats
/// speed up the flight, beats emit brighter rings.
pub struct SpectrumTunnel {
    rings: VecDeque<Ring>,
    travel: f32,
    hue: f32,
}

impl SpectrumTunnel {
    pub fn new() -> Self {
        let rings = (0..((Z_FAR - Z_NEAR) / GAP) as usize)
            .map(|i| Ring {
                z: Z_FAR - i as f32 * GAP,
                shape: vec![0.0; SHAPE],
                hue: i as f32 * 0.01,
                punch: 0.0,
            })
            .rev()
            .collect();
        Self {
            rings,
            travel: 0.0,
            hue: 0.0,
        }
    }
}

impl Effect for SpectrumTunnel {
    fn name(&self) -> &'static str {
        "Spectrum Tunnel"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.4);
        let speed = 0.8 + f.bass * 2.0 + f.pulse * 2.5;
        let step = speed * dt;
        self.hue += dt * 0.05;
        for r in &mut self.rings {
            r.z -= step;
        }
        while self.rings.front().is_some_and(|r| r.z < Z_NEAR) {
            self.rings.pop_front();
        }
        self.travel += step;
        let mut emitted = 0;
        while self.travel >= GAP && emitted < 64 {
            self.travel -= GAP;
            emitted += 1;
            let shape = (0..SHAPE)
                .map(|k| sample(&f.spectrum, k as f32 / (SHAPE - 1) as f32 * 0.9))
                .collect();
            self.rings.push_back(Ring {
                z: Z_FAR - self.travel,
                shape,
                hue: self.hue,
                punch: f.pulse,
            });
        }
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (wf, hf) = (c.w as f32, c.h as f32);
        let m = wf.min(hf);
        let limit = wf.max(hf) * 2.0;
        for ring in self.rings.iter().rev() {
            let z = ring.z.max(Z_NEAR);
            let base = m * 0.3 / z;
            if base > limit {
                continue;
            }
            // The path bends more the further away a ring is.
            let bend = (z - Z_NEAR) / Z_FAR;
            let cx = wf * 0.5 + (t * 0.6 + z * 0.35).sin() * wf * 0.22 * bend;
            let cy = hf * 0.5 + (t * 0.45 + z * 0.3).cos() * hf * 0.22 * bend;
            let fog = (1.0 - z / Z_FAR).clamp(0.0, 1.0).powf(1.2);
            // Tiny far rings pile up in the center; dim them by size.
            let size_k = (base / 10.0).min(1.0);
            let col = scale(
                hsv(ring.hue + z * 0.04, 0.75 - 0.4 * ring.punch, 1.0),
                fog * size_k * (0.55 + 0.45 * ring.punch),
            );
            let twist = t * 0.2 + z * 0.15;
            let segments = (base * 0.8).clamp(10.0, SEGMENTS as f32) as usize;
            let point = |s: usize| {
                let u = s as f32 / segments as f32;
                // Mirrored around the ring: bass at the top, treble at the bottom.
                let fold = 1.0 - (u * 2.0 - 1.0).abs();
                let v = sample(&ring.shape, fold);
                let r = base * (1.0 + 0.7 * v);
                let a = u * TAU + twist - TAU * 0.25;
                (cx + a.cos() * r, cy + a.sin() * r)
            };
            let mut prev = point(0);
            for s in 1..=segments {
                let cur = point(s % segments);
                c.line_add(prev.0, prev.1, cur.0, cur.1, col);
                prev = cur;
            }
        }
    }
}
