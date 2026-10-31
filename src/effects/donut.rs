use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};
use std::f32::consts::TAU;

type V3 = [f32; 3];

const CAM: f32 = 4.5;
/// Checker segments around the ring and around the tube.
const SEGS: (usize, usize) = (24, 10);

/// Per-pixel depth buffer for solid surfaces drawn as dense point clouds.
pub(super) struct ZBuf {
    z: Vec<f32>,
    w: usize,
    h: usize,
}

impl ZBuf {
    pub(super) fn new() -> Self {
        Self {
            z: Vec::new(),
            w: 0,
            h: 0,
        }
    }

    /// Clears every pixel to "infinitely far" at the given canvas size.
    pub(super) fn reset(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.z.clear();
        self.z.resize(w * h, f32::INFINITY);
    }

    /// Depth test at screen position (x, y). Returns the pixel index if `z`
    /// is the nearest so far (and stores it), `None` if hidden or off-screen.
    #[inline]
    pub(super) fn test(&mut self, x: f32, y: f32, z: f32) -> Option<usize> {
        if !(x >= 0.0 && y >= 0.0) {
            return None;
        }
        let (xi, yi) = (x as usize, y as usize);
        if xi >= self.w || yi >= self.h {
            return None;
        }
        let i = yi * self.w + xi;
        if z < self.z[i] {
            self.z[i] = z;
            Some(i)
        } else {
            None
        }
    }
}

/// Rotation matrix Rz(b) · Rx(a).
pub(super) fn rotation(a: f32, b: f32) -> [V3; 3] {
    let (sa, ca) = a.sin_cos();
    let (sb, cb) = b.sin_cos();
    [[cb, -sb * ca, sb * sa], [sb, cb * ca, -cb * sa], [0.0, sa, ca]]
}

#[inline]
pub(super) fn apply(m: &[V3; 3], v: V3) -> V3 {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub(super) fn norm(v: V3) -> V3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-6 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0.0, 0.0, -1.0]
    }
}

#[inline]
pub(super) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Spectrum value at position `s` (0..1), mirrored so a closed loop has no seam.
pub(super) fn band_mirrored(spectrum: &[f32], s: f32) -> f32 {
    let n = spectrum.len();
    if n == 0 {
        return 0.0;
    }
    let m = 1.0 - (2.0 * s.rem_euclid(1.0) - 1.0).abs();
    spectrum[((m * (n - 1) as f32) as usize).min(n - 1)]
}

/// Dark vertical gradient used as backdrop by the solid 3D effects.
pub(super) fn backdrop(c: &mut Canvas, top: V3, bottom: V3) {
    let (w, h) = (c.w, c.h);
    for y in 0..h {
        let k = y as f32 / h.max(1) as f32;
        let col = rgb(
            top[0] + (bottom[0] - top[0]) * k,
            top[1] + (bottom[1] - top[1]) * k,
            top[2] + (bottom[2] - top[2]) * k,
        );
        c.px[y * w..(y + 1) * w].fill(col);
    }
}

/// The classic spinning donut, but solid: a z-buffered checkered torus with
/// diffuse and specular lighting. Bass inflates the ring and spins it, the
/// spectrum lights up its checker segments.
pub struct Donut {
    a: f32,
    b: f32,
    hue: f32,
    major: f32,
    ring: Vec<(f32, f32)>,
    zb: ZBuf,
}

impl Donut {
    pub fn new() -> Self {
        Self {
            a: 0.8,
            b: 0.0,
            hue: 0.05,
            major: 1.0,
            ring: Vec::new(),
            zb: ZBuf::new(),
        }
    }
}

impl Effect for Donut {
    fn name(&self) -> &'static str {
        "Donut"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.a += dt * (0.5 + 1.0 * f.mid);
        self.b += dt * (0.3 + 1.3 * f.bass);
        self.hue += dt * 0.02 + if f.beat { 0.06 } else { 0.0 };
        let target = 1.0 + 0.35 * f.bass + 0.15 * f.pulse;
        self.major += (target - self.major) * (dt * 10.0).min(1.0);

        backdrop(c, [0.02, 0.01, 0.06], [0.06, 0.03, 0.10]);
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        self.zb.reset(w, h);

        let big_r = self.major;
        let r = 0.42 + 0.06 * f.pulse;
        let focal = w.min(h) as f32 * 1.1;
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        // Pixels per world unit at the nearest point decide the sample density.
        let ppu = focal / (CAM - big_r - r).max(0.5);
        let n_phi = (TAU * (big_r + r) * ppu * 1.4).clamp(12.0, 2000.0) as usize;
        let n_th = (TAU * r * ppu * 1.6).clamp(8.0, 600.0) as usize;
        self.ring.clear();
        self.ring
            .extend((0..n_th).map(|j| (j as f32 / n_th as f32 * TAU + t * 0.7).sin_cos()));

        let m = rotation(self.a, self.b);
        let light = norm([-0.45, -0.65, -0.6]);
        let half = norm([light[0], light[1], light[2] - 1.0]);
        for i in 0..n_phi {
            let s = i as f32 / n_phi as f32;
            let (sp, cp) = (s * TAU).sin_cos();
            let lvl = band_mirrored(&f.spectrum, s);
            let seg = (s * SEGS.0 as f32) as usize;
            let base = hsv(self.hue + 0.12 * (s * TAU * 2.0).sin(), 0.75, 1.0).map(|v| v as f32 / 255.0);
            for (j, &(st, ct)) in self.ring.iter().enumerate() {
                let rr = big_r + r * ct;
                let q = apply(&m, [rr * cp, rr * sp, r * st]);
                let z = q[2] + CAM;
                if z < 0.1 {
                    continue;
                }
                let inv = focal / z;
                let Some(idx) = self.zb.test(cx + q[0] * inv, cy + q[1] * inv, z) else {
                    continue;
                };
                let n = apply(&m, [ct * cp, ct * sp, st]);
                let diff = dot(n, light).max(0.0);
                let spec = dot(n, half).max(0.0).powi(24) * 0.9;
                let lit = (seg + j * SEGS.1 / n_th).is_multiple_of(2);
                let k = if lit { 0.55 + 0.45 * lvl } else { 0.35 };
                let k = k * (0.1 + 0.9 * diff);
                c.px[idx] = rgb(base[0] * k + spec, base[1] * k + spec, base[2] * k + spec);
            }
        }
    }
}
