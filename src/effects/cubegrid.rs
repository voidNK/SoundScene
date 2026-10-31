use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix, rgb, scale};

type V3 = [f32; 3];

const N: usize = 12;
const FOOT: f32 = 0.4;

/// Fills the triangle (overwriting) for every pixel whose center lies inside.
pub(super) fn fill_tri(c: &mut Canvas, p: [(f32, f32); 3], col: Rgb) {
    let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0);
    if area.abs() < 1e-6 || !area.is_finite() || c.w == 0 || c.h == 0 {
        return;
    }
    let s = area.signum();
    let xmin = p[0].0.min(p[1].0).min(p[2].0).floor().max(0.0);
    let xmax = p[0].0.max(p[1].0).max(p[2].0).ceil().min(c.w as f32 - 1.0);
    let ymin = p[0].1.min(p[1].1).min(p[2].1).floor().max(0.0);
    let ymax = p[0].1.max(p[1].1).max(p[2].1).ceil().min(c.h as f32 - 1.0);
    if !(xmin <= xmax && ymin <= ymax) {
        return;
    }
    let edge = |a: (f32, f32), b: (f32, f32), x: f32, y: f32| ((b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)) * s;
    for y in ymin as usize..=ymax as usize {
        let py = y as f32 + 0.5;
        for x in xmin as usize..=xmax as usize {
            let px = x as f32 + 0.5;
            if edge(p[0], p[1], px, py) >= 0.0 && edge(p[1], p[2], px, py) >= 0.0 && edge(p[2], p[0], px, py) >= 0.0 {
                c.px[y * c.w + x] = col;
            }
        }
    }
}

/// Fills a convex quad given in order.
pub(super) fn fill_quad(c: &mut Canvas, p: [(f32, f32); 4], col: Rgb) {
    fill_tri(c, [p[0], p[1], p[2]], col);
    fill_tri(c, [p[0], p[2], p[3]], col);
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A field of flat-shaded cubes on a grid, seen from an orbiting camera.
/// Cube heights follow the spectrum from the center outwards (bass in the
/// middle), beats flash the tops and the camera speeds up with the level.
pub struct CubeGrid {
    yaw: f32,
    heights: Vec<f32>,
    order: Vec<(f32, usize)>,
}

impl CubeGrid {
    pub fn new() -> Self {
        Self {
            yaw: 0.6,
            heights: vec![0.2; N * N],
            order: Vec::with_capacity(N * N),
        }
    }
}

impl Effect for CubeGrid {
    fn name(&self) -> &'static str {
        "Cube Grid"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.yaw += dt * (0.12 + 0.35 * f.level);
        let bins = f.spectrum.len();
        let mid = (N as f32 - 1.0) * 0.5;
        let max_d = (2.0 * mid * mid).sqrt().max(1e-3);
        for (i, hgt) in self.heights.iter_mut().enumerate() {
            let (gx, gz) = ((i % N) as f32 - mid, (i / N) as f32 - mid);
            let d = (gx * gx + gz * gz).sqrt() / max_d;
            let lvl = if bins > 0 {
                f.spectrum[((d * 0.85 * (bins - 1) as f32) as usize).min(bins - 1)]
            } else {
                0.0
            };
            let idle = 0.12 * ((gx * 0.6 + t * 1.3).sin() + (gz * 0.5 - t).cos() + 2.0);
            let target = 0.1 + idle + 3.2 * lvl * (1.0 - 0.4 * d);
            // Fast attack, slow release, like the spectrum bars.
            let k = if target > *hgt { 0.6 } else { (dt * 3.0).min(1.0) };
            *hgt += (target - *hgt) * k;
        }

        let (w, h) = (c.w, c.h);
        for y in 0..h {
            let k = y as f32 / h.max(1) as f32;
            c.px[y * w..(y + 1) * w].fill(rgb(0.02 + 0.03 * k, 0.02, 0.06 + 0.06 * k));
        }
        if w == 0 || h == 0 {
            return;
        }

        // Orbiting camera looking down at the grid.
        let dist = 22.0;
        let pitch = 0.62 + 0.1 * (t * 0.2).sin();
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let target = [0.0, 0.8, 0.0];
        let cam = [
            target[0] + dist * cp * cy,
            target[1] + dist * sp,
            target[2] + dist * cp * sy,
        ];
        let fwd = [-cp * cy, -sp, -cp * sy];
        let right = [-sy, 0.0, cy];
        let up = [
            right[1] * fwd[2] - right[2] * fwd[1],
            right[2] * fwd[0] - right[0] * fwd[2],
            right[0] * fwd[1] - right[1] * fwd[0],
        ];
        let focal = (w as f32 * 0.5 / 10.0).min(h as f32 * 0.5 / 6.8) * dist;
        let (scx, scy) = (w as f32 * 0.5, h as f32 * 0.5);
        let project = |p: V3| -> Option<(f32, f32)> {
            let v = sub(p, cam);
            let z = dot(v, fwd);
            (z > 0.5).then(|| (scx + dot(v, right) / z * focal, scy - dot(v, up) / z * focal))
        };
        let light = [0.45f32, 0.8, 0.35];
        let outline = w.min(h) >= 60;

        // Painter's algorithm: far cubes first.
        self.order.clear();
        for i in 0..N * N {
            let p = [(i % N) as f32 - mid, 0.0, (i / N) as f32 - mid];
            let d = sub(p, cam);
            self.order.push((-dot(d, d), i));
        }
        self.order.sort_by(|a, b| a.0.total_cmp(&b.0));

        let faces: [(V3, [usize; 4]); 5] = [
            ([0.0, 1.0, 0.0], [4, 5, 6, 7]),
            ([0.0, 0.0, -1.0], [0, 1, 5, 4]),
            ([0.0, 0.0, 1.0], [3, 2, 6, 7]),
            ([-1.0, 0.0, 0.0], [0, 3, 7, 4]),
            ([1.0, 0.0, 0.0], [1, 2, 6, 5]),
        ];
        for &(_, i) in &self.order {
            let (gx, gz) = ((i % N) as f32 - mid, (i / N) as f32 - mid);
            let hgt = self.heights[i];
            let corners: [V3; 8] = [
                [gx - FOOT, 0.0, gz - FOOT],
                [gx + FOOT, 0.0, gz - FOOT],
                [gx + FOOT, 0.0, gz + FOOT],
                [gx - FOOT, 0.0, gz + FOOT],
                [gx - FOOT, hgt, gz - FOOT],
                [gx + FOOT, hgt, gz - FOOT],
                [gx + FOOT, hgt, gz + FOOT],
                [gx - FOOT, hgt, gz + FOOT],
            ];
            let mut scr = [(0.0, 0.0); 8];
            let mut ok = true;
            for (s, &p) in scr.iter_mut().zip(&corners) {
                match project(p) {
                    Some(q) => *s = q,
                    None => ok = false,
                }
            }
            if !ok {
                continue;
            }
            let hue = 0.66 - 0.5 * (hgt / 2.8).min(1.0) + 0.06 * (t * 0.1).sin();
            let base = hsv(hue, 0.75, 1.0);
            for (n, idx) in faces {
                let fc = corners[idx[0]];
                let to_cam = sub(cam, fc);
                if dot(n, to_cam) <= 0.0 {
                    continue;
                }
                let diff = dot(n, light).max(0.0);
                let mut col = scale(base, 0.2 + 0.75 * diff);
                if n[1] > 0.5 {
                    col = mix(col, [255, 255, 255], 0.5 * f.pulse * (hgt / 2.0).min(1.0));
                }
                fill_quad(c, idx.map(|k| scr[k]), col);
            }
            // Outline the top so neighboring cubes stay readable (not on tiny tiles).
            if outline {
                let edge = scale(base, 0.25);
                for k in 0..4 {
                    let (a, b) = (scr[4 + k], scr[4 + (k + 1) % 4]);
                    c.line_add(a.0, a.1, b.0, b.1, edge);
                }
            }
        }
    }
}
