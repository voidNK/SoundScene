use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

/// Smallest generated radius relative to the outer circle.
const MIN_R: f32 = 0.004;
const MAX_ZOOM: f32 = 4.0;

/// A circle as (curvature, curvature·x, curvature·y). In this form the
/// Descartes theorem is linear: the second circle tangent to three others
/// is 2(a + b + c) − d.
type Circle = [f32; 3];

/// Apollonian gasket: an outer circle filled with ever smaller tangent
/// circles, built with the Descartes circle theorem. Rotates and zooms into
/// tangency points; each circle lights up with the spectrum band matching
/// its size (big ones bass, tiny ones treble), beats flash all rims.
pub struct Apollonian {
    /// Center x, y, radius and generation of every circle except the outer one.
    circles: Vec<(f32, f32, f32, u8)>,
    phase: f32,
    rot: f32,
    hue: f32,
    target: usize,
}

impl Apollonian {
    pub fn new() -> Self {
        let r0 = 2.0 * 3f32.sqrt() - 3.0;
        let outer: Circle = [-1.0, 0.0, 0.0];
        let inner: Vec<Circle> = (0..3)
            .map(|i| {
                let a = TAU * (0.25 + i as f32 / 3.0);
                let k = 1.0 / r0;
                [k, k * (1.0 - r0) * a.cos(), k * (1.0 - r0) * a.sin()]
            })
            .collect();
        let mut s = Self {
            circles: Vec::new(),
            phase: 0.0,
            rot: 0.0,
            hue: 0.0,
            target: 0,
        };
        for c in &inner {
            s.push(*c, 0);
        }
        let (a, b, c) = (inner[0], inner[1], inner[2]);
        let mut stack = vec![
            (a, b, c, outer, 1u8),
            (outer, a, b, c, 1),
            (outer, a, c, b, 1),
            (outer, b, c, a, 1),
        ];
        while let Some((a, b, c, d, gen_)) = stack.pop() {
            let e: Circle = std::array::from_fn(|i| 2.0 * (a[i] + b[i] + c[i]) - d[i]);
            if e[0] <= 0.0 || 1.0 / e[0] < MIN_R {
                continue;
            }
            s.push(e, gen_);
            let g = gen_.saturating_add(1);
            stack.push((a, b, e, c, g));
            stack.push((a, c, e, b, g));
            stack.push((b, c, e, a, g));
        }
        // Largest first, so the band of a circle follows its draw order.
        s.circles.sort_by(|p, q| q.2.total_cmp(&p.2));
        s
    }

    fn push(&mut self, c: Circle, gen_: u8) {
        self.circles.push((c[1] / c[0], c[2] / c[0], 1.0 / c[0], gen_));
    }
}

impl Effect for Apollonian {
    fn name(&self) -> &'static str {
        "Apollonian"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.phase += dt / 18.0 * (0.7 + 0.8 * f.bass);
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.target = (self.target + 1) % 3;
        }
        self.rot += dt * (0.08 + 0.15 * f.mid);
        self.hue += dt * 0.03;
        c.fill([3, 2, 8]);
        if c.w == 0 || c.h == 0 {
            return;
        }

        // Zoom into the point where one of the big circles touches the rim.
        let zoom = MAX_ZOOM.powf(0.5 - 0.5 * (self.phase * TAU).cos());
        let a = TAU * (0.25 + self.target as f32 / 3.0);
        let (tx, ty) = (a.cos(), a.sin());
        let (wx, wy) = (tx - tx / zoom, ty - ty / zoom);
        let scale = c.w.min(c.h) as f32 * 0.48 * zoom * (1.0 + 0.04 * f.pulse);
        let (s, co) = self.rot.sin_cos();
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let bins = f.spectrum.len();
        let (w, h) = (c.w as f32, c.h as f32);

        // Outer rim.
        let rim = hsv(self.hue, 0.5, 0.5 + 0.5 * f.pulse);
        let steps = (scale * TAU).clamp(8.0, 4000.0) as usize;
        for i in 0..steps {
            let p = TAU * i as f32 / steps as f32;
            let (x, y) = (p.cos() - wx, p.sin() - wy);
            c.plot_add(
                (cx + (x * co - y * s) * scale) as i32,
                (cy + (x * s + y * co) * scale) as i32,
                rim,
            );
        }

        for &(x, y, r, gen_) in &self.circles {
            let rs = r * scale;
            if rs < 0.6 {
                continue;
            }
            let (x, y) = (x - wx, y - wy);
            let px = cx + (x * co - y * s) * scale;
            let py = cy + (x * s + y * co) * scale;
            if px + rs < 0.0 || py + rs < 0.0 || px - rs > w || py - rs > h {
                continue;
            }
            // Bigger circles listen to lower bands.
            let band = ((-r.log2() - 1.0) / 6.0 * bins as f32) as usize;
            let e = if bins > 0 { f.spectrum[band.min(bins - 1)] } else { 0.0 };
            let v = 0.3 + 0.7 * e;
            let hue = self.hue + gen_ as f32 * 0.11 + band as f32 * 0.012;
            let y0 = (py - rs).floor().max(0.0) as usize;
            let y1 = ((py + rs).ceil().max(0.0) as usize).min(c.h);
            let inv = 1.0 / rs;
            for yy in y0..y1 {
                let dy = yy as f32 + 0.5 - py;
                let half = rs * rs - dy * dy;
                if half <= 0.0 {
                    continue;
                }
                let half = half.sqrt();
                let x0 = (px - half).round().max(0.0) as usize;
                let x1 = ((px + half).round().max(0.0) as usize).min(c.w);
                for xx in x0..x1 {
                    let dx = xx as f32 + 0.5 - px;
                    let d = ((dx * dx + dy * dy).sqrt() * inv).min(1.0);
                    // Dark core, bright rim that flashes on beats.
                    let k = d * d * d * d;
                    let col = hsv(hue, 0.85 - 0.5 * k * f.pulse, v * (0.18 + 0.82 * k) + 0.3 * k * f.pulse);
                    c.set(xx, yy, col);
                }
            }
        }
    }
}
