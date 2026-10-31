use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, rgb};

type V3 = [f32; 3];

const CAM: f32 = 7.0;
const RING_IN: f32 = 1.35;
const RING_OUT: f32 = 2.35;

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(v: V3) -> V3 {
    let l = dot(v, v).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn hash(x: u32, y: u32) -> u32 {
    let mut h = x.wrapping_mul(0x8DA6_B343) ^ y.wrapping_mul(0xD816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^ (h >> 16)
}

/// Nearest positive hit of the ray `o + t·d` with the unit sphere.
fn sphere_hit(o: V3, d: V3) -> Option<f32> {
    let b = dot(o, d);
    let disc = b * b - (dot(o, o) - 1.0);
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    (t > 1e-3).then_some(t)
}

/// Ring opacity at radius `r` (0 outside the rings).
fn ring_density(r: f32) -> f32 {
    if !(RING_IN..=RING_OUT).contains(&r) {
        return 0.0;
    }
    let s = (r - RING_IN) / (RING_OUT - RING_IN);
    // A dark division a bit past the middle, fine ringlets everywhere.
    let gap = ((s - 0.62).abs() * 30.0).min(1.0);
    let fine = 0.6 + 0.25 * (s * 90.0).sin() + 0.15 * (s * 37.0).sin();
    let edge = (s * 12.0).min(1.0) * ((1.0 - s) * 20.0).min(1.0);
    (fine * gap * edge).clamp(0.0, 1.0)
}

/// A banded gas giant with tilted rings: the rings pass in front of and
/// behind the planet, the planet casts its shadow on them and they cast
/// theirs on it. Ring brightness follows the spectrum from inner to outer
/// edge, treble makes them shimmer and bass pulses the atmosphere glow.
pub struct RingedPlanet {
    spin: f32,
    tilt: f32,
    sun: f32,
}

impl RingedPlanet {
    pub fn new() -> Self {
        Self {
            spin: 0.0,
            tilt: 0.0,
            sun: 0.0,
        }
    }
}

impl Effect for RingedPlanet {
    fn name(&self) -> &'static str {
        "Ringed Planet"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.spin += dt * (0.15 + 0.4 * f.mid);
        self.tilt += dt * 0.12;
        self.sun += dt * 0.05;
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        // Ring plane normal (also the planet's spin axis), slowly wobbling.
        let tx = 0.32 + 0.12 * self.tilt.sin();
        let tz = 0.35 + 0.1 * (self.tilt * 0.7).cos();
        let axis = norm([tz.sin(), -tx.cos() * tz.cos(), -tx.sin()]);
        // Two vectors spanning the equator, for the longitude of the bands.
        let e1 = norm([axis[1], -axis[0], 0.0]);
        let e2 = [
            axis[1] * e1[2] - axis[2] * e1[1],
            axis[2] * e1[0] - axis[0] * e1[2],
            axis[0] * e1[1] - axis[1] * e1[0],
        ];
        let light = norm([-0.8 + 0.3 * self.sun.sin(), -0.35, -0.45]);
        let focal = (w as f32 * 0.5 / 2.6).min(h as f32 * 0.5 / 1.3) * CAM;
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let ro = [0.0, 0.0, -CAM];
        let bins = f.spectrum.len();
        let frame = (t * 12.0) as u32;
        let glow_k = 0.25 + 0.5 * f.bass + 0.3 * f.pulse;

        for y in 0..h {
            for x in 0..w {
                let rd = norm([(x as f32 + 0.5 - cx) / focal, (y as f32 + 0.5 - cy) / focal, 1.0]);
                // Background: sparse twinkling stars plus the atmosphere halo.
                let hs = hash(x as u32, y as u32);
                let mut col = if hs.is_multiple_of(97) {
                    let tw = 0.5 + 0.5 * (t * 2.0 + (hs >> 8) as f32).sin();
                    let b = 0.25 + 0.5 * tw * ((hs >> 16) & 0xFF) as f32 / 255.0;
                    [b, b, b * 1.1]
                } else {
                    [0.0, 0.0, 0.02]
                };
                let b = dot(ro, rd);
                let miss = (dot(ro, ro) - b * b).max(0.0).sqrt();
                if miss > 1.0 {
                    let g = (-(miss - 1.0) * 9.0).exp() * glow_k;
                    col = [col[0] + 0.3 * g, col[1] + 0.55 * g, col[2] + g];
                }

                let ts = sphere_hit(ro, rd);
                if let Some(ts) = ts {
                    let n = [ro[0] + rd[0] * ts, ro[1] + rd[1] * ts, ro[2] + rd[2] * ts];
                    let lat = dot(n, axis);
                    let lon = dot(n, e2).atan2(dot(n, e1)) + self.spin;
                    let swirl = 0.35 * (lon * 3.0 + lat * 8.0).sin();
                    let band = (lat * 14.0 + swirl).sin() * 0.5 + 0.5;
                    let fine = (lat * 41.0 + swirl * 2.0).sin() * 0.5 + 0.5;
                    let mut base = [0.75 + 0.2 * band, 0.5 + 0.2 * band - 0.1 * fine, 0.3 + 0.1 * fine];
                    // A storm spot riding along one band.
                    let spot = ((lat + 0.35) * 9.0).powi(2) + (lon.sin() * 3.0).powi(2);
                    if lon.cos() > 0.0 && spot < 1.0 {
                        let k = 1.0 - spot;
                        base = [base[0] + 0.2 * k, base[1] - 0.15 * k, base[2] - 0.1 * k];
                    }
                    let mut diff = dot(n, light).max(0.0);
                    // Shadow of the rings: march towards the sun to the ring plane.
                    let dl = dot(light, axis);
                    if dl.abs() > 1e-4 {
                        let tl = -dot(n, axis) / dl;
                        if tl > 0.0 {
                            let p = [n[0] + light[0] * tl, n[1] + light[1] * tl, n[2] + light[2] * tl];
                            diff *= 1.0 - 0.7 * ring_density(dot(p, p).sqrt());
                        }
                    }
                    let rim = (1.0 + dot(n, rd)).powi(3) * glow_k;
                    let k = 0.04 + 0.96 * diff;
                    col = [base[0] * k + 0.3 * rim, base[1] * k + 0.5 * rim, base[2] * k + rim];
                }

                // Rings: composite over whatever lies behind them.
                let dn = dot(rd, axis);
                if dn.abs() > 1e-5 {
                    let tr = -dot(ro, axis) / dn;
                    if tr > 0.0 && ts.is_none_or(|ts| tr < ts) {
                        let p = [ro[0] + rd[0] * tr, ro[1] + rd[1] * tr, ro[2] + rd[2] * tr];
                        let r = dot(p, p).sqrt();
                        let dens = ring_density(r);
                        if dens > 0.0 {
                            let s = (r - RING_IN) / (RING_OUT - RING_IN);
                            let lvl = if bins > 0 {
                                f.spectrum[((s * (bins - 1) as f32) as usize).min(bins - 1)]
                            } else {
                                0.0
                            };
                            let lit = if sphere_hit(p, light).is_some() { 0.12 } else { 1.0 };
                            let sparkle = if hash(x as u32 ^ frame, y as u32).is_multiple_of(23) {
                                0.6 * f.treble
                            } else {
                                0.0
                            };
                            let k = lit * (0.35 + 0.45 * dot(light, axis).abs() + 0.5 * lvl) + sparkle;
                            let rc = [0.85 * k, 0.75 * k, 0.6 * k + 0.1 * lvl];
                            let a = dens * 0.85;
                            col = [
                                col[0] + (rc[0] - col[0]) * a,
                                col[1] + (rc[1] - col[1]) * a,
                                col[2] + (rc[2] - col[2]) * a,
                            ];
                        }
                    }
                }
                c.set(x, y, rgb(col[0], col[1], col[2]));
            }
        }
    }
}
