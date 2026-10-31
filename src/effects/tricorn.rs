use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng};
use std::f32::consts::TAU;

const CELL: usize = 1;
const MAX_ITER: u32 = 90;
const OVERVIEW: (f32, f32) = (-0.25, 0.0);
const MAX_ZOOM: f32 = 40.0;

/// Tricorn (Mandelbar, z ← conj(z)² + c) with smooth coloring. Slowly
/// zooms into a random boundary point and back out while rotating; bass
/// speeds up the dive, beats shift the palette, the pulse brightens it.
pub struct Tricorn {
    colors: Vec<[f32; 3]>,
    phase: f32,
    rot: f32,
    shift: f32,
    shift_to: f32,
    target: (f32, f32),
    rng: Rng,
}

impl Tricorn {
    pub fn new() -> Self {
        let mut s = Self {
            colors: Vec::new(),
            phase: 0.0,
            rot: 0.0,
            shift: 0.0,
            shift_to: 0.0,
            target: OVERVIEW,
            rng: Rng::new(0x7C0B),
        };
        s.pick_target();
        s
    }

    /// Searches for a point that escapes late, i.e. sits on the boundary.
    fn pick_target(&mut self) {
        for _ in 0..400 {
            let (cr, ci) = (self.rng.range(-1.8, 1.0), self.rng.range(-1.2, 1.2));
            let n = escape(cr, ci, 200).0;
            if (40..200).contains(&n) {
                self.target = (cr, ci);
                return;
            }
        }
    }
}

/// Iterations until escape and the final |z|², capped at `max`.
fn escape(cr: f32, ci: f32, max: u32) -> (u32, f32) {
    let (mut zr, mut zi) = (0.0f32, 0.0f32);
    let mut n = 0;
    while n < max {
        let m = zr * zr + zi * zi;
        if m > 64.0 {
            return (n, m);
        }
        (zr, zi) = (zr * zr - zi * zi + cr, -2.0 * zr * zi + ci);
        n += 1;
    }
    (max, zr * zr + zi * zi)
}

/// Cosine palette, `t` wraps every 1.0.
fn palette(t: f32) -> [f32; 3] {
    [
        0.5 + 0.5 * (TAU * (t + 0.0)).cos(),
        0.5 + 0.5 * (TAU * (t + 0.15)).cos(),
        0.5 + 0.5 * (TAU * (t + 0.35)).cos(),
    ]
}

impl Effect for Tricorn {
    fn name(&self) -> &'static str {
        "Tricorn"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.phase += dt / 26.0 * (0.7 + 0.8 * f.bass);
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.pick_target();
        }
        self.rot += dt * (0.05 + 0.08 * f.mid);
        if f.beat {
            self.shift_to += 0.17;
        }
        self.shift += (self.shift_to - self.shift) * (1.0 - (-3.0 * dt).exp()) + dt * 0.02;
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        if gw == 0 || gh == 0 {
            return;
        }
        self.colors.resize(gw * gh, [0.0; 3]);

        let zoom = MAX_ZOOM.powf(0.5 - 0.5 * (self.phase * TAU).cos());
        let cx = self.target.0 + (OVERVIEW.0 - self.target.0) / zoom;
        let cy = self.target.1 + (OVERVIEW.1 - self.target.1) / zoom;
        let step = 3.6 / zoom / gw.min(gh).max(1) as f32;
        let (s, co) = self.rot.sin_cos();
        let bright = 0.85 + 0.4 * f.pulse;
        let max = (40.0 + 12.0 * zoom.ln()) as u32;
        let max = max.min(MAX_ITER);

        for gy in 0..gh {
            let dy = (gy as f32 - gh as f32 * 0.5) * step;
            for gx in 0..gw {
                let dx = (gx as f32 - gw as f32 * 0.5) * step;
                let (cr, ci) = (cx + dx * co - dy * s, cy + dx * s + dy * co);
                let (n, m) = escape(cr, ci, max);
                self.colors[gy * gw + gx] = if n >= max {
                    [0.02, 0.0, 0.05 + 0.1 * f.pulse]
                } else {
                    let sm = n as f32 + 1.0 - (0.5 * m.ln()).max(1e-6).ln() / 2f32.ln();
                    let p = palette(self.shift + sm.max(0.0).sqrt() * 0.22);
                    let v = (sm / 10.0).min(1.0).sqrt() * bright;
                    p.map(|x| x * v)
                };
            }
        }
        upscale(c, &self.colors, gw, gh);
    }
}
