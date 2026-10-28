use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, rgb};

/// Shadow radius, disk edges and Einstein radius, in units of the
/// Schwarzschild radius.
const SHADOW: f32 = 2.6;
const DISK_IN: f32 = 3.0;
const DISK_OUT: f32 = 8.5;
const EINSTEIN: f32 = 3.4;

fn hash(x: i32, y: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^ (h >> 15)
}

/// Point-like background stars on a jittered grid.
fn stars(x: f32, y: f32) -> f32 {
    let (cx, cy) = (x.floor(), y.floor());
    let hv = hash(cx as i32, cy as i32);
    if hv & 3 != 0 {
        return 0.0;
    }
    let sx = cx + 0.2 + 0.6 * ((hv >> 8) & 255) as f32 / 255.0;
    let sy = cy + 0.2 + 0.6 * ((hv >> 16) & 255) as f32 / 255.0;
    let d2 = (x - sx) * (x - sx) + (y - sy) * (y - sy);
    let b = 0.3 + 0.7 * (hv >> 24) as f32 / 255.0;
    (b * (1.0 - d2 * 40.0)).max(0.0)
}

/// Black hole with a hot, nearly edge-on accretion disk. The far side of the
/// disk is lensed into an arc over the shadow, a thin photon ring circles it
/// and background stars smear around the Einstein ring. The approaching side
/// is brighter (Doppler beaming); loudness drives the disk, beats flare it.
pub struct BlackHole {
    swirl: f32,
    drift: f32,
}

impl BlackHole {
    pub fn new() -> Self {
        Self { swirl: 0.0, drift: 0.0 }
    }
}

/// Disk emission at disk radius `r` and azimuth (`ca`, `sa`) = cos/sin.
fn emission(r: f32, ca: f32, sa: f32, swirl: f32) -> f32 {
    if !(DISK_IN..DISK_OUT).contains(&r) {
        return 0.0;
    }
    let edge = ((r - DISK_IN) * 2.0).min(1.0) * ((DISK_OUT - r) * 0.5).min(1.0);
    let falloff = (DISK_IN / r).powf(1.6);
    // Keplerian swirl: angle offset grows faster close to the hole.
    let spin = swirl * (DISK_IN / r).powf(1.5);
    let (ss, cs) = spin.sin_cos();
    let (sa, ca) = (sa * cs + ca * ss, ca * cs - sa * ss);
    let lr = r.ln();
    let tex = 0.65 + 0.2 * (lr * 23.0 + 2.0 * ca).sin() + 0.15 * (lr * 41.0 + 3.0 * sa).sin();
    edge * falloff * tex
}

fn heat(v: f32, doppler: f32) -> (f32, f32, f32) {
    let v = v.max(0.0);
    let blue = (doppler - 1.0).max(0.0);
    (
        v * 1.25,
        v * (0.55 + 0.45 * v.min(1.0)) + blue * v * 0.2,
        v * (0.2 + 0.4 * v.min(1.0)) + blue * v * 0.6,
    )
}

impl Effect for BlackHole {
    fn name(&self) -> &'static str {
        "Black Hole"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.swirl += dt * (0.8 + 1.5 * f.level);
        self.drift += dt * 0.15;
        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let unit = (w * 0.055).min(h * 0.085).max(0.1);
        let cos_i = 0.18 + 0.06 * (t * 0.13).sin();
        let gain = 0.45 + 0.9 * f.level + 0.6 * f.pulse;
        let (roll_s, roll_c) = (0.12 * (t * 0.09).sin()).sin_cos();

        for y in 0..c.h {
            for x in 0..c.w {
                let sx = (x as f32 + 0.5 - cx) / unit;
                let sy = (y as f32 + 0.5 - cy) / unit;
                let (px, py) = (sx * roll_c + sy * roll_s, -sx * roll_s + sy * roll_c);
                let r = (px * px + py * py).sqrt().max(1e-3);
                let (ux, uy) = (px / r, py / r);

                let (mut cr, mut cg, mut cb) = if r < SHADOW {
                    (0.0, 0.0, 0.0)
                } else {
                    // Thin-lens mapping to the unlensed sky position.
                    let beta = r - EINSTEIN * EINSTEIN / r;
                    let s = stars(ux * beta * 0.8 + self.drift, uy * beta * 0.8 + 3.0);
                    let s = s * (EINSTEIN / (r - EINSTEIN).abs().max(0.3)).clamp(1.0, 2.5);
                    let mut col = (s * 0.8, s * 0.85, s);
                    // Lensed image of the far disk side, arched over the top.
                    let u = (r - SHADOW) / 2.8;
                    if u < 1.0 {
                        let top = (-uy).max(0.0).sqrt() * 0.9 + 0.12;
                        let rd = DISK_IN + u * (DISK_OUT - DISK_IN);
                        let dop = 1.0 - 0.55 * ux;
                        let e = emission(rd, -ux, uy.abs(), self.swirl) * top * dop * 0.85;
                        let (r2, g2, b2) = heat(e * gain, dop);
                        col = (col.0 + r2, col.1 + g2, col.2 + b2);
                    }
                    let ring = (-((r - SHADOW * 1.02) * 7.0).powi(2)).exp() * (0.5 + 0.5 * gain);
                    (col.0 + ring * 1.0, col.1 + ring * 0.75, col.2 + ring * 0.5)
                };

                // Direct image of the disk; its near half covers the shadow.
                let dz = py / cos_i;
                let rd = (px * px + dz * dz).sqrt().max(1e-3);
                if (DISK_IN..DISK_OUT).contains(&rd) && (py > 0.0 || r >= SHADOW) {
                    let (ca, sa) = (px / rd, dz / rd);
                    let dop = 1.0 - 0.6 * ca;
                    let e = emission(rd, ca, sa, self.swirl) * dop;
                    let (r2, g2, b2) = heat(e * gain, dop);
                    (cr, cg, cb) = (cr + r2, cg + g2, cb + b2);
                }
                c.set(x, y, rgb(cr, cg, cb));
            }
        }
    }
}
