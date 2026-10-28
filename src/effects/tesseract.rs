use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, scale};

/// Projected vertex: screen position, 4D depth `w` and 3D depth `z`.
#[derive(Clone, Copy, Default)]
struct Vert {
    x: f32,
    y: f32,
    w: f32,
    z: f32,
}

/// Rotating 4D hypercube, projected to 3D and then to 2D. Edges glow in
/// colors that follow the 4D depth, so the inner cube and outer cube differ.
/// Bass drives the 4D rotation, beats inflate it and spark the vertices.
pub struct Tesseract {
    xw: f32,
    yw: f32,
    zw: f32,
    yaw: f32,
    hue: f32,
    edges: Vec<(usize, usize)>,
}

impl Tesseract {
    pub fn new() -> Self {
        let mut edges = Vec::new();
        for a in 0..16usize {
            for bit in 0..4 {
                let b = a ^ (1 << bit);
                if a < b {
                    edges.push((a, b));
                }
            }
        }
        Self {
            xw: 0.0,
            yw: 0.3,
            zw: 0.0,
            yaw: 0.0,
            hue: 0.55,
            edges,
        }
    }
}

/// Line with a soft one-pixel halo, color blended from `ca` to `cb`.
fn glow_line(c: &mut Canvas, a: Vert, b: Vert, ca: Rgb, cb: Rgb) {
    let steps = (b.x - a.x).abs().max((b.y - a.y).abs()).ceil().clamp(1.0, 1000.0) as i32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (a.x + (b.x - a.x) * t).round() as i32;
        let y = (a.y + (b.y - a.y) * t).round() as i32;
        let col = crate::gfx::mix(ca, cb, t);
        let halo = scale(col, 0.3);
        c.plot_add(x, y, col);
        c.plot_add(x + 1, y, halo);
        c.plot_add(x - 1, y, halo);
        c.plot_add(x, y + 1, halo);
        c.plot_add(x, y - 1, halo);
    }
}

impl Effect for Tesseract {
    fn name(&self) -> &'static str {
        "Tesseract"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.5);
        let spin = 0.25 + 1.4 * f.bass + 0.8 * f.pulse;
        self.xw += dt * spin * 0.9;
        self.yw += dt * spin * 0.55;
        self.zw += dt * spin * 0.35;
        self.yaw += dt * (0.2 + 0.5 * f.mid);
        self.hue += dt * 0.025 + if f.beat { 0.05 } else { 0.0 };

        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let size = w.min(h) * (0.62 + 0.12 * f.pulse + 0.08 * f.level);
        let rot = |a: f32, b: f32, ang: f32| {
            let (s, co) = ang.sin_cos();
            (a * co - b * s, a * s + b * co)
        };
        let tilt = 0.45 + 0.2 * (t * 0.23).sin();

        let mut verts = [Vert::default(); 16];
        for (i, v) in verts.iter_mut().enumerate() {
            let s = |bit: usize| if i & (1 << bit) != 0 { 1.0 } else { -1.0 };
            let (mut x, mut y, mut z, mut wd) = (s(0), s(1), s(2), s(3));
            (x, wd) = rot(x, wd, self.xw);
            (y, wd) = rot(y, wd, self.yw);
            (z, wd) = rot(z, wd, self.zw);
            // 4D perspective: the far cube (negative w) shrinks inside.
            let k = 1.6 / (3.0 - wd).max(0.5);
            let (x3, y3, z3) = (x * k, y * k, z * k);
            let (x3, z3) = rot(x3, z3, self.yaw);
            let (y3, z3) = rot(y3, z3, tilt);
            let d = 1.0 / (z3 + 5.0).max(1.0);
            *v = Vert {
                x: cx + x3 * size * d * 2.2,
                y: cy + y3 * size * d * 2.2,
                w: wd,
                z: z3,
            };
        }

        let color = |v: Vert| {
            let near = (1.0 - (v.z + 3.0) / 6.0).clamp(0.15, 1.0);
            hsv(self.hue + v.w * 0.12, 0.75, 0.25 + 0.45 * near)
        };
        for &(a, b) in &self.edges {
            glow_line(c, verts[a], verts[b], color(verts[a]), color(verts[b]));
        }
        let spark = 0.3 + 0.7 * f.pulse;
        for v in verts {
            let col = hsv(self.hue + v.w * 0.12 + 0.5, 0.3, spark);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                c.plot_add(v.x as i32 + dx, v.y as i32 + dy, col);
            }
        }
    }
}
