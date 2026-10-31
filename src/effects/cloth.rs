use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};

/// Physics substep in seconds and the cap per frame.
const STEP: f32 = 1.0 / 60.0;
const MAX_STEPS: usize = 6;
const ITERATIONS: usize = 4;

#[derive(Clone, Copy)]
struct Point {
    p: [f32; 3],
    old: [f32; 3],
}

/// A Verlet cloth hanging from the top edge, seen from the front and lit
/// from the upper left. Bass blows wind gusts into it, beats snap it with a
/// punch, and stretched areas glow hotter.
pub struct Cloth {
    pts: Vec<Point>,
    cols: usize,
    rows: usize,
    /// Rest spacing between neighbours, in pixels.
    rest: (f32, f32),
    size: (usize, usize),
    zbuf: Vec<f32>,
    light: Vec<f32>,
    stretch: Vec<f32>,
    time: f32,
    gust: f32,
    hue: f32,
}

impl Cloth {
    pub fn new() -> Self {
        Self {
            pts: Vec::new(),
            cols: 0,
            rows: 0,
            rest: (1.0, 1.0),
            size: (0, 0),
            zbuf: Vec::new(),
            light: Vec::new(),
            stretch: Vec::new(),
            time: 0.0,
            gust: 0.0,
            hue: 0.62,
        }
    }

    fn reset(&mut self, w: usize, h: usize) {
        self.size = (w, h);
        self.cols = (w / 7).clamp(3, 30);
        self.rows = (h / 6).clamp(3, 20);
        let (cw, ch) = (w as f32 * 0.84, h as f32 * 0.8);
        let (x0, y0) = (w as f32 * 0.08, h as f32 * 0.04);
        self.rest = (cw / (self.cols - 1) as f32, ch / (self.rows - 1) as f32);
        self.pts.clear();
        for r in 0..self.rows {
            for col in 0..self.cols {
                let p = [x0 + col as f32 * self.rest.0, y0 + r as f32 * self.rest.1, 0.0];
                self.pts.push(Point { p, old: p });
            }
        }
        self.zbuf = vec![0.0; w * h];
        self.light = vec![0.0; self.pts.len()];
        self.stretch = vec![0.0; self.pts.len()];
    }

    fn step(&mut self, h: f32, scale: f32, f: &Features) {
        let (cols, rows) = (self.cols, self.rows);
        let t = self.time;
        let gravity = scale * 2.2;
        let wind = scale * (0.5 + 2.2 * self.gust);
        for (i, pt) in self.pts.iter_mut().enumerate() {
            if i < cols {
                continue;
            }
            let (u, v) = ((i % cols) as f32 / cols as f32, (i / cols) as f32 / rows as f32);
            // Gusts travel across the cloth as waves.
            let wave = (u * 5.0 - t * 2.3).sin() * 0.6 + (v * 3.0 + u * 2.0 - t * 1.7).cos() * 0.4;
            let ax = wind * (0.25 + 0.35 * wave) * (0.6 + 0.4 * (t * 0.37).sin());
            let az = wind * (0.2 + wave) * 0.9 + scale * 1.2 * f.pulse * wave;
            let mut next = [0.0; 3];
            let acc = [ax, gravity, az];
            for k in 0..3 {
                let vel = (pt.p[k] - pt.old[k]) * 0.985;
                next[k] = pt.p[k] + vel + acc[k] * h * h;
            }
            pt.old = pt.p;
            pt.p = next;
        }
        let (rx, ry) = self.rest;
        let diag = (rx * rx + ry * ry).sqrt();
        for _ in 0..ITERATIONS {
            for r in 0..rows {
                for col in 0..cols {
                    let i = r * cols + col;
                    if col + 1 < cols {
                        self.satisfy(i, i + 1, rx);
                    }
                    if r + 1 < rows {
                        self.satisfy(i, i + cols, ry);
                        if col + 1 < cols {
                            self.satisfy(i, i + cols + 1, diag);
                        }
                        if col > 0 {
                            self.satisfy(i, i + cols - 1, diag);
                        }
                    }
                }
            }
        }
    }

    /// Moves two points towards their rest distance; the top row is pinned.
    fn satisfy(&mut self, a: usize, b: usize, rest: f32) {
        let (pa, pb) = (self.pts[a].p, self.pts[b].p);
        let d = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len < 1e-6 {
            return;
        }
        let diff = (len - rest) / len;
        let (wa, wb) = match (a < self.cols, b < self.cols) {
            (true, true) => return,
            (true, false) => (0.0, 1.0),
            (false, true) => (1.0, 0.0),
            (false, false) => (0.5, 0.5),
        };
        for (k, dk) in d.iter().enumerate() {
            self.pts[a].p[k] += dk * diff * wa;
            self.pts[b].p[k] -= dk * diff * wb;
        }
    }
}

impl Effect for Cloth {
    fn name(&self) -> &'static str {
        "Cloth"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        if self.size != (c.w, c.h) {
            self.reset(c.w, c.h);
        }
        let dt = dt.clamp(0.0, 0.1);
        let target = f.bass * 0.8 + f.pulse * 0.4;
        let rate = if target > self.gust { 6.0 } else { 0.8 };
        self.gust += (target - self.gust) * (1.0 - (-rate * dt).exp());
        self.hue += dt * 0.01;

        let scale = c.h as f32;
        let steps = ((dt / STEP).ceil() as usize).min(MAX_STEPS);
        let h = if steps > 0 { dt / steps as f32 } else { 0.0 };
        for _ in 0..steps {
            self.time += h;
            self.step(h, scale, f);
        }

        // Vertex normals and stretch from the neighbouring grid points.
        let (cols, rows) = (self.cols, self.rows);
        let lv = {
            let l = [-0.5f32, -0.6, 0.65];
            let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
            [l[0] / n, l[1] / n, l[2] / n]
        };
        for r in 0..rows {
            for col in 0..cols {
                let i = r * cols + col;
                let (l, rt) = (r * cols + col.saturating_sub(1), r * cols + (col + 1).min(cols - 1));
                let (u, d) = (r.saturating_sub(1) * cols + col, (r + 1).min(rows - 1) * cols + col);
                let (pl, pr, pu, pd) = (self.pts[l].p, self.pts[rt].p, self.pts[u].p, self.pts[d].p);
                let tx = [pr[0] - pl[0], pr[1] - pl[1], pr[2] - pl[2]];
                let ty = [pd[0] - pu[0], pd[1] - pu[1], pd[2] - pu[2]];
                // Cross product ty × tx points towards the viewer for a flat cloth.
                let n = [
                    ty[1] * tx[2] - ty[2] * tx[1],
                    ty[2] * tx[0] - ty[0] * tx[2],
                    ty[0] * tx[1] - ty[1] * tx[0],
                ];
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
                let ndl = (n[0] * lv[0] + n[1] * lv[1] + n[2] * lv[2]) / len;
                self.light[i] = 0.18 + 0.82 * ndl.abs();
                let span_x = (tx[0] * tx[0] + tx[1] * tx[1] + tx[2] * tx[2]).sqrt();
                let span_y = (ty[0] * ty[0] + ty[1] * ty[1] + ty[2] * ty[2]).sqrt();
                let rest_x = self.rest.0 * (rt - l) as f32;
                let rest_y = self.rest.1 * ((d - u) / cols) as f32;
                let sx = if rest_x > 0.0 { span_x / rest_x - 1.0 } else { 0.0 };
                let sy = if rest_y > 0.0 { span_y / rest_y - 1.0 } else { 0.0 };
                self.stretch[i] = ((sx.max(0.0) + sy.max(0.0)) * 12.0).min(1.0);
            }
        }

        // Dark backdrop with a soft vertical gradient.
        for y in 0..c.h {
            let k = y as f32 / c.h as f32;
            let col = rgb(0.03 + 0.03 * k, 0.03 + 0.02 * k, 0.06 + 0.04 * k + 0.05 * f.level);
            c.px[y * c.w..(y + 1) * c.w].fill(col);
        }
        self.zbuf.fill(f32::NEG_INFINITY);

        for r in 0..rows - 1 {
            for col in 0..cols - 1 {
                let i = r * cols + col;
                let quad = [i, i + 1, i + cols + 1, i + cols];
                // Stripes along the columns.
                let stripe = if (col / 2) % 2 == 0 { 0.0 } else { 0.08 };
                for tri in [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]] {
                    self.raster(c, tri, stripe, f);
                }
            }
        }
    }
}

impl Cloth {
    /// Gouraud-shaded triangle with a depth test (larger z is closer).
    fn raster(&mut self, c: &mut Canvas, tri: [usize; 3], stripe: f32, f: &Features) {
        let p = tri.map(|i| self.pts[i].p);
        let lit = tri.map(|i| self.light[i]);
        let st = tri.map(|i| self.stretch[i]);
        let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        if area.abs() < 1e-6 {
            return;
        }
        let minx = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let maxx = p
            .iter()
            .map(|q| q[0])
            .fold(f32::MIN, f32::max)
            .ceil()
            .min(c.w as f32 - 1.0);
        let miny = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let maxy = p
            .iter()
            .map(|q| q[1])
            .fold(f32::MIN, f32::max)
            .ceil()
            .min(c.h as f32 - 1.0);
        if maxx < 0.0 || maxy < 0.0 {
            return;
        }
        let (maxx, maxy) = (maxx as usize, maxy as usize);
        let inv = 1.0 / area;
        let base_hue = self.hue + stripe;
        for y in miny..=maxy {
            let py = y as f32 + 0.5;
            for x in minx..=maxx {
                let px = x as f32 + 0.5;
                let w0 = ((p[1][0] - px) * (p[2][1] - py) - (p[2][0] - px) * (p[1][1] - py)) * inv;
                let w1 = ((p[2][0] - px) * (p[0][1] - py) - (p[0][0] - px) * (p[2][1] - py)) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    continue;
                }
                let z = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
                let idx = y * c.w + x;
                if z <= self.zbuf[idx] {
                    continue;
                }
                self.zbuf[idx] = z;
                let l = w0 * lit[0] + w1 * lit[1] + w2 * lit[2];
                let s = w0 * st[0] + w1 * st[1] + w2 * st[2];
                let hue = base_hue - 0.55 * s;
                let v = (l * (0.85 + 0.25 * f.pulse) + 0.5 * s).min(1.0);
                c.set(x, y, hsv(hue, 0.7 - 0.3 * s, v));
            }
        }
    }
}
