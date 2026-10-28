use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, rgb};
use std::f32::consts::PI;

const TILT: f32 = 0.41;
/// Grid spacing in radians (15°).
const GRID: f32 = PI / 12.0;

fn hash3(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343)
        ^ (y as u32).wrapping_mul(0xD816_3841)
        ^ (z as u32).wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

fn noise(x: f32, y: f32, z: f32) -> f32 {
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (fx, fy, fz) = (x - xi, y - yi, z - zi);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (u, v, w) = (s(fx), s(fy), s(fz));
    let (xi, yi, zi) = (xi as i32, yi as i32, zi as i32);
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let c = |dx, dy, dz| hash3(xi + dx, yi + dy, zi + dz);
    l(
        l(l(c(0, 0, 0), c(1, 0, 0), u), l(c(0, 1, 0), c(1, 1, 0), u), v),
        l(l(c(0, 0, 1), c(1, 0, 1), u), l(c(0, 1, 1), c(1, 1, 1), u), v),
        w,
    )
}

fn fbm(x: f32, y: f32, z: f32) -> f32 {
    noise(x, y, z) * 0.55 + noise(x * 2.1, y * 2.1, z * 2.1) * 0.3 + noise(x * 4.3, y * 4.3, z * 4.3) * 0.15
}

/// Closeness (0..1) of `a` to the nearest multiple of `step`, for grid lines.
fn line(a: f32, step: f32, width: f32) -> f32 {
    let d = ((a / step).rem_euclid(1.0) - 0.5).abs() * 2.0;
    ((d - (1.0 - width)) / width).clamp(0.0, 1.0)
}

/// A spinning planet with procedural continents, ice caps, city lights on
/// the night side and a glowing latitude/longitude grid. Bass spins it,
/// spectrum bands light up the latitude lines, beats pulse the atmosphere.
pub struct Globe {
    spin: f32,
}

impl Globe {
    pub fn new() -> Self {
        Self { spin: 0.0 }
    }
}

impl Effect for Globe {
    fn name(&self) -> &'static str {
        "Globe"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.spin += dt * (0.12 + 0.5 * f.bass);
        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let radius = (w.min(h) * 0.4 * (1.0 + 0.03 * f.pulse)).max(0.5);
        let (ss, cs) = self.spin.sin_cos();
        let (st, ct) = TILT.sin_cos();
        let la = 0.6 * (t * 0.05).sin() - 0.5;
        let light = {
            let (x, y, z) = (la.sin(), 0.35f32, la.cos());
            let l = (x * x + y * y + z * z).sqrt();
            [x / l, y / l, z / l]
        };
        let bins = f.spectrum.len();
        let atmo = 0.35 + 0.65 * f.pulse;
        let grid_gain = 0.15 + 0.5 * f.mid;
        // Grid line width in angle units shrinks as the planet gets larger.
        let lw = (1.6 / (radius * GRID)).clamp(0.05, 0.5);

        for y in 0..c.h {
            let dy = -(y as f32 + 0.5 - cy) / radius;
            for x in 0..c.w {
                let dx = (x as f32 + 0.5 - cx) / radius;
                let d2 = dx * dx + dy * dy;
                let col = if d2 < 1.0 {
                    let nz = (1.0 - d2).sqrt();
                    let n = [dx, dy, nz];
                    let diff = n[0] * light[0] + n[1] * light[1] + n[2] * light[2];
                    // View space -> planet space: undo tilt, then spin.
                    let (py, pz) = (n[1] * ct + n[2] * st, -n[1] * st + n[2] * ct);
                    let (px, pz) = (n[0] * cs - pz * ss, n[0] * ss + pz * cs);
                    let lat = py.clamp(-1.0, 1.0).asin();
                    let lon = pz.atan2(px);
                    let e = fbm(px * 1.8 + 7.0, py * 1.8 + 3.0, pz * 1.8 + 11.0) - 0.52;
                    let ice = (py.abs() - 0.82 + e * 0.3).max(0.0) * 8.0;
                    let (mut r, mut g, mut b) = if e > 0.0 {
                        let k = (e * 5.0).min(1.0);
                        (0.15 + 0.35 * k, 0.4 - 0.1 * k, 0.12 + 0.05 * k)
                    } else {
                        let k = (-e * 4.0).min(1.0);
                        (0.02, 0.12 - 0.06 * k, 0.4 - 0.15 * k)
                    };
                    if ice > 0.0 {
                        let k = ice.min(1.0);
                        (r, g, b) = (r + (0.85 - r) * k, g + (0.9 - g) * k, b + (0.95 - b) * k);
                    }
                    let day = diff.max(0.0);
                    let lit = 0.05 + 0.95 * day;
                    let (mut r, mut g, mut b) = (r * lit, g * lit, b * lit);
                    if e <= 0.0 && ice <= 0.0 {
                        // Sun glint on the ocean.
                        let half = [light[0], light[1], light[2] + 1.0];
                        let hl = (half[0] * half[0] + half[1] * half[1] + half[2] * half[2])
                            .sqrt()
                            .max(1e-3);
                        let glint = ((n[0] * half[0] + n[1] * half[1] + n[2] * half[2]) / hl)
                            .max(0.0)
                            .powi(40)
                            * 0.5;
                        (r, g, b) = (r + glint, g + glint, b + glint);
                    }
                    let night = (-diff * 3.0).clamp(0.0, 1.0);
                    if e > 0.02 && ice <= 0.0 && night > 0.0 {
                        let cell = hash3((lon * 40.0) as i32, (lat * 40.0) as i32, 3);
                        if cell > 0.8 {
                            let k = night * (0.3 + 0.7 * f.treble) * (cell - 0.8) * 5.0;
                            (r, g, b) = (r + 0.9 * k, g + 0.7 * k, b + 0.3 * k);
                        }
                    }
                    let lat_i = ((lat + PI * 0.5) / GRID).round() as usize;
                    let band = if bins > 0 {
                        f.spectrum[(lat_i * bins / 13).min(bins - 1)]
                    } else {
                        0.0
                    };
                    let gl = line(lat, GRID, lw) * (grid_gain + 0.6 * band)
                        + line(lon, GRID, (lw / lat.cos().max(0.1)).min(0.9)) * grid_gain;
                    let gl = gl * (0.4 + 0.6 * nz);
                    let rim = (1.0 - nz).powi(3) * atmo;
                    rgb(
                        r + gl * 0.1 + rim * 0.3,
                        g + gl * 0.8 + rim * 0.6,
                        b + gl * 0.9 + rim * 1.0,
                    )
                } else {
                    let r = d2.sqrt();
                    let side = (dx * light[0] + dy * light[1]) / r;
                    let glow = (-(r - 1.0) * 14.0).exp() * atmo * (0.5 + 0.5 * side.max(0.0));
                    let star = if hash3(x as i32, y as i32, 0) > 0.995 {
                        hash3(x as i32, y as i32, 1) * 0.6
                    } else {
                        0.0
                    };
                    rgb(star + glow * 0.3, star + glow * 0.6, star + glow)
                };
                c.set(x, y, col);
            }
        }
    }
}
