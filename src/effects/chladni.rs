use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, rgb};
use std::f32::consts::PI;

const GRAINS: usize = 7000;
/// Plate modes (n, m, sign), roughly ordered from simple to complex.
const MODES: [(f32, f32, f32); 16] = [
    (2.0, 1.0, -1.0),
    (3.0, 1.0, -1.0),
    (3.0, 2.0, 1.0),
    (4.0, 1.0, 1.0),
    (4.0, 3.0, -1.0),
    (5.0, 2.0, -1.0),
    (5.0, 1.0, 1.0),
    (5.0, 3.0, 1.0),
    (5.0, 4.0, -1.0),
    (6.0, 1.0, -1.0),
    (6.0, 5.0, 1.0),
    (7.0, 2.0, -1.0),
    (7.0, 4.0, 1.0),
    (8.0, 3.0, -1.0),
    (8.0, 5.0, 1.0),
    (9.0, 4.0, -1.0),
];

/// Plate displacement and its gradient for mode (n, m, s) at (x, y) in 0..1.
fn field(mode: (f32, f32, f32), x: f32, y: f32) -> (f32, f32, f32) {
    let (n, m, s) = mode;
    let (snx, cnx) = (n * PI * x).sin_cos();
    let (smx, cmx) = (m * PI * x).sin_cos();
    let (sny, cny) = (n * PI * y).sin_cos();
    let (smy, cmy) = (m * PI * y).sin_cos();
    let v = cnx * cmy + s * cmx * cny;
    let gx = -n * PI * snx * cmy - s * m * PI * smx * cny;
    let gy = -m * PI * cnx * smy - s * n * PI * cmx * sny;
    (v, gx, gy)
}

/// Chladni plate: sand grains bounce wherever the plate vibrates and settle
/// on its nodal lines. The dominant frequency picks the mode on beats, so
/// the sand keeps migrating into new figures; loudness shakes it harder.
pub struct Chladni {
    grains: Vec<(f32, f32)>,
    mode: usize,
    since: f32,
    rng: Rng,
    count: Vec<u16>,
    hue: f32,
}

impl Chladni {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xC1AD);
        let grains = (0..GRAINS).map(|_| (rng.f32(), rng.f32())).collect();
        Self {
            grains,
            mode: 0,
            since: 0.0,
            rng,
            count: Vec::new(),
            hue: 0.1,
        }
    }
}

impl Effect for Chladni {
    fn name(&self) -> &'static str {
        "Chladni"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        let dt = dt.min(0.1);
        self.since += dt;
        if (f.beat && self.since > 2.5) || self.since > 9.0 {
            // Spectral centroid picks the neighborhood, randomness the exact mode.
            let total: f32 = f.spectrum.iter().sum();
            let centroid = if total > 0.0 {
                f.spectrum.iter().enumerate().map(|(i, v)| i as f32 * v).sum::<f32>()
                    / total
                    / f.spectrum.len().max(1) as f32
            } else {
                0.3
            };
            let base = (centroid * 1.6 * MODES.len() as f32) as i32;
            let pick = (base + (self.rng.next_u32() % 5) as i32 - 2).clamp(0, MODES.len() as i32 - 1) as usize;
            self.mode = if pick == self.mode {
                (pick + 1) % MODES.len()
            } else {
                pick
            };
            self.since = 0.0;
            self.hue = self.rng.f32();
        }
        let mode = MODES[self.mode];

        // The field is mirror-symmetric, so a wide canvas shows the square
        // plate's figure mirrored side by side instead of stretched.
        let side = c.w.min(c.h).max(1) as f32;
        let (aw, ah) = ((c.w as f32 / side).max(1.0), (c.h as f32 / side).max(1.0));
        let shake = (0.3 + 0.7 * f.level + f.pulse) * 0.025 * (dt * 30.0).sqrt();
        for g in &mut self.grains {
            let (v, gx, gy) = field(mode, g.0 * aw, g.1 * ah);
            // Jump around where the plate moves, slide towards the node line.
            let amp = v.abs() * shake + 0.0008 * f.level;
            g.0 += self.rng.range(-amp, amp) / aw;
            g.1 += self.rng.range(-amp, amp) / ah;
            let g2 = gx * gx + gy * gy;
            if g2 > 1e-3 {
                let k = (v / g2 * (dt * 3.0).min(0.5)).clamp(-0.02, 0.02);
                g.0 -= gx * k / aw;
                g.1 -= gy * k / ah;
            }
            g.0 = g.0.clamp(0.0, 1.0);
            g.1 = g.1.clamp(0.0, 1.0);
        }

        let (w, h) = (c.w, c.h);
        self.count.clear();
        self.count.resize(w * h, 0);
        if w == 0 || h == 0 {
            return;
        }
        for &(x, y) in &self.grains {
            let px = ((x * w as f32) as usize).min(w - 1);
            let py = ((y * h as f32) as usize).min(h - 1);
            let i = py * w + px;
            self.count[i] = self.count[i].saturating_add(1);
        }

        let sand = hsv(self.hue * 0.15 + 0.07, 0.45, 0.75);
        let bright = rgb(1.0, 0.97, 0.88);
        for y in 0..h {
            let fy = (y as f32 + 0.5) / h as f32;
            for x in 0..w {
                let fx = (x as f32 + 0.5) / w as f32;
                let n = self.count[y * w + x];
                let col = if n > 0 {
                    mix(sand, bright, (n as f32 - 1.0) / 4.0)
                } else {
                    // The vibrating metal plate, faintly lit by its own motion.
                    let (v, _, _) = field(mode, fx * aw, fy * ah);
                    let glow = v.abs() * (0.04 + 0.1 * f.level + 0.1 * f.pulse);
                    hsv(self.hue, 0.6, 0.05 + glow)
                };
                c.set(x, y, col);
            }
        }
    }
}
