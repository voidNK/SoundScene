use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix, scale};

type V3 = [f32; 3];

/// Seconds each solid is shown, including the shrink/grow transition.
const SHOW: f32 = 8.0;
const CAM: f32 = 4.0;

struct Solid {
    verts: Vec<V3>,
    /// Faces as vertex index loops, counter-clockwise seen from outside.
    faces: Vec<Vec<usize>>,
}

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

fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Builds the convex hull faces of points on the unit sphere by brute force:
/// a plane through three points is a face if no point lies outside it.
fn hull(points: Vec<V3>) -> Solid {
    let verts: Vec<V3> = points.into_iter().map(normalize).collect();
    let n = verts.len();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut normals: Vec<V3> = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                let mut nrm = normalize(cross(sub(verts[j], verts[i]), sub(verts[k], verts[i])));
                let mut d = dot(nrm, verts[i]);
                if d < 0.0 {
                    nrm = nrm.map(|v| -v);
                    d = -d;
                }
                if d < 1e-4 || verts.iter().any(|&p| dot(nrm, p) > d + 1e-4) {
                    continue;
                }
                if normals.iter().any(|&m| dot(m, nrm) > 0.9999) {
                    continue;
                }
                // Collect all points on the plane and sort them around the center.
                let mut on: Vec<usize> = (0..n).filter(|&p| (dot(nrm, verts[p]) - d).abs() < 1e-4).collect();
                let center = on.iter().fold([0.0; 3], |acc, &p| {
                    [acc[0] + verts[p][0], acc[1] + verts[p][1], acc[2] + verts[p][2]]
                });
                let e1 = normalize(sub(verts[on[0]], center.map(|v| v / on.len() as f32)));
                let e2 = cross(nrm, e1);
                on.sort_by(|&a, &b| {
                    let ang = |p: usize| dot(verts[p], e2).atan2(dot(verts[p], e1));
                    ang(a).total_cmp(&ang(b))
                });
                faces.push(on);
                normals.push(nrm);
            }
        }
    }
    Solid { verts, faces }
}

fn solids() -> Vec<Solid> {
    let phi = (1.0 + 5f32.sqrt()) * 0.5;
    let ip = 1.0 / phi;
    let cyclic = |a: f32, b: f32| {
        let mut v = Vec::new();
        for sa in [-1.0, 1.0] {
            for sb in [-1.0, 1.0] {
                let (x, y) = (a * sa, b * sb);
                v.extend([[0.0, x, y], [x, y, 0.0], [y, 0.0, x]]);
            }
        }
        v
    };
    let cube: Vec<V3> = (0..8)
        .map(|i| {
            [
                (i & 1) as f32 * 2.0 - 1.0,
                (i >> 1 & 1) as f32 * 2.0 - 1.0,
                (i >> 2 & 1) as f32 * 2.0 - 1.0,
            ]
        })
        .collect();
    let mut dodeca = cube.clone();
    dodeca.extend(cyclic(ip, phi));
    vec![
        hull(cyclic(1.0, phi)),
        hull(dodeca),
        hull(vec![
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ]),
        hull(cube),
        hull(vec![
            [1.0, 1.0, 1.0],
            [1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
        ]),
    ]
}

/// Overwrites every pixel whose center lies inside the triangle.
fn fill_tri(c: &mut Canvas, p: [(f32, f32); 3], col: Rgb) {
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
                c.set(x, y, col);
            }
        }
    }
}

/// Flat-shaded Platonic solids taking turns: icosahedron, dodecahedron,
/// octahedron, cube, tetrahedron. Pulse scales them, mid spins them.
pub struct Polyhedra {
    solids: Vec<Solid>,
    clock: f32,
    rot: [f32; 2],
    hue: f32,
}

impl Polyhedra {
    pub fn new() -> Self {
        Self {
            solids: solids(),
            clock: 0.0,
            rot: [0.0, 0.0],
            hue: 0.05,
        }
    }
}

impl Effect for Polyhedra {
    fn name(&self) -> &'static str {
        "Polyhedra"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.clock += dt;
        self.rot[0] += dt * (0.5 + f.mid * 1.5);
        self.rot[1] += dt * (0.3 + f.treble * 0.8);
        self.hue += dt * 0.02 + if f.beat { 0.03 } else { 0.0 };
        if c.w == 0 || c.h == 0 {
            return;
        }

        // Dark radial backdrop that brightens with the bass.
        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let inner = hsv(self.hue + 0.5, 0.6, 0.12 + 0.18 * f.bass);
        let rmax = (cx * cx + cy * cy).sqrt().max(1.0);
        for y in 0..c.h {
            for x in 0..c.w {
                let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / rmax;
                c.set(x, y, scale(inner, 1.0 - d));
            }
        }

        let count = self.solids.len().max(1);
        let index = (self.clock / SHOW) as usize % count;
        let local = self.clock % SHOW;
        let appear = (local / 0.6).min(1.0).min((SHOW - local) / 0.6).max(0.0);
        let appear = appear * appear * (3.0 - 2.0 * appear);
        let size = w.min(h) * 0.36 * (1.0 + 0.25 * f.pulse) * appear;
        if size < 0.5 {
            return;
        }

        let (sa, ca) = self.rot[0].sin_cos();
        let (sb, cb) = (self.rot[1] + 0.3 * (t * 0.5).sin()).sin_cos();
        let solid = &self.solids[index];
        let verts: Vec<V3> = solid
            .verts
            .iter()
            .map(|v| {
                let (x, z) = (v[0] * ca + v[2] * sa, -v[0] * sa + v[2] * ca);
                let (y, z) = (v[1] * cb - z * sb, v[1] * sb + z * cb);
                [x, y, z]
            })
            .collect();
        let screen: Vec<(f32, f32)> = verts
            .iter()
            .map(|v| {
                let p = CAM / (CAM + v[2]) * size;
                (cx + v[0] * p, cy + v[1] * p)
            })
            .collect();
        let light = normalize([-0.5, -0.7, -0.6]);
        let base = hsv(self.hue + index as f32 * 0.2, 0.7, 1.0);
        let rim = hsv(self.hue + index as f32 * 0.2 + 0.1, 0.3, 1.0);
        for (fi, face) in solid.faces.iter().enumerate() {
            if face.len() < 3 {
                continue;
            }
            let nrm = normalize(cross(
                sub(verts[face[1]], verts[face[0]]),
                sub(verts[face[2]], verts[face[0]]),
            ));
            let center = face.iter().fold([0.0; 3], |a, &i| {
                [a[0] + verts[i][0], a[1] + verts[i][1], a[2] + verts[i][2]]
            });
            let nrm = if dot(nrm, center) < 0.0 { nrm.map(|v| -v) } else { nrm };
            // Back-face culling: the camera sits at z = -CAM looking towards +z.
            let k = 1.0 / face.len() as f32;
            let to_cam = sub([0.0, 0.0, -CAM], center.map(|v| v * k));
            if dot(nrm, to_cam) <= 0.0 {
                continue;
            }
            let lambert = dot(nrm, light).max(0.0);
            let tint = mix(base, rim, (fi % 3) as f32 * 0.15);
            let col = scale(tint, 0.15 + 0.85 * lambert);
            for i in 1..face.len() - 1 {
                fill_tri(c, [screen[face[0]], screen[face[i]], screen[face[i + 1]]], col);
            }
            // Thin edges keep the shape readable on tiny tiles.
            let edge = scale(rim, 0.25 + 0.2 * f.pulse);
            for i in 0..face.len() {
                let (a, b) = (screen[face[i]], screen[face[(i + 1) % face.len()]]);
                c.line_add(a.0, a.1, b.0, b.1, edge);
            }
        }
    }
}
