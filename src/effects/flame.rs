use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};
use std::f32::consts::TAU;

const XFORMS: usize = 4;
const VARIATIONS: usize = 6;
const POINTS: usize = 24_000;
const PALETTE: usize = 64;

/// One affine map followed by a weighted blend of nonlinear variations.
#[derive(Clone, Copy)]
struct Xform {
    m: [f32; 6],
    var: [f32; VARIATIONS],
    color: f32,
}

impl Xform {
    fn random(rng: &mut Rng, i: usize) -> Self {
        // Rotation with per-axis scale below 1, so the linear part contracts.
        let (s, c) = rng.range(0.0, TAU).sin_cos();
        let (sx, sy) = (
            rng.range(0.35, 0.95),
            rng.range(0.35, 0.95) * if rng.f32() < 0.3 { -1.0 } else { 1.0 },
        );
        let m = [
            sx * c,
            -sy * s,
            rng.range(-0.8, 0.8),
            sx * s,
            sy * c,
            rng.range(-0.8, 0.8),
        ];
        // Two variations per map keep the shapes recognizable.
        let mut var = [0.0; VARIATIONS];
        let a = (rng.next_u32() as usize) % VARIATIONS;
        let b = (rng.next_u32() as usize) % VARIATIONS;
        let k = rng.range(0.3, 0.8);
        var[a] += k;
        var[b] += 1.0 - k;
        Self {
            m,
            var,
            color: (i as f32 + rng.f32()) / XFORMS as f32,
        }
    }

    fn lerp(&self, o: &Self, t: f32) -> Self {
        let mut r = *self;
        for (a, b) in r.m.iter_mut().zip(&o.m) {
            *a += (b - *a) * t;
        }
        for (a, b) in r.var.iter_mut().zip(&o.var) {
            *a += (b - *a) * t;
        }
        r.color += (o.color - r.color) * t;
        r
    }

    fn apply(&self, x: f32, y: f32, shift: f32) -> (f32, f32) {
        let m = &self.m;
        let ax = m[0] * x + m[1] * y + m[2] + shift;
        let ay = m[3] * x + m[4] * y + m[5];
        let r2 = ax * ax + ay * ay + 1e-6;
        let r = r2.sqrt();
        let (mut ox, mut oy) = (0.0, 0.0);
        let v = &self.var;
        if v[0] > 0.0 {
            ox += v[0] * ax;
            oy += v[0] * ay;
        }
        if v[1] > 0.0 {
            ox += v[1] * ax.sin();
            oy += v[1] * ay.sin();
        }
        if v[2] > 0.0 {
            ox += v[2] * ax / r2;
            oy += v[2] * ay / r2;
        }
        if v[3] > 0.0 {
            let (s, c) = r2.sin_cos();
            ox += v[3] * (ax * s - ay * c);
            oy += v[3] * (ax * c + ay * s);
        }
        if v[4] > 0.0 {
            ox += v[4] * (ax - ay) * (ax + ay) / r;
            oy += v[4] * 2.0 * ax * ay / r;
        }
        if v[5] > 0.0 {
            ox += v[5] * ay.atan2(ax) / std::f32::consts::PI;
            oy += v[5] * (r - 1.0);
        }
        (ox, oy)
    }
}

/// Fractal flame in the style of Electric Sheep: a chaos game over four
/// morphing nonlinear maps with rotational symmetry, drawn as a log-density
/// histogram that decays across frames. Each map is nudged by its own
/// spectrum band, mids speed up the morph, beats flash and shift the hues.
pub struct FractalFlame {
    from: [Xform; XFORMS],
    to: [Xform; XFORMS],
    sym: usize,
    morph: f32,
    rng: Rng,
    pt: (f32, f32, f32),
    rot: f32,
    hue: f32,
    /// Per pixel: summed r, g, b and hit density.
    hist: Vec<[f32; 4]>,
    size: (usize, usize),
}

impl FractalFlame {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xF1A3E);
        let from = std::array::from_fn(|i| Xform::random(&mut rng, i));
        let to = std::array::from_fn(|i| Xform::random(&mut rng, i));
        Self {
            from,
            to,
            sym: 3,
            morph: 0.0,
            rng,
            pt: (0.1, 0.1, 0.0),
            rot: 0.0,
            hue: 0.0,
            hist: Vec::new(),
            size: (0, 0),
        }
    }
}

impl Effect for FractalFlame {
    fn name(&self) -> &'static str {
        "Fractal Flame"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.hist = vec![[0.0; 4]; c.w * c.h];
        }
        self.morph += dt / 14.0 * (0.5 + f.mid * 1.5);
        if self.morph >= 1.0 {
            self.morph = 0.0;
            self.from = self.to;
            for (i, x) in self.to.iter_mut().enumerate() {
                *x = Xform::random(&mut self.rng, i);
            }
            self.sym = 1 + (self.rng.next_u32() % 5) as usize;
        }
        self.rot += dt * (0.08 + 0.2 * f.bass);
        self.hue += dt * 0.015 + if f.beat { 0.07 } else { 0.0 };

        let m = self.morph * self.morph * (3.0 - 2.0 * self.morph);
        let xf: [Xform; XFORMS] = std::array::from_fn(|i| self.from[i].lerp(&self.to[i], m));
        let bins = f.spectrum.len();
        let shifts: [f32; XFORMS] = std::array::from_fn(|i| {
            if bins == 0 {
                return 0.0;
            }
            let band = &f.spectrum[i * bins / XFORMS..((i + 1) * bins / XFORMS).max(i * bins / XFORMS + 1)];
            band.iter().sum::<f32>() / band.len() as f32 * 0.35
        });
        let palette: [[f32; 3]; PALETTE] = std::array::from_fn(|i| {
            let p = hsv(self.hue + i as f32 / PALETTE as f32 * 0.55, 0.75, 1.0);
            [p[0] as f32, p[1] as f32, p[2] as f32]
        });

        let (w, h) = (c.w, c.h);
        let keep = 0.86f32.powf(dt * 30.0);
        for p in &mut self.hist {
            for v in p.iter_mut() {
                *v *= keep;
            }
        }

        let scale = w.min(h) as f32 * 0.32 * (1.0 + 0.08 * f.pulse);
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let (rs, rc) = self.rot.sin_cos();
        let (ss, sc) = (TAU / self.sym as f32).sin_cos();
        let points = POINTS.min(w * h * 3 + 50);
        let (mut x, mut y, mut col) = self.pt;
        for _ in 0..points {
            let r = self.rng.next_u32();
            let i = (r as usize >> 8) % XFORMS;
            (x, y) = xf[i].apply(x, y, shifts[i]);
            col = (col + xf[i].color) * 0.5;
            // Symmetry: sometimes rotate by a fraction of a turn.
            if self.sym > 1 && r & 3 == 0 {
                (x, y) = (x * sc - y * ss, x * ss + y * sc);
            }
            if !x.is_finite() || !y.is_finite() || x.abs() > 1e4 || y.abs() > 1e4 {
                (x, y) = (self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0));
                continue;
            }
            let px = cx + (x * rc - y * rs) * scale;
            let py = cy + (x * rs + y * rc) * scale;
            if px >= 0.0 && py >= 0.0 && (px as usize) < w && (py as usize) < h {
                let cell = &mut self.hist[py as usize * w + px as usize];
                let p = &palette[((col * PALETTE as f32) as usize).min(PALETTE - 1)];
                cell[0] += p[0];
                cell[1] += p[1];
                cell[2] += p[2];
                cell[3] += 1.0;
            }
        }
        self.pt = (x, y, col);

        let peak = self.hist.iter().fold(1.0f32, |m, p| m.max(p[3]));
        let norm = 1.0 / (1.0 + peak).ln();
        let bright = 1.1 + 0.5 * f.pulse;
        for (px, p) in c.px.iter_mut().zip(&self.hist) {
            if p[3] < 0.05 {
                *px = [0; 3];
                continue;
            }
            let a = ((1.0 + p[3]).ln() * norm).powf(0.45) * bright / p[3];
            *px = [
                (p[0] * a).min(255.0) as u8,
                (p[1] * a).min(255.0) as u8,
                (p[2] * a).min(255.0) as u8,
            ];
        }
    }
}
