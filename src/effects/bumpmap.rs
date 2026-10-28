use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, rgb};

/// Classic 2D bump mapping: two lights glide over an embossed brick relief
/// with domes, rings, pyramids and grooves. Beats throw the white light to a
/// new spot and flash it, the relief depth breathes with the level and bass
/// feeds the colored light.
pub struct BumpMap {
    w: usize,
    h: usize,
    height: Vec<f32>,
    tint: Vec<u8>,
    rng: Rng,
    phase: f32,
    jump: (f32, f32),
    hue: f32,
}

const TINTS: usize = 6;

fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

fn floats(c: [u8; 3]) -> [f32; 3] {
    [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]
}

impl BumpMap {
    pub fn new() -> Self {
        Self {
            w: 0,
            h: 0,
            height: Vec::new(),
            tint: Vec::new(),
            rng: Rng::new(0xB0B5),
            phase: 0.0,
            jump: (0.0, 0.0),
            hue: 0.0,
        }
    }

    /// Precomputes the height field (0..1, roughly) and a per-tile tint index.
    fn build(&mut self, w: usize, h: usize) {
        (self.w, self.h) = (w, h);
        self.height = vec![0.0; w * h];
        self.tint = vec![0; w * h];
        let s = (w.min(h) as f32 / 4.5).max(4.0);
        for y in 0..h {
            let fy = y as f32;
            let row = (fy / s).floor();
            let shift = if (row as i32) & 1 == 1 { s * 0.5 } else { 0.0 };
            for x in 0..w {
                let xo = x as f32 + shift;
                let col = (xo / s).floor();
                let (u, v) = (xo / s - col, fy / s - row);
                let edge = u.min(1.0 - u).min(v).min(1.0 - v);
                let bevel = (edge * 7.0).min(1.0);
                let id = hash(col as i32, row as i32);
                let (du, dv) = (u - 0.5, v - 0.5);
                let d = (du * du + dv * dv).sqrt();
                let motif = match id % 4 {
                    0 => (1.0 - d * 2.4).max(0.0).sqrt(),
                    1 => ((d * 20.0).cos() * 0.5 + 0.5) * (1.0 - d * 2.2).max(0.0),
                    2 => (0.32 - du.abs().max(dv.abs())).max(0.0) * 3.0,
                    _ => -(1.0 - du.abs().min(dv.abs()) * 14.0).max(0.0) * 0.6 * (d < 0.36) as u8 as f32,
                };
                let fx = x as f32;
                let grain = (fx * 0.9).sin() * (fy * 1.3 + (fx * 0.31).sin() * 2.0).sin() * 0.03;
                let i = y * w + x;
                self.height[i] = bevel * 0.55 + motif * 0.4 * bevel + grain;
                self.tint[i] = ((id >> 8) % TINTS as u32) as u8;
            }
        }
    }
}

impl Effect for BumpMap {
    fn name(&self) -> &'static str {
        "Bump Mapping"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if (c.w, c.h) != (self.w, self.h) {
            self.build(c.w, c.h);
        }
        let (cw, ch) = (c.w, c.h);
        if cw == 0 || ch == 0 {
            return;
        }
        let (w, h) = (cw as f32, ch as f32);
        self.phase += dt * (0.4 + 0.8 * f.mid);
        self.hue += dt * 0.02;
        let k = (-3.0 * dt).exp();
        self.jump = (self.jump.0 * k, self.jump.1 * k);
        if f.beat {
            self.jump = (self.rng.range(-0.35, 0.35) * w, self.rng.range(-0.35, 0.35) * h);
        }

        let m = w.min(h);
        let l1 = (
            w * 0.5 + w * 0.32 * (self.phase * 1.1).cos() + self.jump.0,
            h * 0.5 + h * 0.3 * (self.phase * 1.7).sin() + self.jump.1,
        );
        let l2 = (
            w * 0.5 + w * 0.35 * (self.phase * 0.7 + 2.0).sin(),
            h * 0.5 + h * 0.32 * (self.phase * 1.3 + 1.0).cos(),
        );
        let lz = (m * 0.3).max(1.0);
        let depth = (m / 4.5).max(4.0) * (0.3 + 1.1 * f.level);
        let inv_r2 = 2.0 / (m * 0.6).max(1.0).powi(2);
        let i1 = 0.9 + 0.9 * f.pulse;
        let c1 = [1.5 * i1, 1.38 * i1, 1.2 * i1];
        let k2 = 0.45 + 0.7 * f.bass;
        let c2 = floats(hsv(self.hue + 0.55, 0.85, 1.0)).map(|v| v * k2);
        let albedo: Vec<[f32; 3]> = (0..TINTS)
            .map(|i| floats(hsv(self.hue + i as f32 * 0.07, 0.5, 1.0)))
            .collect();

        for y in 0..ch {
            let (yu, yd) = (y.saturating_sub(1), (y + 1).min(ch - 1));
            let fy = y as f32;
            for x in 0..cw {
                let (xl, xr) = (x.saturating_sub(1), (x + 1).min(cw - 1));
                let i = y * cw + x;
                let gx = (self.height[y * cw + xr] - self.height[y * cw + xl]) * 0.5 * depth;
                let gy = (self.height[yd * cw + x] - self.height[yu * cw + x]) * 0.5 * depth;
                let inv = 1.0 / (gx * gx + gy * gy + 1.0).sqrt();
                let n = [-gx * inv, -gy * inv, inv];
                let fx = x as f32;

                let light = |l: (f32, f32)| {
                    let (lx, ly) = (l.0 - fx, l.1 - fy);
                    let d2 = lx * lx + ly * ly;
                    let invd = 1.0 / (d2 + lz * lz).sqrt();
                    let ld = [lx * invd, ly * invd, lz * invd];
                    let diff = (n[0] * ld[0] + n[1] * ld[1] + n[2] * ld[2]).max(0.0);
                    let hv = [ld[0], ld[1], ld[2] + 1.0];
                    let hl = (hv[0] * hv[0] + hv[1] * hv[1] + hv[2] * hv[2]).sqrt().max(1e-6);
                    let spec = ((n[0] * hv[0] + n[1] * hv[1] + n[2] * hv[2]) / hl).max(0.0).powi(32);
                    let att = 1.0 / (1.0 + d2 * inv_r2);
                    (diff * att, spec * att)
                };
                let (d1, s1) = light(l1);
                let (d2, s2) = light(l2);
                let a = albedo[self.tint[i] as usize % TINTS];
                let shade = 0.55 + 0.45 * self.height[i].clamp(0.0, 1.0);
                let chan = |j: usize| {
                    a[j] * shade * (0.06 + d1 * c1[j] + d2 * c2[j] * 1.4) + s1 * c1[j] * 0.8 + s2 * c2[j] * 0.6
                };
                c.set(x, y, rgb(chan(0), chan(1), chan(2)));
            }
        }
    }
}
