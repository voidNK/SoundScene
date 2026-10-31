use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, rgb};
use std::f32::consts::TAU;

/// Neon tube colors as linear RGB.
const PALETTE: [[f32; 3]; 6] = [
    [1.0, 0.15, 0.55],
    [0.1, 0.85, 1.0],
    [0.45, 1.0, 0.2],
    [1.0, 0.5, 0.05],
    [0.6, 0.25, 1.0],
    [1.0, 0.9, 0.2],
];

/// Unit-size outlines, centered on the origin.
fn outline(kind: usize) -> Vec<(f32, f32)> {
    match kind % 6 {
        0 => (0..=24).map(|i| (i as f32 / 24.0 * TAU).sin_cos()).collect(),
        1 => (0..=3)
            .map(|i| {
                let a = i as f32 / 3.0 * TAU;
                (a.sin(), -a.cos())
            })
            .collect(),
        2 => vec![(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0)],
        3 => (0..=8)
            .map(|i| (i as f32 / 4.0 - 1.0, if i % 2 == 0 { -0.4 } else { 0.4 }))
            .collect(),
        4 => (0..=20)
            .map(|i| {
                let u = i as f32 / 20.0;
                (u * 2.0 - 1.0, (u * TAU * 1.5).sin() * 0.45)
            })
            .collect(),
        _ => (0..=28)
            .map(|i| {
                let u = i as f32 / 28.0;
                let (s, c) = (u * TAU * 1.6).sin_cos();
                (s * (0.15 + 0.85 * u), c * (0.15 + 0.85 * u))
            })
            .collect(),
    }
}

struct Tube {
    kind: usize,
    points: Vec<(f32, f32)>,
    color: usize,
    spin: f32,
    angle: f32,
    /// Remaining seconds of a flicker-out.
    dark: f32,
}

/// Glowing neon tubes bent into abstract shapes, with hot white cores and
/// bloom halos. Bass widens the halos, each tube follows its own band,
/// treble makes them flicker and beats switch the colors.
pub struct Neon {
    tubes: Vec<Tube>,
    acc: Vec<[f32; 3]>,
    dist: Vec<f32>,
    rng: Rng,
    time: f32,
    swaps: f32,
}

impl Neon {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x9E04);
        let tubes = (0..4)
            .map(|i| Tube {
                kind: i * 2 % 6,
                points: outline(i * 2 % 6),
                color: i,
                spin: rng.range(-0.4, 0.4),
                angle: rng.range(0.0, TAU),
                dark: 0.0,
            })
            .collect();
        Self {
            tubes,
            acc: Vec::new(),
            dist: Vec::new(),
            rng,
            time: 0.0,
            swaps: 0.0,
        }
    }
}

impl Effect for Neon {
    fn name(&self) -> &'static str {
        "Neon"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (w, h) = (c.w, c.h);
        if self.acc.len() != w * h {
            self.acc = vec![[0.0; 3]; w * h];
            self.dist = vec![0.0; w * h];
        }
        self.time += dt;

        if f.beat {
            self.swaps += 1.0;
            let shift = 1 + self.rng.next_u32() as usize % 5;
            for t in &mut self.tubes {
                t.color = (t.color + shift) % PALETTE.len();
            }
            // Every few beats one tube gets bent into a new shape.
            if self.swaps >= 6.0 {
                self.swaps = 0.0;
                let i = self.rng.next_u32() as usize % self.tubes.len();
                let t = &mut self.tubes[i];
                t.kind = (t.kind + 1 + self.rng.next_u32() as usize % 5) % 6;
                t.points = outline(t.kind);
            }
        }
        let flicker = dt * (0.15 + 2.5 * f.treble);
        for t in &mut self.tubes {
            t.angle += dt * t.spin * (1.0 + f.mid);
            t.dark = (t.dark - dt).max(0.0);
            if self.rng.f32() < flicker * 0.25 {
                t.dark = self.rng.range(0.03, 0.15);
            }
        }

        // Faint violet wall.
        for (i, a) in self.acc.iter_mut().enumerate() {
            let y = (i / w) as f32 / h as f32;
            *a = [0.03 + 0.02 * y, 0.01, 0.05 + 0.02 * y];
        }

        let (wf, hf) = (w as f32, h as f32);
        let s = wf.min(hf);
        let core = (s * 0.012).max(0.55);
        let halo = s * (0.05 + 0.05 * f.bass + 0.02 * f.pulse);
        let reach = halo * 2.6;
        let bands = [f.bass, (f.bass + f.mid) * 0.5, f.mid, f.treble];
        let slots = [(-0.26, -0.22), (0.26, -0.22), (-0.26, 0.24), (0.26, 0.24)];
        for (n, t) in self.tubes.iter().enumerate() {
            if t.dark > 0.0 {
                continue;
            }
            let (sx, sy) = slots[n % 4];
            let cx = wf * (0.5 + sx) + (self.time * 0.31 + n as f32 * 1.7).sin() * wf * 0.04;
            let cy = hf * (0.5 + sy) + (self.time * 0.23 + n as f32 * 2.3).cos() * hf * 0.04;
            let size = s * (0.17 + 0.02 * (self.time * 0.9 + n as f32).sin()) * (1.0 + 0.12 * f.pulse);
            let (sa, ca) = t.angle.sin_cos();
            let pts: Vec<(f32, f32)> = t
                .points
                .iter()
                .map(|&(x, y)| (cx + (x * ca - y * sa) * size, cy + (x * sa + y * ca) * size))
                .collect();

            // Distance field of the tube within its bounding box.
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for &(x, y) in &pts {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
            let bx0 = (x0 - reach).max(0.0) as usize;
            let by0 = (y0 - reach).max(0.0) as usize;
            let bx1 = ((x1 + reach).max(0.0) as usize).min(w);
            let by1 = ((y1 + reach).max(0.0) as usize).min(h);
            if bx0 >= bx1 || by0 >= by1 {
                continue;
            }
            for y in by0..by1 {
                self.dist[y * w + bx0..y * w + bx1].fill(f32::MAX);
            }
            for seg in pts.windows(2) {
                let ((ax, ay), (bx, by)) = (seg[0], seg[1]);
                let (dx, dy) = (bx - ax, by - ay);
                let len2 = (dx * dx + dy * dy).max(1e-6);
                let sx0 = ((ax.min(bx) - reach).max(0.0) as usize).max(bx0);
                let sy0 = ((ay.min(by) - reach).max(0.0) as usize).max(by0);
                let sx1 = ((ax.max(bx) + reach).max(0.0) as usize).min(bx1);
                let sy1 = ((ay.max(by) + reach).max(0.0) as usize).min(by1);
                for y in sy0..sy1 {
                    let py = y as f32 + 0.5 - ay;
                    for x in sx0..sx1 {
                        let px = x as f32 + 0.5 - ax;
                        let k = ((px * dx + py * dy) / len2).clamp(0.0, 1.0);
                        let d2 = (px - dx * k).powi(2) + (py - dy * k).powi(2);
                        let p = &mut self.dist[y * w + x];
                        *p = p.min(d2);
                    }
                }
            }

            let col = PALETTE[t.color % PALETTE.len()];
            let hum = 0.92 + 0.08 * (self.time * 50.0 + n as f32).sin();
            let bright = (0.55 + 0.6 * bands[n % 4] + 0.35 * f.pulse) * hum;
            let inv_halo = 1.0 / (halo * halo).max(1e-3);
            for y in by0..by1 {
                for x in bx0..bx1 {
                    let i = y * w + x;
                    let d2 = self.dist[i];
                    if d2 == f32::MAX {
                        continue;
                    }
                    let d = d2.sqrt();
                    let hot = (core + 0.6 - d).clamp(0.0, 1.0);
                    let glow = (-d2 * inv_halo).exp() * 0.7 + (1.0 - d / reach).max(0.0).powi(2) * 0.2;
                    let a = &mut self.acc[i];
                    for ch in 0..3 {
                        a[ch] += (col[ch] * (glow + hot * 1.2) + hot * 0.9) * bright;
                    }
                }
            }
        }

        for (p, a) in c.px.iter_mut().zip(&self.acc) {
            *p = rgb(
                1.0 - (-a[0] * 1.3).exp(),
                1.0 - (-a[1] * 1.3).exp(),
                1.0 - (-a[2] * 1.3).exp(),
            );
        }
    }
}
