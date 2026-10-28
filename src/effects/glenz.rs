use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, rgb, scale};

type V3 = [f32; 3];

const CORNERS: [V3; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];
/// Cube faces as cyclic corner lists plus their outward axis.
const FACES: [([usize; 4], V3); 6] = [
    ([0, 1, 2, 3], [0.0, 0.0, -1.0]),
    ([5, 4, 7, 6], [0.0, 0.0, 1.0]),
    ([4, 0, 3, 7], [-1.0, 0.0, 0.0]),
    ([1, 5, 6, 2], [1.0, 0.0, 0.0]),
    ([4, 5, 1, 0], [0.0, -1.0, 0.0]),
    ([3, 2, 6, 7], [0.0, 1.0, 0.0]),
];
const CAM: f32 = 4.0;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Adds `col` to every pixel whose center lies inside the triangle.
fn fill_tri_add(c: &mut Canvas, p: [(f32, f32); 3], col: Rgb) {
    let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0);
    if area.abs() < 1e-6 || !area.is_finite() || c.w == 0 || c.h == 0 {
        return;
    }
    let s = area.signum();
    let xmin = p[0].0.min(p[1].0).min(p[2].0).floor().max(0.0);
    let xmax = p[0].0.max(p[1].0).max(p[2].0).ceil().min(c.w as f32 - 1.0);
    let ymin = p[0].1.min(p[1].1).min(p[2].1).floor().max(0.0);
    let ymax = p[0].1.max(p[1].1).max(p[2].1).ceil().min(c.h as f32 - 1.0);
    if xmin > xmax || ymin > ymax {
        return;
    }
    let edge = |a: (f32, f32), b: (f32, f32), x: f32, y: f32| ((b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)) * s;
    for y in ymin as usize..=ymax as usize {
        let py = y as f32 + 0.5;
        for x in xmin as usize..=xmax as usize {
            let px = x as f32 + 0.5;
            if edge(p[0], p[1], px, py) >= 0.0 && edge(p[1], p[2], px, py) >= 0.0 && edge(p[2], p[0], px, py) >= 0.0 {
                let i = y * c.w + x;
                c.px[i] = crate::gfx::add(c.px[i], col);
            }
        }
    }
}

/// Glenz vectors: two nested translucent tetrakis cubes (a pyramid on every
/// face) with checkered facets, drawn additively so the back side shows
/// through the glass. Bass spins them, beats push the pyramids out.
pub struct Glenz {
    a: V3,
    hue: f32,
}

impl Glenz {
    pub fn new() -> Self {
        Self {
            a: [0.3, 0.0, 0.1],
            hue: 0.58,
        }
    }
}

struct Tri {
    p: [(f32, f32); 3],
    front: bool,
    light: f32,
    parity: usize,
    solid: usize,
}

impl Effect for Glenz {
    fn name(&self) -> &'static str {
        "Glenz Vectors"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        let spin = 0.3 + 2.2 * f.bass;
        self.a[0] += dt * spin * 0.6;
        self.a[1] += dt * spin;
        self.a[2] += dt * (0.15 + 0.4 * f.mid);
        self.hue += dt * 0.02 + if f.beat { 0.03 } else { 0.0 };

        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let rmax = (cx * cx + cy * cy).sqrt().max(1.0);
        for y in 0..c.h {
            let dy = y as f32 - cy;
            for x in 0..c.w {
                let dx = x as f32 - cx;
                let g = 1.0 - (dx * dx + dy * dy).sqrt() / rmax;
                c.set(x, y, rgb(0.02 + 0.05 * g, 0.02 + 0.03 * g, 0.05 + 0.12 * g));
            }
        }

        let size = w.min(h) * 1.1;
        let apex = 1.5 + 0.6 * f.pulse + 0.2 * f.level;
        let lv = {
            let l = [0.3f32, -0.45, -1.0];
            let n = dot(l, l).sqrt();
            l.map(|v| v / n)
        };
        let mut tris = Vec::with_capacity(48);
        for (solid, (k, dir)) in [(0.7f32, 1.0f32), (0.32, -1.4)].into_iter().enumerate() {
            let [(sx, cx_), (sy, cy_), (sz, cz_)] = self.a.map(|a| (a * dir).sin_cos());
            let rot = |v: V3| -> V3 {
                let (y, z) = (v[1] * cx_ - v[2] * sx, v[1] * sx + v[2] * cx_);
                let (x, z) = (v[0] * cy_ + z * sy, -v[0] * sy + z * cy_);
                let (x, y) = (x * cz_ - y * sz, x * sz + y * cz_);
                [x * k, y * k, z * k]
            };
            let proj = |v: V3| {
                let d = size / (v[2] + CAM).max(0.1);
                (cx + v[0] * d, cy + v[1] * d)
            };
            for (fi, (corners, axis)) in FACES.iter().enumerate() {
                let top = rot(axis.map(|v| v * apex));
                for j in 0..4 {
                    let a = rot(CORNERS[corners[j]]);
                    let b = rot(CORNERS[corners[(j + 1) % 4]]);
                    let g = [
                        (a[0] + b[0] + top[0]) / 3.0,
                        (a[1] + b[1] + top[1]) / 3.0,
                        (a[2] + b[2] + top[2]) / 3.0,
                    ];
                    let mut n = cross(sub(b, a), sub(top, a));
                    if dot(n, g) < 0.0 {
                        n = n.map(|v| -v);
                    }
                    let len = dot(n, n).sqrt().max(1e-6);
                    let n = n.map(|v| v / len);
                    let front = dot(n, sub(g, [0.0, 0.0, -CAM])) < 0.0;
                    tris.push(Tri {
                        p: [proj(a), proj(b), proj(top)],
                        front,
                        light: dot(n, lv),
                        parity: (fi + j) % 2,
                        solid,
                    });
                }
            }
        }

        let flash = 1.0 + 0.3 * f.pulse;
        for front in [false, true] {
            for t in tris.iter().filter(|t| t.front == front) {
                let hue = self.hue + t.solid as f32 * 0.35;
                let l = if front { t.light.max(0.0) } else { t.light.abs() * 0.5 };
                let col = if t.parity == 0 {
                    hsv(hue, 0.85, 0.1 + 0.35 * l)
                } else {
                    hsv(hue + 0.08, 0.2, 0.04 + 0.22 * l)
                };
                let k = if front { flash } else { 0.45 };
                fill_tri_add(c, t.p, scale(col, k.min(1.5)));
            }
        }
    }
}
