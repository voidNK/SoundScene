use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv};

const VERTICES: [[f32; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];
const EDGES: [(usize, usize); 12] =
    [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)];

/// Two nested wireframe cubes spinning in opposite directions, with glow
/// trails. Bass spins and inflates them, mids tilt them.
pub struct Cube {
    ax: f32,
    ay: f32,
    hue: f32,
}

impl Cube {
    pub fn new() -> Self {
        Self { ax: 0.3, ay: 0.0, hue: 0.45 }
    }
}

impl Effect for Cube {
    fn name(&self) -> &'static str {
        "Cube"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fade(0.55);
        self.ax += dt * (0.3 + f.mid * 1.2);
        self.ay += dt * (0.5 + f.bass * 1.5);
        self.hue += dt * 0.04 + if f.beat { 0.08 } else { 0.0 };

        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let size = c.w.min(c.h) as f32 * (0.9 + 0.25 * f.pulse + 0.2 * f.bass);

        for (k, (scale, dir)) in [(1.0f32, 1.0f32), (0.5, -1.6)].into_iter().enumerate() {
            let (sx, cxr) = (self.ax * dir).sin_cos();
            let (sy, cyr) = (self.ay * dir).sin_cos();
            let points: Vec<(f32, f32)> = VERTICES
                .iter()
                .map(|v| {
                    let (x, y, z) = (v[0] * scale, v[1] * scale, v[2] * scale);
                    let (y, z) = (y * cxr - z * sx, y * sx + z * cxr);
                    let (x, z) = (x * cyr + z * sy, -x * sy + z * cyr);
                    let d = size / (z + 4.0);
                    (cx + x * d, cy + y * d)
                })
                .collect();
            for (i, &(a, b)) in EDGES.iter().enumerate() {
                let col = hsv(self.hue + k as f32 * 0.5 + i as f32 * 0.02, 0.6, 0.55);
                c.line_add(points[a].0, points[a].1, points[b].0, points[b].1, col);
            }
        }
    }
}
