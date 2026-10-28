use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};
use std::f32::consts::TAU;

const BALLS: usize = 64;
const SHAPES: usize = 4;

/// Position of ball `i` in shape `s`: cube grid, sphere, torus, cone spiral.
fn shape(s: usize, i: usize) -> [f32; 3] {
    let k = i as f32;
    match s % SHAPES {
        0 => {
            let g = |v: usize| (v % 4) as f32 / 1.5 - 1.0;
            [g(i) * 0.8, g(i / 4) * 0.8, g(i / 16) * 0.8]
        }
        1 => {
            let y = 1.0 - 2.0 * (k + 0.5) / BALLS as f32;
            let r = (1.0 - y * y).max(0.0).sqrt();
            let a = k * 2.399_963;
            [r * a.cos(), y, r * a.sin()]
        }
        2 => {
            let u = (i % 16) as f32 / 16.0 * TAU;
            let v = (i / 16) as f32 / 4.0 * TAU;
            let r = 0.75 + 0.3 * v.cos();
            [r * u.cos(), 0.3 * v.sin(), r * u.sin()]
        }
        _ => {
            let t = k / BALLS as f32;
            let a = t * 4.0 * TAU;
            let r = 0.25 + 0.75 * t;
            [r * a.cos(), 1.0 - 2.0 * t, r * a.sin()]
        }
    }
}

/// Amiga-style vector balls: 64 shaded spheres forming a cube grid, sphere,
/// torus or spiral, depth-sorted and rotating. Beats morph to the next shape,
/// each ball bounces outward with its spectrum band, bass spins the object.
pub struct VectorBalls {
    from: usize,
    to: usize,
    morph: f32,
    since: f32,
    ax: f32,
    ay: f32,
    hue: f32,
}

impl VectorBalls {
    pub fn new() -> Self {
        Self {
            from: 0,
            to: 0,
            morph: 1.0,
            since: 0.0,
            ax: 0.4,
            ay: 0.0,
            hue: 0.0,
        }
    }
}

impl Effect for VectorBalls {
    fn name(&self) -> &'static str {
        "Vector Balls"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.ax += dt * (0.3 + 0.6 * f.mid);
        self.ay += dt * (0.5 + 1.6 * f.bass);
        self.hue += dt * 0.03;
        self.since += dt;
        self.morph = (self.morph + dt * 1.2).min(1.0);
        if self.morph >= 1.0 && ((f.beat && self.since > 2.5) || self.since > 10.0) {
            self.from = self.to;
            self.to = (self.to + 1) % SHAPES;
            self.morph = 0.0;
            self.since = 0.0;
        }

        let (w, h) = (c.w as f32, c.h as f32);
        for y in 0..c.h {
            let g = y as f32 / h.max(1.0);
            let col = rgb(0.02 + 0.04 * g, 0.02 + 0.03 * g, 0.06 + 0.1 * g);
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        let m = self.morph * self.morph * (3.0 - 2.0 * self.morph);
        let (sx, cx) = self.ax.sin_cos();
        let (sy, cy) = self.ay.sin_cos();
        let size = w.min(h) * (1.3 + 0.12 * f.pulse);
        let bins = f.spectrum.len();
        let mut balls: Vec<(f32, f32, f32, f32, usize)> = (0..BALLS)
            .map(|i| {
                let (a, b) = (shape(self.from, i), shape(self.to, i));
                let pop = 1.0 + 0.18 * if bins > 0 { f.spectrum[i * bins / BALLS] } else { 0.0 };
                let p = [0, 1, 2].map(|j| (a[j] + (b[j] - a[j]) * m) * pop);
                let (y, z) = (p[1] * cx - p[2] * sx, p[1] * sx + p[2] * cx);
                let (x, z) = (p[0] * cy + z * sy, -p[0] * sy + z * cy);
                let d = size / (z + 3.5);
                (w * 0.5 + x * d, h * 0.5 + y * d, z, d * 0.11 * (1.0 + 0.3 * f.pulse), i)
            })
            .collect();
        balls.sort_by(|a, b| b.2.total_cmp(&a.2));

        let light = {
            let l = [-0.5f32, -0.6, 0.62];
            let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
            l.map(|v| v / n)
        };
        for (bx, by, z, r, i) in balls {
            let r = r.max(0.7);
            if !(bx + r >= 0.0 && by + r >= 0.0 && bx - r < w && by - r < h) {
                continue;
            }
            let base = hsv(self.hue + i as f32 / BALLS as f32 * 0.5, 0.75, 1.0);
            let depth = (1.0 - (z + 1.2) * 0.25).clamp(0.35, 1.0);
            let x0 = (bx - r).floor().max(0.0) as usize;
            let x1 = ((bx + r).ceil().max(0.0) as usize).min(c.w - 1);
            let y0 = (by - r).floor().max(0.0) as usize;
            let y1 = ((by + r).ceil().max(0.0) as usize).min(c.h - 1);
            for y in y0..=y1 {
                let ny = (y as f32 + 0.5 - by) / r;
                for x in x0..=x1 {
                    let nx = (x as f32 + 0.5 - bx) / r;
                    let q = nx * nx + ny * ny;
                    if q > 1.0 {
                        continue;
                    }
                    let nz = (1.0 - q).sqrt();
                    let diff = (nx * light[0] + ny * light[1] + nz * light[2]).max(0.0);
                    let spec = diff.powi(24) * 0.9;
                    let k = (0.12 + 0.88 * diff) * depth;
                    let ch = |j: usize| base[j] as f32 / 255.0 * k + spec;
                    c.set(x, y, rgb(ch(0), ch(1), ch(2)));
                }
            }
        }
    }
}
