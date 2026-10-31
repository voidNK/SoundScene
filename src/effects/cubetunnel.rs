use super::Effect;
use super::cubegrid::fill_quad;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, scale};
use std::f32::consts::TAU;

type V3 = [f32; 3];

const RINGS: usize = 16;
const PER_RING: usize = 8;
const SPACING: f32 = 1.6;
const RADIUS: f32 = 2.4;

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
const FACES: [[usize; 4]; 6] = [
    [0, 1, 2, 3],
    [5, 4, 7, 6],
    [4, 0, 3, 7],
    [1, 5, 6, 2],
    [4, 5, 1, 0],
    [3, 2, 6, 7],
];

/// A curving tunnel built from rings of tumbling cubes (dark faces, glowing
/// edges) that fly towards the camera. Level sets the speed, the spectrum
/// sizes the cubes around each ring and beats flash the whole tunnel.
pub struct CubeTunnel {
    dist: f32,
    spin: f32,
    hue: f32,
    flash: f32,
}

impl CubeTunnel {
    pub fn new() -> Self {
        Self {
            dist: 0.0,
            spin: 0.0,
            hue: 0.5,
            flash: 0.0,
        }
    }
}

impl Effect for CubeTunnel {
    fn name(&self) -> &'static str {
        "Cube Tunnel"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.dist += dt * (2.0 + 7.0 * f.level + 4.0 * f.pulse);
        self.spin += dt * (0.2 + 0.6 * f.mid);
        self.hue += dt * 0.02;
        self.flash = if f.beat { 1.0 } else { self.flash * (-dt * 6.0).exp() };

        let bg = scale(hsv(self.hue, 0.6, 1.0), 0.03 + 0.18 * self.flash);
        c.fill(bg);
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let focal = w.max(h) as f32 * 0.45;
        let glow = (w.min(h) as f32 / 100.0).clamp(0.3, 1.0);
        let bins = f.spectrum.len();
        // The tunnel bends: its center drifts sideways with depth.
        let bend = |z: f32| -> (f32, f32) { (0.025 * z * z * (t * 0.31).sin(), 0.02 * z * z * (t * 0.23 + 1.0).cos()) };
        let first = (self.dist / SPACING).floor();
        let offset = self.dist - first * SPACING;
        let far = RINGS as f32 * SPACING;

        // Far rings first so near cubes cover them.
        for ri in (0..RINGS).rev() {
            let z = ri as f32 * SPACING - offset + 0.4;
            if z < 0.3 {
                continue;
            }
            let id = first + ri as f32;
            let (bx, by) = bend(z);
            let fog = (1.0 - z / far).clamp(0.0, 1.0);
            let twist = id * 0.3 + self.spin;
            for k in 0..PER_RING {
                let a = k as f32 / PER_RING as f32 * TAU + twist;
                let lvl = if bins > 0 {
                    f.spectrum[(k * bins / PER_RING).min(bins - 1)]
                } else {
                    0.0
                };
                let size = 0.25 + 0.3 * lvl;
                let center = [bx + a.cos() * RADIUS, by + a.sin() * RADIUS, z];
                let (sa, ca) = (t * 1.1 + id + k as f32).sin_cos();
                let (sb, cb) = (t * 0.7 + id * 0.5).sin_cos();
                let verts = CORNERS.map(|v| {
                    let (y, zz) = (v[1] * ca - v[2] * sa, v[1] * sa + v[2] * ca);
                    let (x, zz) = (v[0] * cb + zz * sb, -v[0] * sb + zz * cb);
                    [center[0] + x * size, center[1] + y * size, center[2] + zz * size]
                });
                if verts.iter().any(|v| v[2] < 0.15) {
                    continue;
                }
                let scr = verts.map(|v| (cx + v[0] / v[2] * focal, cy + v[1] / v[2] * focal));
                let hue = self.hue + id * 0.04 + k as f32 * 0.02;
                // Visible faces: the outward normal points away from the camera ray.
                let mut faces = [(0.0, 0); 6];
                let mut nf = 0;
                for (fi, face) in FACES.iter().enumerate() {
                    let fc = face.iter().fold([0.0; 3], |s, &i| {
                        [s[0] + verts[i][0], s[1] + verts[i][1], s[2] + verts[i][2]]
                    });
                    let n = [
                        fc[0] * 0.25 - center[0],
                        fc[1] * 0.25 - center[1],
                        fc[2] * 0.25 - center[2],
                    ];
                    if n[0] * fc[0] + n[1] * fc[1] + n[2] * fc[2] < 0.0 {
                        let shade = (-n[2] / size.max(1e-3)).clamp(0.0, 1.0);
                        faces[nf] = (shade, fi);
                        nf += 1;
                    }
                }
                let edge = hsv(hue, 0.5 - 0.4 * self.flash, (0.25 + 0.5 * fog) * fog * glow);
                for &(shade, fi) in &faces[..nf] {
                    let col = hsv(hue, 0.7, (0.08 + 0.25 * shade) * fog * (1.0 + self.flash));
                    let quad = FACES[fi].map(|i| scr[i]);
                    fill_quad(c, quad, col);
                    // Shared edges get drawn twice and glow a little brighter.
                    for e in 0..4 {
                        let (a, b) = (quad[e], quad[(e + 1) % 4]);
                        c.line_add(a.0, a.1, b.0, b.1, edge);
                    }
                }
            }
        }
    }
}
