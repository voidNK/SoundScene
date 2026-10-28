use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, rgb};

const STARS: usize = 160;
const CURTAINS: usize = 3;

fn hash(i: i32) -> f32 {
    let mut n = (i as u32).wrapping_mul(0x9E37_79B9);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 13;
    (n & 0xFFFF) as f32 / 65535.0
}

/// Smooth 1D value noise in 0..1.
fn noise(x: f32) -> f32 {
    let i = x.floor();
    let t = x - i;
    let t = t * t * (3.0 - 2.0 * t);
    let i = i as i32;
    hash(i) + (hash(i.wrapping_add(1)) - hash(i)) * t
}

/// Northern lights: three folded curtains of vertical rays, green at the
/// lower edge fading to violet on top, over a starry sky and a mountain
/// silhouette. The mids drive curtain brightness and height, the spectrum
/// lights individual rays, beats make the whole sky flare.
pub struct Aurora {
    stars: Vec<(f32, f32, f32)>,
    time: f32,
    drift: f32,
}

impl Aurora {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xA_0A0A);
        let stars = (0..STARS)
            .map(|_| (rng.f32(), rng.f32() * 0.85, rng.range(0.0, std::f32::consts::TAU)))
            .collect();
        Self {
            stars,
            time: 0.0,
            drift: 0.0,
        }
    }
}

impl Effect for Aurora {
    fn name(&self) -> &'static str {
        "Aurora"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        self.drift += dt * (0.15 + 0.5 * f.mid + 0.4 * f.pulse);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (w, h) = (c.w as f32, c.h as f32);
        let (t, drift) = (self.time, self.drift);
        let amp = 0.3 + 1.1 * f.mid + 0.4 * f.pulse;
        let bins = f.spectrum.len();

        // Per column and curtain: lower edge, ray brightness, ray height.
        let mut cols = vec![[(0.0f32, 0.0f32, 1.0f32); CURTAINS]; c.w];
        for (x, col) in cols.iter_mut().enumerate() {
            let u = x as f32 / w;
            let s = if bins > 0 {
                f.spectrum[((u * bins as f32) as usize).min(bins - 1)]
            } else {
                0.0
            };
            for (r, cur) in col.iter_mut().enumerate() {
                let rf = r as f32;
                let edge = h
                    * (0.6 - 0.07 * rf
                        + 0.07 * (u * (4.0 + rf) + t * (0.2 + 0.07 * rf) + rf * 2.0).sin()
                        + 0.06 * (noise(u * 3.0 + drift * 0.3 + rf * 9.0) - 0.5));
                let n = noise(x as f32 / h * 22.0 + drift * (1.0 + 0.4 * rf) + rf * 17.0);
                let fold = 0.55 + 0.45 * (u * (6.0 + 2.0 * rf) - drift * 0.7 + rf).sin();
                let ray = (0.15 + 0.85 * n * n) * fold * (0.6 + 0.7 * s) * amp * (1.0 - 0.25 * rf);
                let height = h * (0.1 + 0.16 * f.mid + 0.04 * rf + 0.1 * n);
                *cur = (edge, ray, height.max(1.0));
            }
        }

        let horizon: Vec<f32> = (0..c.w)
            .map(|x| {
                let u = x as f32 / w;
                h * (0.8 - 0.06 * (u * 5.0 + 1.0).sin() - 0.03 * (u * 13.0 + 2.0).sin() - 0.03 * noise(u * 30.0))
            })
            .collect();

        for y in 0..c.h {
            let fy = y as f32;
            let g = fy / h;
            let sky = [0.005 + 0.02 * g, 0.01 + 0.05 * g, 0.04 + 0.08 * g];
            for (x, (col, &hz)) in cols.iter().zip(&horizon).enumerate() {
                if fy >= hz {
                    c.set(x, y, rgb(0.005, 0.01, 0.015));
                    continue;
                }
                let mut px = sky;
                for &(edge, ray, height) in col {
                    let d = edge - fy;
                    if d > height * 5.0 {
                        continue;
                    }
                    let v = if d >= 0.0 {
                        ray * (-d / height).exp()
                    } else {
                        ray * (d * 0.7).exp()
                    };
                    // Green near the edge, violet higher up, a pink fringe below.
                    let k = (d / height * 0.7).clamp(0.0, 1.0);
                    let (r, g, b) = if d < 0.0 {
                        (0.6, 0.5, 0.4)
                    } else {
                        (0.1 + 0.5 * k, 1.0 - 0.8 * k, 0.4 + 0.5 * k)
                    };
                    px[0] += r * v;
                    px[1] += g * v;
                    px[2] += b * v;
                }
                c.set(x, y, rgb(px[0], px[1], px[2]));
            }
        }

        for &(sx, sy, phase) in &self.stars {
            let (x, y) = ((sx * w) as usize, (sy * h) as usize);
            if x >= c.w || y >= c.h || y as f32 >= horizon[x] {
                continue;
            }
            let b = 0.25 + 0.2 * (t * 2.5 + phase * 3.0).sin() + 0.2 * f.treble;
            c.plot_add(x as i32, y as i32, rgb(b, b, b * 1.1));
        }
    }
}
