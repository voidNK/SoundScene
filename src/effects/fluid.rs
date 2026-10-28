use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};
use std::f32::consts::TAU;

/// Pixels per simulation cell along each axis.
const CELL: usize = 2;
const SOLVER_ITERS: usize = 16;
const EMITTERS: usize = 3;

/// Stable-fluids (Jos Stam) smoke: three orbiting nozzles inject colored dye
/// and force, each driven by one band (bass, mid, treble); beats fire a big
/// puff from a random nozzle. Simulated at half resolution and upscaled
/// smoothly.
pub struct Fluid {
    gw: usize,
    gh: usize,
    u: Vec<f32>,
    v: Vec<f32>,
    u0: Vec<f32>,
    v0: Vec<f32>,
    dye: [Vec<f32>; 3],
    tmp: Vec<f32>,
    out: Vec<[f32; 3]>,
    rng: Rng,
    angle: f32,
    hue: f32,
}

impl Fluid {
    pub fn new() -> Self {
        Self {
            gw: 0,
            gh: 0,
            u: Vec::new(),
            v: Vec::new(),
            u0: Vec::new(),
            v0: Vec::new(),
            dye: [Vec::new(), Vec::new(), Vec::new()],
            tmp: Vec::new(),
            out: Vec::new(),
            rng: Rng::new(0xF1D),
            angle: 0.0,
            hue: 0.0,
        }
    }

    fn resize(&mut self, gw: usize, gh: usize) {
        (self.gw, self.gh) = (gw, gh);
        let n = (gw + 2) * (gh + 2);
        for b in [&mut self.u, &mut self.v, &mut self.u0, &mut self.v0, &mut self.tmp] {
            *b = vec![0.0; n];
        }
        for d in &mut self.dye {
            *d = vec![0.0; n];
        }
        self.out = vec![[0.0; 3]; gw * gh];
    }

    /// Adds dye and velocity in a disc around cell (cx, cy).
    fn splat(&mut self, cx: f32, cy: f32, radius: f32, fx: f32, fy: f32, col: [f32; 3]) {
        let s = self.gw + 2;
        let r = radius.max(0.5);
        let x0 = (cx - r).floor().max(1.0) as usize;
        let x1 = ((cx + r).ceil() as usize).min(self.gw);
        let y0 = (cy - r).floor().max(1.0) as usize;
        let y1 = ((cy + r).ceil() as usize).min(self.gh);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                let k = (1.0 - (dx * dx + dy * dy) / (r * r)).max(0.0);
                if k > 0.0 {
                    let i = y * s + x;
                    self.u[i] += fx * k;
                    self.v[i] += fy * k;
                    for (d, c) in self.dye.iter_mut().zip(col) {
                        d[i] = (d[i] + c * k).min(4.0);
                    }
                }
            }
        }
    }
}

/// Mirrors the border cells; `b` = 1 or 2 negates the x or y component (solid walls).
fn set_bnd(b: u8, x: &mut [f32], gw: usize, gh: usize) {
    let s = gw + 2;
    for i in 1..=gw {
        let (top, bot) = (x[s + i], x[gh * s + i]);
        x[i] = if b == 2 { -top } else { top };
        x[(gh + 1) * s + i] = if b == 2 { -bot } else { bot };
    }
    for j in 1..=gh {
        let (left, right) = (x[j * s + 1], x[j * s + gw]);
        x[j * s] = if b == 1 { -left } else { left };
        x[j * s + gw + 1] = if b == 1 { -right } else { right };
    }
    x[0] = 0.5 * (x[1] + x[s]);
    x[gw + 1] = 0.5 * (x[gw] + x[s + gw + 1]);
    x[(gh + 1) * s] = 0.5 * (x[(gh + 1) * s + 1] + x[gh * s]);
    x[(gh + 1) * s + gw + 1] = 0.5 * (x[(gh + 1) * s + gw] + x[gh * s + gw + 1]);
}

/// Semi-Lagrangian advection of `d0` into `d` along (u, v), in cells per second.
#[allow(clippy::too_many_arguments)]
fn advect(b: u8, d: &mut [f32], d0: &[f32], u: &[f32], v: &[f32], dt: f32, gw: usize, gh: usize) {
    let s = gw + 2;
    let (maxx, maxy) = (gw as f32 + 0.5, gh as f32 + 0.5);
    for j in 1..=gh {
        for i in 1..=gw {
            let k = j * s + i;
            let x = (i as f32 - dt * u[k]).clamp(0.5, maxx);
            let y = (j as f32 - dt * v[k]).clamp(0.5, maxy);
            let (i0, j0) = (x as usize, y as usize);
            let (sx, sy) = (x - i0 as f32, y - j0 as f32);
            let (i1, j1) = ((i0 + 1).min(gw + 1), (j0 + 1).min(gh + 1));
            d[k] = (1.0 - sx) * ((1.0 - sy) * d0[j0 * s + i0] + sy * d0[j1 * s + i0])
                + sx * ((1.0 - sy) * d0[j0 * s + i1] + sy * d0[j1 * s + i1]);
        }
    }
    set_bnd(b, d, gw, gh);
}

/// Makes (u, v) divergence-free; `p` and `div` are scratch buffers.
fn project(u: &mut [f32], v: &mut [f32], p: &mut [f32], div: &mut [f32], gw: usize, gh: usize) {
    let s = gw + 2;
    for j in 1..=gh {
        for i in 1..=gw {
            let k = j * s + i;
            div[k] = -0.5 * (u[k + 1] - u[k - 1] + v[k + s] - v[k - s]);
            p[k] = 0.0;
        }
    }
    set_bnd(0, div, gw, gh);
    set_bnd(0, p, gw, gh);
    for _ in 0..SOLVER_ITERS {
        for j in 1..=gh {
            for i in 1..=gw {
                let k = j * s + i;
                p[k] = (div[k] + p[k - 1] + p[k + 1] + p[k - s] + p[k + s]) * 0.25;
            }
        }
        set_bnd(0, p, gw, gh);
    }
    for j in 1..=gh {
        for i in 1..=gw {
            let k = j * s + i;
            u[k] -= 0.5 * (p[k + 1] - p[k - 1]);
            v[k] -= 0.5 * (p[k + s] - p[k - s]);
        }
    }
    set_bnd(1, u, gw, gh);
    set_bnd(2, v, gw, gh);
}

impl Effect for Fluid {
    fn name(&self) -> &'static str {
        "Fluid"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        if (gw, gh) != (self.gw, self.gh) {
            self.resize(gw, gh);
        }
        let s = gw + 2;
        let size = gw.min(gh) as f32;

        self.angle += dt * (0.25 + 0.5 * f.mid);
        self.hue += dt * 0.015;
        let bands = [f.bass, f.mid, f.treble];
        let (cx, cy) = (gw as f32 * 0.5 + 0.5, gh as f32 * 0.5 + 0.5);
        let nozzle = |e: usize, angle: f32| {
            let a = angle + e as f32 * TAU / EMITTERS as f32;
            let (px, py) = (cx + a.cos() * gw as f32 * 0.3, cy + a.sin() * gh as f32 * 0.3);
            // Aim roughly tangentially, slightly inwards, wobbling over time.
            let dir = a + TAU * 0.3 + (angle * 3.0 + e as f32).sin() * 0.6;
            (px, py, dir.cos(), dir.sin())
        };
        for (e, &band) in bands.iter().enumerate() {
            let (px, py, dx, dy) = nozzle(e, self.angle);
            let force = size * (0.5 + 3.0 * band) * dt * 4.0;
            let col = hsv(self.hue + e as f32 / EMITTERS as f32, 1.0, 1.0);
            let amount = (0.4 + 2.5 * band) * dt * 4.0;
            let col = col.map(|v| v as f32 / 255.0 * amount);
            self.splat(px, py, (size * 0.06).max(1.0), dx * force, dy * force, col);
        }
        if f.beat {
            let e = self.rng.next_u32() as usize % EMITTERS;
            let (px, py, dx, dy) = nozzle(e, self.angle);
            let force = size * (1.5 + 2.5 * f.bass);
            let col = hsv(self.hue + self.rng.f32(), 0.7, 1.0).map(|v| v as f32 / 255.0 * 1.5);
            self.splat(px, py, (size * 0.12).max(1.5), dx * force, dy * force, col);
        }

        // Light vorticity confinement keeps the swirls alive at low resolution.
        let curl = &mut self.tmp;
        for j in 1..=gh {
            for i in 1..=gw {
                let k = j * s + i;
                curl[k] = 0.5 * (self.v[k + 1] - self.v[k - 1] - self.u[k + s] + self.u[k - s]);
            }
        }
        set_bnd(0, curl, gw, gh);
        let eps = 2.0 * dt;
        for j in 1..=gh {
            for i in 1..=gw {
                let k = j * s + i;
                let nx = 0.5 * (curl[k + 1].abs() - curl[k - 1].abs());
                let ny = 0.5 * (curl[k + s].abs() - curl[k - s].abs());
                let len = (nx * nx + ny * ny).sqrt() + 1e-5;
                self.u[k] += eps * ny / len * curl[k];
                self.v[k] -= eps * nx / len * curl[k];
            }
        }

        project(&mut self.u, &mut self.v, &mut self.u0, &mut self.v0, gw, gh);
        self.u0.copy_from_slice(&self.u);
        self.v0.copy_from_slice(&self.v);
        advect(1, &mut self.u, &self.u0, &self.u0, &self.v0, dt, gw, gh);
        advect(2, &mut self.v, &self.v0, &self.u0, &self.v0, dt, gw, gh);
        project(&mut self.u, &mut self.v, &mut self.u0, &mut self.v0, gw, gh);

        let damp = (-dt * 0.4).exp();
        let vmax = size * 3.0;
        let mut finite = true;
        for k in 0..self.u.len() {
            self.u[k] = (self.u[k] * damp).clamp(-vmax, vmax);
            self.v[k] = (self.v[k] * damp).clamp(-vmax, vmax);
            finite &= self.u[k].is_finite() && self.v[k].is_finite();
        }
        if !finite {
            self.resize(gw, gh);
            return;
        }
        let fade = (-dt * 0.6).exp();
        for d in &mut self.dye {
            self.tmp.copy_from_slice(d);
            advect(0, d, &self.tmp, &self.u, &self.v, dt, gw, gh);
            for x in d.iter_mut() {
                *x = if x.is_finite() {
                    (*x * fade).clamp(0.0, 4.0)
                } else {
                    0.0
                };
            }
        }

        for j in 0..gh {
            for i in 0..gw {
                let k = (j + 1) * s + i + 1;
                let tone = |d: f32| 1.0 - (-d * 1.3).exp();
                self.out[j * gw + i] = [
                    tone(self.dye[0][k]) + 0.01,
                    tone(self.dye[1][k]) + 0.01,
                    tone(self.dye[2][k]) + 0.04,
                ];
            }
        }
        upscale(c, &self.out, gw, gh);
    }
}
