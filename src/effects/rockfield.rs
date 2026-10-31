use super::Effect;
use super::cubegrid::fill_tri;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, rgb};

type V3 = [f32; 3];

const ROCKS: usize = 48;
const STARS: usize = 160;
const FAR: f32 = 40.0;
const NEAR: f32 = 0.6;
const SPREAD: f32 = 9.0;

const ICO_FACES: [[usize; 3]; 20] = [
    [0, 11, 5],
    [0, 5, 1],
    [0, 1, 7],
    [0, 7, 10],
    [0, 10, 11],
    [1, 5, 9],
    [5, 11, 4],
    [11, 10, 2],
    [10, 7, 6],
    [7, 1, 8],
    [3, 9, 4],
    [3, 4, 2],
    [3, 2, 6],
    [3, 6, 8],
    [3, 8, 9],
    [4, 9, 5],
    [2, 4, 11],
    [6, 2, 10],
    [8, 6, 7],
    [9, 8, 1],
];

fn ico_vertices() -> [V3; 12] {
    let p = (1.0 + 5f32.sqrt()) * 0.5;
    let raw = [
        [-1.0, p, 0.0],
        [1.0, p, 0.0],
        [-1.0, -p, 0.0],
        [1.0, -p, 0.0],
        [0.0, -1.0, p],
        [0.0, 1.0, p],
        [0.0, -1.0, -p],
        [0.0, 1.0, -p],
        [p, 0.0, -1.0],
        [p, 0.0, 1.0],
        [-p, 0.0, -1.0],
        [-p, 0.0, 1.0],
    ];
    let l = (1.0 + p * p).sqrt();
    raw.map(|v: V3| [v[0] / l, v[1] / l, v[2] / l])
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

/// A projected rock facet: sort depth, screen corners and color.
type Face = (f32, [(f32, f32); 3], Rgb);

struct Rock {
    verts: [V3; 12],
    pos: V3,
    rot: (f32, f32),
    spin: (f32, f32),
    size: f32,
    tint: V3,
}

impl Rock {
    fn spawn(rng: &mut Rng, z: f32) -> Self {
        let base = ico_vertices();
        let verts = base.map(|v| {
            let k = rng.range(0.7, 1.25);
            [v[0] * k, v[1] * k * rng.range(0.8, 1.0), v[2] * k]
        });
        // Keep the flight path itself mostly clear.
        let mut x = rng.range(-SPREAD, SPREAD);
        let y = rng.range(-SPREAD * 0.6, SPREAD * 0.6);
        if x.abs() < 1.2 && y.abs() < 1.2 {
            x += 2.4 * x.signum();
        }
        let g = rng.range(0.45, 0.7);
        Self {
            verts,
            pos: [x, y, z],
            rot: (rng.range(0.0, 6.3), rng.range(0.0, 6.3)),
            spin: (rng.range(-1.2, 1.2), rng.range(-1.2, 1.2)),
            size: rng.range(0.4, 1.3),
            tint: [g + rng.range(0.0, 0.15), g, g - rng.range(0.0, 0.15)],
        }
    }
}

/// A flight through an asteroid field of tumbling, flat-shaded low-poly
/// rocks with streaking stars behind them. Flight speed follows the level,
/// beats give a speed kick and flash the sunlight.
pub struct RockField {
    rocks: Vec<Rock>,
    stars: Vec<V3>,
    rng: Rng,
    flash: f32,
    order: Vec<(f32, usize)>,
    faces: Vec<Face>,
}

impl RockField {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x0C4);
        let rocks = (0..ROCKS)
            .map(|i| Rock::spawn(&mut rng, NEAR + (i as f32 + 0.5) / ROCKS as f32 * (FAR - NEAR)))
            .collect();
        let stars = (0..STARS)
            .map(|_| [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(0.05, 1.0)])
            .collect();
        Self {
            rocks,
            stars,
            rng,
            flash: 0.0,
            order: Vec::with_capacity(ROCKS),
            faces: Vec::with_capacity(20),
        }
    }
}

impl Effect for RockField {
    fn name(&self) -> &'static str {
        "Rock Field"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.flash = if f.beat { 1.0 } else { self.flash * (-dt * 4.0).exp() };
        let speed = 3.0 + 14.0 * f.level + 10.0 * f.pulse;
        for r in &mut self.rocks {
            r.pos[2] -= speed * dt;
            r.rot.0 += r.spin.0 * dt;
            r.rot.1 += r.spin.1 * dt;
        }
        for i in 0..self.rocks.len() {
            if self.rocks[i].pos[2] < NEAR {
                let z = FAR + self.rng.range(0.0, 3.0);
                self.rocks[i] = Rock::spawn(&mut self.rng, z);
            }
        }

        c.fill([2, 2, 6]);
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let focal = w.max(h) as f32 * 0.6;

        // Stars: move along their depth and streak with speed.
        let streak = (speed * 0.004).min(0.08);
        for s in &mut self.stars {
            s[2] -= speed * dt * 0.02;
            if s[2] < 0.05 {
                s[2] += 0.95;
            }
            let (x0, y0) = (cx + s[0] / s[2] * cx, cy + s[1] / s[2] * cx);
            let z1 = s[2] + streak;
            let (x1, y1) = (cx + s[0] / z1 * cx, cy + s[1] / z1 * cx);
            let b = ((1.0 - s[2]) * 0.8).clamp(0.1, 0.8);
            c.line_add(x0, y0, x1, y1, rgb(b * 0.8, b * 0.85, b));
        }

        let sun = {
            let v = [0.6f32, -0.55, -0.45];
            let l = dot(v, v).sqrt();
            [v[0] / l, v[1] / l, v[2] / l]
        };
        let sun_k = 1.0 + 0.8 * self.flash;
        self.order.clear();
        self.order
            .extend(self.rocks.iter().enumerate().map(|(i, r)| (-r.pos[2], i)));
        self.order.sort_by(|a, b| a.0.total_cmp(&b.0));
        let sway = (0.4 * (t * 0.3).sin(), 0.3 * (t * 0.23).cos());

        for oi in 0..self.order.len() {
            let r = &self.rocks[self.order[oi].1];
            let (sa, ca) = r.rot.0.sin_cos();
            let (sb, cb) = r.rot.1.sin_cos();
            let pos = [r.pos[0] + sway.0, r.pos[1] + sway.1, r.pos[2]];
            let world = r.verts.map(|v| {
                let (y, z) = (v[1] * ca - v[2] * sa, v[1] * sa + v[2] * ca);
                let (x, z) = (v[0] * cb + z * sb, -v[0] * sb + z * cb);
                [pos[0] + x * r.size, pos[1] + y * r.size, pos[2] + z * r.size]
            });
            if world.iter().any(|v| v[2] < 0.2) {
                continue;
            }
            let fog = (1.0 - (pos[2] / FAR)).clamp(0.0, 1.0);
            self.faces.clear();
            for tri in ICO_FACES {
                let (a, b, cc) = (world[tri[0]], world[tri[1]], world[tri[2]]);
                let mut n = cross(sub(b, a), sub(cc, a));
                if dot(n, sub(a, pos)) < 0.0 {
                    n = [-n[0], -n[1], -n[2]];
                }
                // The outward normal must face the camera at the origin.
                if dot(n, a) >= 0.0 {
                    continue;
                }
                let nl = dot(n, n).sqrt().max(1e-6);
                let diff = (dot(n, sun) / nl).max(0.0) * sun_k;
                let k = (0.12 + 0.95 * diff) * fog;
                let col = rgb(
                    r.tint[0] * k + 0.02 * fog,
                    r.tint[1] * k + 0.02 * fog,
                    r.tint[2] * k + 0.05 * fog,
                );
                let pr = |v: V3| (cx + v[0] / v[2] * focal, cy + v[1] / v[2] * focal);
                let depth = a[2] + b[2] + cc[2];
                self.faces.push((-depth, [pr(a), pr(b), pr(cc)], col));
            }
            self.faces.sort_by(|a, b| a.0.total_cmp(&b.0));
            for &(_, p, col) in &self.faces {
                fill_tri(c, p, col);
            }
        }
    }
}
