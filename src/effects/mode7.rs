use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};

fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^ (h >> 12)
}

fn floats(c: [u8; 3]) -> [f32; 3] {
    c.map(|v| v as f32 / 255.0)
}

fn blend(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|j| a[j] + (b[j] - a[j]) * t)
}

/// SNES Mode 7 flight: a perspective-mapped floor of tiles, each lit by one
/// spectrum band, under a mirrored plane of drifting clouds. The camera
/// swerves with the mids, flies faster with the level and hops on beats,
/// which also flash the tile seams.
pub struct Mode7 {
    x: f32,
    z: f32,
    yaw: f32,
    hue: f32,
    flash: f32,
}

impl Mode7 {
    pub fn new() -> Self {
        Self {
            x: 0.0,
            z: 0.0,
            yaw: 0.0,
            hue: 0.55,
            flash: 0.0,
        }
    }
}

impl Effect for Mode7 {
    fn name(&self) -> &'static str {
        "Mode7"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.flash = 1.0;
        }
        self.flash *= (-5.0 * dt).exp();
        self.yaw += dt * (0.35 * (t * 0.13).sin() + 0.3 * f.mid);
        let speed = 1.5 + 7.0 * f.level;
        let (sy, cy) = self.yaw.sin_cos();
        self.x += sy * speed * dt;
        self.z += cy * speed * dt;
        // Wrap to keep f32 precision; 1000 × 0.7 is a whole number of tiles.
        self.x = self.x.rem_euclid(1000.0);
        self.z = self.z.rem_euclid(1000.0);
        self.hue += dt * 0.015;

        let (w, h) = (c.w as f32, c.h as f32);
        let horizon = h * (0.42 + 0.03 * (t * 0.5).sin());
        let focal = (w * 0.55).max(1.0);
        let cam_h = 1.0 + 0.25 * f.pulse;
        let fwd = (sy, cy);
        let right = (cy, -sy);
        let fog = floats(hsv(self.hue + 0.45, 0.55, 0.45 + 0.2 * f.bass));
        let sky_top = floats(hsv(self.hue + 0.2, 0.8, 0.12));
        let cloud_col = floats(hsv(self.hue + 0.35, 0.3, 0.75 + 0.25 * f.treble));
        let seam = floats(hsv(self.hue + 0.6, 0.5, 0.7 + 0.3 * self.flash));
        let bins = f.spectrum.len();

        for y in 0..c.h {
            let dy = y as f32 + 0.5 - horizon;
            if dy.abs() < 0.6 {
                let col = rgb(fog[0] * 1.4, fog[1] * 1.4, fog[2] * 1.4);
                for x in 0..c.w {
                    c.set(x, y, col);
                }
                continue;
            }
            let floor = dy > 0.0;
            let plane = if floor { cam_h } else { 3.0 };
            let dist = plane * focal / dy.abs();
            let fogk = if floor {
                (dist / 28.0).powf(0.7)
            } else {
                (dist / 60.0).powf(0.6)
            }
            .min(1.0);
            let x0 = (w * 0.5 - 0.5) / focal * dist;
            let mut px = self.x + fwd.0 * dist - right.0 * x0;
            let mut pz = self.z + fwd.1 * dist - right.1 * x0;
            let (stx, stz) = (right.0 * dist / focal, right.1 * dist / focal);
            let sky = blend(sky_top, fog, (1.0 + dy / horizon.max(1.0)).clamp(0.0, 1.0));

            for x in 0..c.w {
                let col = if floor {
                    let (u, v) = (px * 0.7, pz * 0.7);
                    let (iu, iv) = (u.floor(), v.floor());
                    let id = hash(iu as i32, iv as i32);
                    let e = if bins > 0 { f.spectrum[id as usize % bins] } else { 0.0 };
                    let m = (u - iu - 0.5).abs().max((v - iv - 0.5).abs());
                    let base = if m > 0.44 {
                        seam
                    } else {
                        let ring = ((m * 9.0) as i32) & 1 == 0;
                        let k = (0.18 + 0.82 * e) * if ring { 1.0 } else { 0.6 };
                        floats(hsv(self.hue + ((id >> 7) % 3) as f32 * 0.12, 0.75, k))
                    };
                    blend(base, fog, fogk)
                } else {
                    let (u, v) = (px * 0.12, pz * 0.12 + t * 0.03);
                    let n = (u * 3.0 + (v * 2.1).sin()).sin() * (v * 2.7 + (u * 1.7).sin() * 1.3).sin();
                    let cloud = ((n - 0.15) * 2.5).clamp(0.0, 1.0);
                    blend(blend(sky, cloud_col, cloud * (1.0 - fogk)), fog, fogk * 0.7)
                };
                c.set(x, y, rgb(col[0], col[1], col[2]));
                px += stx;
                pz += stz;
            }
        }
    }
}
