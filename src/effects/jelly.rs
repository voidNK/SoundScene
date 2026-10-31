use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, rgb};
use std::f32::consts::TAU;

const BLOBS: usize = 3;
const POINTS: usize = 20;
const STEP: f32 = 1.0 / 300.0;
const MAX_STEPS: usize = 30;
const FLOOR: f32 = 0.94;

struct Blob {
    x: Vec<[f32; 2]>,
    v: Vec<[f32; 2]>,
    radius: f32,
    hue: f32,
}

impl Blob {
    fn center(&self) -> [f32; 2] {
        let n = self.x.len().max(1) as f32;
        let s = self.x.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0], a[1] + p[1]]);
        [s[0] / n, s[1] / n]
    }

    /// Polygon area via the shoelace formula.
    fn area(&self) -> f32 {
        let n = self.x.len();
        let mut a = 0.0;
        for i in 0..n {
            let (p, q) = (self.x[i], self.x[(i + 1) % n]);
            a += p[0] * q[1] - q[0] * p[1];
        }
        (a * 0.5).abs()
    }
}

/// Soft-body jelly blobs: rings of point masses held together by springs and
/// inflated by gas pressure, bouncing on the floor and against each other.
/// Bass softens and squashes them, beats make a random blob jump.
pub struct Jelly {
    blobs: Vec<Blob>,
    aspect: f32,
    rng: Rng,
    hue: f32,
}

impl Jelly {
    pub fn new() -> Self {
        Self {
            blobs: Vec::new(),
            aspect: 0.0,
            rng: Rng::new(0x7E11),
            hue: 0.0,
        }
    }

    fn reset(&mut self, aspect: f32) {
        self.aspect = aspect;
        self.blobs.clear();
        let radius = (aspect / (BLOBS as f32 * 2.6)).min(0.2);
        for i in 0..BLOBS {
            let cx = aspect * (i as f32 + 0.5) / BLOBS as f32;
            let cy = 0.3 + 0.15 * i as f32;
            let x = (0..POINTS)
                .map(|k| {
                    let a = k as f32 / POINTS as f32 * TAU;
                    [cx + a.cos() * radius, cy + a.sin() * radius]
                })
                .collect();
            self.blobs.push(Blob {
                x,
                v: vec![[0.0; 2]; POINTS],
                radius,
                hue: i as f32 / BLOBS as f32,
            });
        }
    }

    fn step(&mut self, h: f32, softness: f32, gravity: f32) {
        let aspect = self.aspect;
        for b in &mut self.blobs {
            let r = b.radius;
            let edge = TAU * r / POINTS as f32;
            let rest_area = std::f32::consts::PI * r * r;
            let pressure = 260.0 * (1.0 - 0.6 * softness) * (rest_area / b.area().max(1e-6) - 1.0);
            let mut force = [[0.0, gravity]; POINTS];
            let c = b.center();
            for i in 0..POINTS {
                let j = (i + 1) % POINTS;
                let (p, q) = (b.x[i], b.x[j]);
                let d = [q[0] - p[0], q[1] - p[1]];
                let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-6);
                let dir = [d[0] / len, d[1] / len];
                // Edge spring with damping along the edge.
                let rel = [b.v[j][0] - b.v[i][0], b.v[j][1] - b.v[i][1]];
                let fs = 900.0 * (len - edge) + 6.0 * (rel[0] * dir[0] + rel[1] * dir[1]);
                // Pressure pushes along the outward normal, scaled by edge length.
                let mut n = [dir[1], -dir[0]];
                let mid = [(p[0] + q[0]) * 0.5 - c[0], (p[1] + q[1]) * 0.5 - c[1]];
                if n[0] * mid[0] + n[1] * mid[1] < 0.0 {
                    n = [-n[0], -n[1]];
                }
                let fp = pressure * len;
                for k in 0..2 {
                    force[i][k] += dir[k] * fs + n[k] * fp * 0.5;
                    force[j][k] += -dir[k] * fs + n[k] * fp * 0.5;
                }
                // Springs to the second neighbour resist sharp bends.
                let k2 = (i + 2) % POINTS;
                let d2 = [b.x[k2][0] - p[0], b.x[k2][1] - p[1]];
                let len2 = (d2[0] * d2[0] + d2[1] * d2[1]).sqrt().max(1e-6);
                let fb = 300.0 * (len2 - 2.0 * r * (TAU / POINTS as f32).sin());
                for k in 0..2 {
                    force[i][k] += d2[k] / len2 * fb;
                    force[k2][k] -= d2[k] / len2 * fb;
                }
                // A spring to the center keeps the shape from folding.
                let to_c = [c[0] - p[0], c[1] - p[1]];
                let dc = (to_c[0] * to_c[0] + to_c[1] * to_c[1]).sqrt().max(1e-6);
                let fc = 120.0 * (1.0 - 0.5 * softness) * (dc - r);
                force[i][0] += to_c[0] / dc * fc;
                force[i][1] += to_c[1] / dc * fc;
            }
            for (i, fi) in force.iter().enumerate() {
                for (k, fk) in fi.iter().enumerate() {
                    b.v[i][k] = ((b.v[i][k] + fk * h) * (1.0 - 2.0 * h)).clamp(-20.0, 20.0);
                    b.x[i][k] += b.v[i][k] * h;
                }
                // Floor and walls with friction.
                if b.x[i][1] > FLOOR {
                    b.x[i][1] = FLOOR;
                    b.v[i][1] = -b.v[i][1].max(0.0) * 0.3;
                    b.v[i][0] *= 0.9;
                }
                if b.x[i][1] < 0.02 {
                    b.x[i][1] = 0.02;
                    b.v[i][1] = b.v[i][1].abs() * 0.3;
                }
                if b.x[i][0] < 0.01 {
                    b.x[i][0] = 0.01;
                    b.v[i][0] = b.v[i][0].abs() * 0.4;
                }
                if b.x[i][0] > aspect - 0.01 {
                    b.x[i][0] = aspect - 0.01;
                    b.v[i][0] = -b.v[i][0].abs() * 0.4;
                }
            }
        }
        // Points pushed out of other blobs' cores.
        let centers: Vec<([f32; 2], f32)> = self.blobs.iter().map(|b| (b.center(), b.radius)).collect();
        for (bi, b) in self.blobs.iter_mut().enumerate() {
            for (ci, &(c, r)) in centers.iter().enumerate() {
                if ci == bi {
                    continue;
                }
                for i in 0..POINTS {
                    let d = [b.x[i][0] - c[0], b.x[i][1] - c[1]];
                    let len = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-6);
                    let reach = r * 0.95;
                    if len < reach {
                        let k = (reach - len) / len;
                        b.x[i][0] += d[0] * k * 0.5;
                        b.x[i][1] += d[1] * k * 0.5;
                        b.v[i][0] += d[0] / len * 0.5;
                        b.v[i][1] += d[1] / len * 0.5;
                    }
                }
            }
        }
    }
}

impl Effect for Jelly {
    fn name(&self) -> &'static str {
        "Jelly"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        // World units: height 1, width = aspect ratio.
        let unit = c.h as f32;
        let aspect = c.w as f32 / unit;
        if (aspect - self.aspect).abs() > 1e-3 || self.blobs.is_empty() {
            self.reset(aspect);
        }
        self.hue += dt * 0.015;

        if f.beat {
            self.hue += 0.03;
            let i = self.rng.next_u32() as usize % self.blobs.len();
            let jump = -(1.2 + 1.3 * f.bass);
            let side = self.rng.range(-0.5, 0.5);
            for v in &mut self.blobs[i].v {
                v[1] += jump;
                v[0] += side;
            }
        }
        // Gentle hops in silence.
        if self.rng.f32() < dt * 0.25 {
            let i = self.rng.next_u32() as usize % self.blobs.len();
            let side = self.rng.range(-0.4, 0.4);
            for v in &mut self.blobs[i].v {
                v[1] -= 1.0;
                v[0] += side;
            }
        }

        let softness = f.bass.max(f.pulse * 0.8);
        let gravity = 2.0 * (1.0 + 0.8 * f.bass);
        let steps = ((dt / STEP).ceil() as usize).min(MAX_STEPS);
        let h = if steps > 0 { dt / steps as f32 } else { 0.0 };
        for _ in 0..steps {
            self.step(h, softness, gravity);
        }

        // Background with a floor line.
        let floor_y = (FLOOR * unit) as usize;
        for y in 0..c.h {
            let col = if y >= floor_y {
                rgb(0.10, 0.08 + 0.05 * f.level, 0.14)
            } else {
                let k = y as f32 / c.h as f32;
                rgb(0.02 + 0.03 * k, 0.03 + 0.03 * k, 0.07 + 0.06 * k)
            };
            c.px[y * c.w..(y + 1) * c.w].fill(col);
        }

        // Scanline polygon fill, shaded from the core to a bright rim.
        let mut xs: Vec<f32> = Vec::with_capacity(POINTS);
        for b in &self.blobs {
            let pts: Vec<[f32; 2]> = b.x.iter().map(|p| [p[0] * unit, p[1] * unit]).collect();
            let cen = b.center();
            let (ccx, ccy) = (cen[0] * unit, cen[1] * unit);
            let r = (b.radius * unit).max(1.0);
            let base = hsv(self.hue + b.hue, 0.75, 0.75 + 0.25 * f.pulse);
            let core = hsv(self.hue + b.hue + 0.05, 0.9, 0.35);
            let ymin = pts.iter().map(|p| p[1]).fold(f32::MAX, f32::min).max(0.0) as usize;
            let ymax = (pts.iter().map(|p| p[1]).fold(f32::MIN, f32::max).max(0.0) as usize).min(c.h - 1);
            for y in ymin..=ymax {
                let py = y as f32 + 0.5;
                xs.clear();
                for i in 0..POINTS {
                    let (p, q) = (pts[i], pts[(i + 1) % POINTS]);
                    if (p[1] <= py) != (q[1] <= py) {
                        xs.push(p[0] + (py - p[1]) / (q[1] - p[1]) * (q[0] - p[0]));
                    }
                }
                xs.sort_by(|a, b| a.total_cmp(b));
                for &[a, b] in xs.as_chunks::<2>().0 {
                    let x0 = a.max(0.0).round() as usize;
                    let x1 = (b.max(0.0).round() as usize).min(c.w);
                    for x in x0..x1 {
                        let (dx, dy) = (x as f32 + 0.5 - ccx, py - ccy);
                        let d = ((dx * dx + dy * dy).sqrt() / r).min(1.2);
                        let mut col = mix(core, base, d * d);
                        // Specular highlight towards the upper left.
                        let (hx, hy) = (dx / r + 0.4, dy / r + 0.45);
                        let spec = (1.0 - (hx * hx + hy * hy) * 6.0).max(0.0);
                        col = mix(col, [255, 255, 255], spec * 0.8);
                        c.set(x, y, col);
                    }
                }
            }
        }
    }
}
