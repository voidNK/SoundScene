use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, rgb};
use std::f64::consts::PI;

/// Points on the boundary that stay detailed at every depth (Misiurewicz
/// points and the seahorse valley).
const TARGETS: [(f64, f64); 4] = [
    (-0.743_643_887_037_151, 0.131_825_904_205_330),
    (-0.101_096_363_845_622_1, 0.956_286_510_809_141_5),
    (-0.775_683_77, 0.136_467_37),
    (-1.543_689_012_692_076, 0.0),
];
/// Deepest zoom as a natural log, e^11 ≈ 60000×.
const DEPTH: f64 = 11.0;
/// Part of each cycle spent panning to the next target while zoomed out.
const PAN: f64 = 0.12;
const PALETTE: usize = 512;

/// Endless Mandelbrot zoom: dives into a boundary point, zooms back out and
/// pans to the next one. Bass speeds up the dive, treble and beats rotate the
/// palette, the pulse brightens it.
pub struct MandelbrotZoom {
    phase: f64,
    target: usize,
    from: (f64, f64),
    rot: f64,
    hue: f32,
    /// Smooth iteration count per pixel, negative inside the set.
    vals: Vec<f32>,
    prev: Vec<f32>,
    /// View of the previous frame: origin, pixel size, sin and cos of the rotation.
    view: Option<(f64, f64, f64, f64, f64)>,
    frame: usize,
    size: (usize, usize),
    palette: Vec<Rgb>,
}

impl MandelbrotZoom {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            target: 0,
            from: TARGETS[TARGETS.len() - 1],
            rot: 0.0,
            hue: 0.0,
            vals: Vec::new(),
            prev: Vec::new(),
            view: None,
            frame: 0,
            size: (0, 0),
            palette: vec![[0; 3]; PALETTE],
        }
    }
}

fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Effect for MandelbrotZoom {
    fn name(&self) -> &'static str {
        "Mandelbrot Zoom"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.vals = vec![-1.0; c.w * c.h];
            self.prev = vec![-1.0; c.w * c.h];
            self.view = None;
        }
        self.phase += dt as f64 / 34.0 * (0.7 + 0.9 * f.bass as f64);
        if self.phase >= 1.0 {
            self.phase = self.phase.fract();
            self.from = TARGETS[self.target];
            self.target = (self.target + 1) % TARGETS.len();
        }
        self.rot += dt as f64 * (0.04 + 0.1 * f.mid as f64);
        self.hue += dt * (0.02 + 0.12 * f.treble) + if f.beat { 0.06 } else { 0.0 };

        let to = TARGETS[self.target];
        let s = smoothstep(self.phase / PAN);
        let (ox, oy) = (
            self.from.0 + (to.0 - self.from.0) * s,
            self.from.1 + (to.1 - self.from.1) * s,
        );
        let depth = if self.phase < PAN {
            0.0
        } else {
            DEPTH * (PI * (self.phase - PAN) / (1.0 - PAN)).sin().powi(2)
        };
        let per = 2.6 * (-depth).exp() / c.w.min(c.h).max(1) as f64;
        let (sn, cs) = self.rot.sin_cos();
        let max_iter = (64.0 + depth * 14.0) as u32;
        let (cx, cy) = (c.w as f64 * 0.5, c.h as f64 * 0.5);

        // Only every other row is iterated per frame. The rows in between are
        // reprojected from the previous frame, which hides the interlacing even
        // while the view zooms and turns.
        std::mem::swap(&mut self.vals, &mut self.prev);
        let old = self.view;
        self.frame += 1;
        let parity = ((self.frame - 1) % 2) as f64;
        for y in 0..c.h {
            let fresh = old.is_none() || (y + self.frame).is_multiple_of(2);
            let dy = (y as f64 - cy) * per;
            for x in 0..c.w {
                let dx = (x as f64 - cx) * per;
                let (pr, pi) = (ox + dx * cs - dy * sn, oy + dx * sn + dy * cs);
                let reused = match old {
                    Some((qx, qy, qper, qs, qc)) if !fresh => {
                        let (ax, ay) = (pr - qx, pi - qy);
                        let u = ((ax * qc + ay * qs) / qper + cx).round();
                        // Snap to the nearest row that was iterated last frame, so values are
                        // never more than one frame old.
                        let v = ((((ay * qc - ax * qs) / qper + cy) - parity) * 0.5).round() * 2.0 + parity;
                        if u >= 0.0 && v >= 0.0 && (u as usize) < c.w && (v as usize) < c.h {
                            Some(self.prev[v as usize * c.w + u as usize])
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                self.vals[y * c.w + x] = reused.unwrap_or_else(|| escape(pr, pi, max_iter));
            }
        }
        self.view = Some((ox, oy, per, sn, cs));

        for (i, p) in self.palette.iter_mut().enumerate() {
            let t = (i as f32 / PALETTE as f32 + self.hue) * std::f32::consts::TAU;
            *p = rgb(
                0.5 + 0.5 * t.cos(),
                0.5 + 0.5 * (t - 0.9 - f.bass * 0.6).cos(),
                0.5 + 0.5 * (t - 1.9).cos(),
            );
        }
        let flash = 0.75 + 0.25 * f.pulse;
        let inside = rgb(0.05 * f.pulse, 0.02 + 0.05 * f.pulse, 0.06 + 0.1 * f.pulse);
        for (px, &v) in c.px.iter_mut().zip(&self.vals) {
            *px = if v < 0.0 {
                inside
            } else {
                let t = v.sqrt() * 0.35;
                let idx = ((t.fract() * PALETTE as f32) as usize).min(PALETTE - 1);
                let k = (v / 10.0).min(1.0) * flash;
                let p = self.palette[idx];
                [
                    (p[0] as f32 * k) as u8,
                    (p[1] as f32 * k) as u8,
                    (p[2] as f32 * k) as u8,
                ]
            };
        }
    }
}

/// Smooth escape count, or -1 inside the set.
fn escape(cr: f64, ci: f64, max_iter: u32) -> f32 {
    // Main cardioid and period-2 bulb are inside for sure.
    let q = (cr - 0.25) * (cr - 0.25) + ci * ci;
    if q * (q + (cr - 0.25)) <= 0.25 * ci * ci || (cr + 1.0) * (cr + 1.0) + ci * ci <= 0.0625 {
        return -1.0;
    }
    let (mut zr, mut zi) = (0.0f64, 0.0f64);
    // Periodicity check: an orbit that returns to a saved point is a cycle,
    // so the point is inside. Saves most of the work on interior pixels.
    let (mut sr, mut si, mut next_save) = (0.0f64, 0.0f64, 8);
    let mut n = 0;
    while n < max_iter {
        let (zr2, zi2) = (zr * zr, zi * zi);
        if zr2 + zi2 > 256.0 {
            let mu = n as f64 + 1.0 - (0.5 * (zr2 + zi2).ln()).ln() / std::f64::consts::LN_2;
            return mu.max(0.0) as f32;
        }
        zi = 2.0 * zr * zi + ci;
        zr = zr2 - zi2 + cr;
        n += 1;
        if (zr - sr).abs() < 1e-13 && (zi - si).abs() < 1e-13 {
            return -1.0;
        }
        if n == next_save {
            (sr, si) = (zr, zi);
            next_save *= 2;
        }
    }
    -1.0
}
