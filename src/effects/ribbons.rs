use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix};

const RIBBONS: usize = 5;
const SEGMENTS: usize = 70;

type V3 = [f32; 3];

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

fn norm(a: V3) -> V3 {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l > 1e-6 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// Center line of ribbon `i` at curve parameter `u`.
fn path(i: usize, u: f32) -> V3 {
    let k = i as f32;
    [
        1.4 * (1.1 * u + k * 1.7).sin() + 0.3 * (2.3 * u + k).sin(),
        0.85 * (1.7 * u + k * 2.3).sin() + 0.2 * (3.1 * u - k).cos(),
        1.1 * (0.9 * u + k * 0.9).cos(),
    ]
}

/// Screen-space vertex: x, y and depth.
type P = [f32; 3];

/// Flat-colored triangle with a z-buffer.
fn triangle(c: &mut Canvas, zb: &mut [f32], a: P, mut b: P, mut d: P, col: Rgb) {
    let edge = |p: P, q: P, x: f32, y: f32| (q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0]);
    let mut area = edge(a, b, d[0], d[1]);
    if area.abs() < 1e-4 || !area.is_finite() {
        return;
    }
    if area < 0.0 {
        std::mem::swap(&mut b, &mut d);
        area = -area;
    }
    let (w, h) = (c.w as f32, c.h as f32);
    let x0 = a[0].min(b[0]).min(d[0]).floor().max(0.0);
    let x1 = a[0].max(b[0]).max(d[0]).ceil().min(w);
    let y0 = a[1].min(b[1]).min(d[1]).floor().max(0.0);
    let y1 = a[1].max(b[1]).max(d[1]).ceil().min(h);
    if !(x0 < x1 && y0 < y1) {
        return;
    }
    for y in y0 as usize..y1 as usize {
        let py = y as f32 + 0.5;
        for x in x0 as usize..x1 as usize {
            let px = x as f32 + 0.5;
            let w0 = edge(b, d, px, py);
            let w1 = edge(d, a, px, py);
            let w2 = edge(a, b, px, py);
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let z = (w0 * a[2] + w1 * b[2] + w2 * d[2]) / area;
            let i = y * c.w + x;
            if z < zb[i] {
                zb[i] = z;
                c.px[i] = col;
            }
        }
    }
}

/// Five shaded 3D ribbons snake through space and twist around their own
/// path. Each ribbon's width follows one part of the spectrum, the loudness
/// sets the flow speed and beats make them flare.
pub struct Ribbons {
    phase: f32,
    yaw: f32,
    hue: f32,
    zbuf: Vec<f32>,
}

impl Ribbons {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            yaw: 0.0,
            hue: 0.0,
            zbuf: Vec::new(),
        }
    }
}

impl Effect for Ribbons {
    fn name(&self) -> &'static str {
        "Ribbons"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.phase += dt * (0.35 + 0.9 * f.level + 0.5 * f.pulse);
        self.yaw += dt * (0.1 + 0.25 * f.mid);
        self.hue += dt * 0.02 + if f.beat { 0.05 } else { 0.0 };

        let n = c.w * c.h;
        self.zbuf.clear();
        self.zbuf.resize(n, f32::MAX);
        let (w, h) = (c.w as f32, c.h as f32);
        let top = hsv(self.hue + 0.6, 0.6, 0.02);
        let bottom = hsv(self.hue + 0.7, 0.7, 0.12 + 0.06 * f.pulse);
        for y in 0..c.h {
            let col = mix(top, bottom, y as f32 / h.max(1.0));
            c.px[y * c.w..(y + 1) * c.w].fill(col);
        }

        let (cx, cy) = (w * 0.5, h * 0.5);
        let size = w.min(h) * 0.9;
        let (sy, cyw) = self.yaw.sin_cos();
        let pitch = 0.3 * (t * 0.17).sin();
        let (sp, cp) = pitch.sin_cos();
        let project = |v: V3| -> P {
            let (x, z) = (v[0] * cyw + v[2] * sy, -v[0] * sy + v[2] * cyw);
            let (y, z) = (v[1] * cp - z * sp, v[1] * sp + z * cp);
            let d = 3.5 / (z + 4.0).max(0.5);
            [cx + x * size * d * 0.45, cy + y * size * d * 0.45, z]
        };
        let rotate = |v: V3| -> V3 {
            let (x, z) = (v[0] * cyw + v[2] * sy, -v[0] * sy + v[2] * cyw);
            let (y, z) = (v[1] * cp - z * sp, v[1] * sp + z * cp);
            [x, y, z]
        };
        let light = norm([-0.5, -0.7, -0.6]);
        let bins = f.spectrum.len();
        let band = |i: usize| {
            if bins == 0 {
                return 0.0;
            }
            let (lo, hi) = (
                i * bins / RIBBONS,
                ((i + 1) * bins / RIBBONS).max(i * bins / RIBBONS + 1),
            );
            let s = &f.spectrum[lo.min(bins - 1)..hi.min(bins)];
            s.iter().sum::<f32>() / s.len().max(1) as f32
        };

        for i in 0..RIBBONS {
            let width = 0.05 + 0.25 * band(i) + 0.06 * f.pulse;
            let hue = self.hue + i as f32 / RIBBONS as f32;
            let head = self.phase * (1.0 + 0.13 * i as f32) + i as f32 * 3.0;
            let mut prev: Option<(P, P)> = None;
            for s in 0..=SEGMENTS {
                let k = s as f32 / SEGMENTS as f32;
                let u = head - k * 2.2;
                let p = path(i, u);
                let tan = norm(sub(path(i, u + 0.01), p));
                let side = norm(cross(tan, [0.0, 1.0, 0.0]));
                let up = cross(side, tan);
                let a = k * 7.0 + self.phase * 1.5 + i as f32;
                let (sa, ca) = a.sin_cos();
                let wd = [
                    side[0] * ca + up[0] * sa,
                    side[1] * ca + up[1] * sa,
                    side[2] * ca + up[2] * sa,
                ];
                let hw = width * (1.0 - 0.75 * k);
                let l = project([p[0] + wd[0] * hw, p[1] + wd[1] * hw, p[2] + wd[2] * hw]);
                let r = project([p[0] - wd[0] * hw, p[1] - wd[1] * hw, p[2] - wd[2] * hw]);
                if let Some((pl, pr)) = prev {
                    let nrm = rotate(norm(cross(tan, wd)));
                    let lambert = (nrm[0] * light[0] + nrm[1] * light[1] + nrm[2] * light[2]).abs();
                    // Two-sided specular toward the viewer.
                    let half = norm([light[0], light[1], light[2] - 1.0]);
                    let spec = (nrm[0] * half[0] + nrm[1] * half[1] + nrm[2] * half[2]).abs().powi(20);
                    let fade = 1.0 - 0.85 * k;
                    let base = hsv(hue + k * 0.15, 0.75 - 0.4 * spec, (0.12 + 0.8 * lambert) * fade);
                    let col = mix(base, [255, 255, 255], spec * 0.7 * fade);
                    triangle(c, &mut self.zbuf, pl, pr, l, col);
                    triangle(c, &mut self.zbuf, pr, r, l, col);
                }
                prev = Some((l, r));
            }
        }
    }
}
