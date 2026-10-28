use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, rgb, scale};

const BANDS: usize = 16;
const PER_SIDE: usize = 14;
const FAR: f32 = 60.0;
const NEAR: f32 = 0.25;
const STREET: f32 = 2.2;
const CAM_H: f32 = 1.4;
const FLOOR: f32 = 0.55;
const WINDOW: f32 = 0.45;

struct Building {
    /// -1 for the left side of the street, 1 for the right side.
    side: f32,
    width: f32,
    z: f32,
    depth: f32,
    height: f32,
    band: usize,
    seed: u32,
}

/// Night flight down an avenue whose buildings are equalizer bars: every
/// tower belongs to one band and grows with it, its windows light up with
/// that band's energy and its neon roof line glows. A distant skyline shows
/// the full spectrum, bass and beats push the camera forward.
pub struct EqualizerCity {
    buildings: Vec<Building>,
    bands: [f32; BANDS],
    skyline: Vec<f32>,
    cam: f32,
    rng: Rng,
}

fn hash(a: u32, b: u32, c: u32) -> f32 {
    let mut x = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B) ^ c.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_536.0
}

/// True if a facade point lies in a window; returns the window's id.
fn window(u: f32, v: f32) -> Option<(u32, u32)> {
    let (fu, fv) = ((u / WINDOW).rem_euclid(1.0), (v / FLOOR).rem_euclid(1.0));
    ((0.25..0.75).contains(&fu) && (0.3..0.75).contains(&fv))
        .then(|| ((u / WINDOW).floor() as i32 as u32, (v / FLOOR).floor() as i32 as u32))
}

impl EqualizerCity {
    pub fn new() -> Self {
        let mut s = Self {
            buildings: Vec::new(),
            bands: [0.0; BANDS],
            skyline: Vec::new(),
            cam: 0.0,
            rng: Rng::new(0xC17E),
        };
        for side in [-1.0, 1.0] {
            let mut z = 1.0;
            for _ in 0..PER_SIDE {
                let b = s.spawn(side, z);
                z = b.z + b.depth;
                s.buildings.push(b);
            }
        }
        s
    }

    fn spawn(&mut self, side: f32, after: f32) -> Building {
        let r = &mut self.rng;
        Building {
            side,
            width: r.range(1.5, 3.5),
            z: after + r.range(0.6, 2.0),
            depth: r.range(2.0, 4.5),
            height: r.range(3.0, 9.0),
            band: (r.f32() * BANDS as f32) as usize % BANDS,
            seed: r.next_u32(),
        }
    }
}

impl Effect for EqualizerCity {
    fn name(&self) -> &'static str {
        "Equalizer City"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        let bins = f.spectrum.len();
        let band = |i: usize, n: usize| -> f32 {
            if bins == 0 {
                return 0.0;
            }
            let (a, b) = (i * bins / n, ((i + 1) * bins / n).max(i * bins / n + 1).min(bins));
            let s = &f.spectrum[a.min(bins - 1)..b];
            s.iter().sum::<f32>() / s.len().max(1) as f32
        };
        let k = 1.0 - (-10.0 * dt).exp();
        for i in 0..BANDS {
            self.bands[i] += (band(i, BANDS) - self.bands[i]) * k;
        }
        let cols = c.w.max(1);
        self.skyline.resize(cols, 0.0);
        let n_sky = (cols / 3).clamp(1, 64);
        for (x, s) in self.skyline.iter_mut().enumerate() {
            // Mirrored around the vanishing point, bass in the middle.
            let d = ((x as f32 / cols as f32 - 0.5).abs() * 2.0 * n_sky as f32) as usize;
            *s += (band(d.min(n_sky - 1), n_sky) - *s) * k;
        }

        self.cam += dt * (2.5 + 7.0 * f.bass + 6.0 * f.pulse);
        // Rebase to keep float precision over hours of flight.
        if self.cam > 1000.0 {
            for b in &mut self.buildings {
                b.z -= 1000.0;
            }
            self.cam -= 1000.0;
        }
        for i in 0..self.buildings.len() {
            if self.buildings[i].z + self.buildings[i].depth < self.cam + NEAR {
                let side = self.buildings[i].side;
                let last = self
                    .buildings
                    .iter()
                    .filter(|b| b.side == side)
                    .map(|b| b.z + b.depth)
                    .fold(self.cam, f32::max);
                self.buildings[i] = self.spawn(side, last);
            }
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, hor) = (w * 0.5, h * 0.5);
        let fpx = h.max(1.0) * 0.8;
        let camx = (t * 0.25).sin() * 0.6;
        let sky_low = rgb(0.22, 0.06, 0.28);

        // Sky, moon, stars and the street below the horizon.
        let (mx, my, mr) = (w * 0.8, h * 0.17, h * 0.07 * (1.0 + 0.1 * f.pulse));
        for y in 0..c.h {
            let yf = y as f32 + 0.5;
            for x in 0..c.w {
                let xf = x as f32 + 0.5;
                let col = if yf < hor {
                    let k = (yf / hor.max(1.0)).powi(2);
                    let mut col = mix(rgb(0.01, 0.0, 0.05), sky_low, k);
                    if hash(x as u32, y as u32, 7) < 0.004 {
                        col = [210, 210, 240];
                    }
                    let (dx, dy) = (xf - mx, yf - my);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < mr {
                        col = rgb(0.95, 0.9, 0.75);
                    } else if d < mr * 2.5 {
                        col = mix(col, rgb(0.5, 0.4, 0.5), 0.25 * (1.0 - (d - mr) / (mr * 1.5)));
                    }
                    col
                } else {
                    let zr = CAM_H * fpx / (yf - hor).max(0.01);
                    let wx = (xf - cx) * zr / fpx + camx;
                    let mut col = if wx.abs() < STREET {
                        let dash = wx.abs() < 0.06 && ((self.cam + zr) / 3.0).rem_euclid(1.0) < 0.5;
                        if dash {
                            rgb(0.8, 0.7, 0.3)
                        } else {
                            rgb(0.06, 0.06, 0.08)
                        }
                    } else {
                        rgb(0.12, 0.11, 0.14)
                    };
                    // Faint neon reflections of the street lights on the asphalt.
                    if wx.abs() < STREET {
                        let glow = (1.0 - (wx.abs() - STREET).abs()).max(0.0) * 0.3;
                        col = mix(col, hsv(0.85 + 0.1 * f.mid, 0.8, 0.8), glow * (0.4 + f.level));
                    }
                    mix(col, sky_low, (zr / FAR).clamp(0.0, 1.0).powf(0.6))
                };
                c.set(x, y, col);
            }
        }

        // Distant skyline sitting on the horizon.
        for x in 0..c.w {
            let s = self.skyline[x.min(self.skyline.len() - 1)];
            let top = hor - h * (0.04 + 0.2 * s);
            let y0 = top.max(0.0) as usize;
            let y1 = (hor.max(0.0) as usize).min(c.h);
            for y in y0..y1 {
                let lit = hash(x as u32 / 2, y as u32 / 2, 3) < 0.15 + 0.5 * s && x % 2 == 0 && y % 2 == 1;
                let col = if (y as f32) < top + 1.0 {
                    hsv(0.55 + x as f32 / w * 0.4, 0.7, 0.4 + 0.6 * s)
                } else if lit {
                    rgb(0.55, 0.45, 0.2)
                } else {
                    rgb(0.09, 0.06, 0.15)
                };
                c.set(x, y, col);
            }
        }

        // Towers, far to near.
        let mut order: Vec<usize> = (0..self.buildings.len()).collect();
        order.sort_by(|&a, &b| self.buildings[b].z.total_cmp(&self.buildings[a].z));
        for i in order {
            let b = &self.buildings[i];
            let lvl = self.bands[b.band];
            let height = b.height * (0.35 + 1.2 * lvl);
            let hue = b.band as f32 / BANDS as f32 * 0.8 + 0.5;
            let lit_frac = 0.1 + 0.8 * lvl;
            let inner = b.side * STREET - camx;
            let outer = b.side * (STREET + b.width) - camx;
            let wall = rgb(0.05, 0.04, 0.08);
            let shade = |col: [u8; 3], zr: f32| mix(col, sky_low, (zr / FAR).clamp(0.0, 1.0).powf(0.8));
            let paint = |c: &mut Canvas, x: usize, zr: f32, u: f32, front: bool| {
                let ground = hor + CAM_H * fpx / zr;
                let top = hor - (height - CAM_H) * fpx / zr;
                let y0 = top.max(0.0) as usize;
                let y1 = (ground.max(0.0) as usize).min(c.h);
                for y in y0..y1 {
                    let v = CAM_H + (hor - y as f32 - 0.5) * zr / fpx;
                    let col = if (y as f32) < top + 1.0 {
                        hsv(hue, 0.8, 0.5 + 0.5 * lvl)
                    } else if let Some((wu, wv)) = window(u, v).filter(|_| v < height - 0.3) {
                        if hash(b.seed, wu, wv) < lit_frac {
                            mix(rgb(1.0, 0.8, 0.4), hsv(hue, 0.6, 1.0), 0.3)
                        } else {
                            rgb(0.03, 0.03, 0.05)
                        }
                    } else if front {
                        wall
                    } else {
                        scale(wall, 0.6)
                    };
                    c.set(x, y, shade(col, zr));
                }
            };

            // Street-facing side wall, clipped at the near plane.
            let za = (b.z - self.cam).max(NEAR);
            let zb = b.z + b.depth - self.cam;
            if zb > za && zb > NEAR {
                let sa = cx + inner * fpx / za;
                let sb = cx + inner * fpx / zb;
                let (lo, hi) = (sa.min(sb).max(0.0) as usize, (sa.max(sb).max(0.0) as usize).min(c.w));
                for x in lo..hi {
                    let dx = x as f32 + 0.5 - cx;
                    if dx * inner <= 0.0 {
                        continue;
                    }
                    let zr = (inner * fpx / dx).clamp(za, zb);
                    paint(c, x, zr, self.cam + zr, false);
                }
            }
            // Front facade.
            let zr = b.z - self.cam;
            if zr > NEAR {
                let (s0, s1) = (cx + inner * fpx / zr, cx + outer * fpx / zr);
                let (lo, hi) = (s0.min(s1).max(0.0) as usize, (s0.max(s1).max(0.0) as usize).min(c.w));
                for x in lo..hi {
                    let u = (x as f32 + 0.5 - cx) * zr / fpx - inner;
                    paint(c, x, zr, u.abs(), true);
                }
            }
        }
    }
}
