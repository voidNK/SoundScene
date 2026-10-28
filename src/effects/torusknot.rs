use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};
use std::f32::consts::TAU;

const SAMPLES: usize = 720;
const RING: usize = 10;
const KNOTS: [(f32, f32); 7] = [
    (2.0, 3.0),
    (3.0, 2.0),
    (2.0, 5.0),
    (3.0, 4.0),
    (5.0, 2.0),
    (3.0, 5.0),
    (4.0, 3.0),
];
/// Beats between two knot changes.
const BEATS_PER_KNOT: u32 = 8;

type V3 = [f32; 3];

fn knot((p, q): (f32, f32), phi: f32) -> V3 {
    let r = (q * phi).cos() + 2.0;
    [
        r * (p * phi).cos() / 3.0,
        r * (p * phi).sin() / 3.0,
        -(q * phi).sin() / 3.0,
    ]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(a: V3) -> Option<V3> {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (l > 1e-6).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// A (p, q) torus knot drawn as a shaded tube of glowing dots. The tube
/// swells with the spectrum along its length and with every beat; every few
/// beats it morphs into a different knot.
pub struct TorusKnot {
    rx: f32,
    ry: f32,
    hue: f32,
    spin: f32,
    from: usize,
    to: usize,
    morph: f32,
    beats: u32,
    path: Vec<V3>,
}

impl TorusKnot {
    pub fn new() -> Self {
        Self {
            rx: 0.6,
            ry: 0.0,
            hue: 0.0,
            spin: 0.0,
            from: 0,
            to: 0,
            morph: 1.0,
            beats: 0,
            path: Vec::with_capacity(SAMPLES + 1),
        }
    }
}

impl Effect for TorusKnot {
    fn name(&self) -> &'static str {
        "Torus Knot"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.45);
        self.rx += dt * (0.2 + 0.6 * f.mid);
        self.ry += dt * (0.35 + 1.2 * f.bass);
        self.spin += dt * (1.0 + 3.0 * f.level);
        self.hue += dt * 0.03 + if f.beat { 0.04 } else { 0.0 };
        self.morph = (self.morph + dt * 0.7).min(1.0);
        if f.beat {
            self.beats += 1;
            if self.beats >= BEATS_PER_KNOT && self.morph >= 1.0 {
                self.beats = 0;
                self.from = self.to;
                self.to = (self.to + 1) % KNOTS.len();
                self.morph = 0.0;
            }
        }
        let m = self.morph * self.morph * (3.0 - 2.0 * self.morph);
        let (ka, kb) = (KNOTS[self.from], KNOTS[self.to]);

        self.path.clear();
        for i in 0..=SAMPLES {
            let phi = i as f32 / SAMPLES as f32 * TAU;
            let a = knot(ka, phi);
            let b = knot(kb, phi);
            self.path.push([
                a[0] + (b[0] - a[0]) * m,
                a[1] + (b[1] - a[1]) * m,
                a[2] + (b[2] - a[2]) * m,
            ]);
        }

        let (sx, cx_) = self.rx.sin_cos();
        let (sy, cy_) = self.ry.sin_cos();
        let rot = |v: V3| -> V3 {
            let (y, z) = (v[1] * cx_ - v[2] * sx, v[1] * sx + v[2] * cx_);
            let (x, z) = (v[0] * cy_ + z * sy, -v[0] * sy + z * cy_);
            [x, y, z]
        };
        let light = [-0.4f32, -0.6, -0.7];
        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let size = w.min(h);
        let bins = f.spectrum.len();

        for i in 0..SAMPLES {
            let s = i as f32 / SAMPLES as f32;
            let p = self.path[i];
            let Some(tan) = norm(sub(self.path[i + 1], p)) else {
                continue;
            };
            let bi = norm(cross(tan, p)).unwrap_or([0.0, 0.0, 1.0]);
            let nn = cross(bi, tan);
            // Mirror the spectrum so the knot closes without a seam.
            let mirrored = 1.0 - (2.0 * s - 1.0).abs();
            let level = if bins > 0 {
                f.spectrum[((mirrored * (bins - 1) as f32) as usize).min(bins - 1)]
            } else {
                0.0
            };
            let rad = 0.035 + 0.04 * f.pulse + 0.07 * level;
            let hue = self.hue + s * 2.0 + 0.1 * (t * 0.5).sin();
            for k in 0..RING {
                let a = k as f32 / RING as f32 * TAU + self.spin + s * 40.0;
                let (sa, ca) = a.sin_cos();
                let n = [
                    nn[0] * ca + bi[0] * sa,
                    nn[1] * ca + bi[1] * sa,
                    nn[2] * ca + bi[2] * sa,
                ];
                let q = rot([p[0] + n[0] * rad, p[1] + n[1] * rad, p[2] + n[2] * rad]);
                let nr = rot(n);
                // Light comes from the viewer side; back-facing dots stay dim.
                let diff = (nr[0] * light[0] + nr[1] * light[1] + nr[2] * light[2]).max(0.0);
                let facing = (-nr[2]).max(0.0);
                let d = 3.0 / (q[2] + 3.0).max(0.5);
                let bri = (0.06 + 0.4 * diff * diff + 0.1 * facing) * (0.6 + 0.4 * d) * (0.8 + 0.6 * level);
                c.plot_add(
                    (cx + q[0] * size * d * 0.5).round() as i32,
                    (cy + q[1] * size * d * 0.5).round() as i32,
                    hsv(hue, 0.65 - 0.3 * diff, bri),
                );
            }
        }
    }
}
