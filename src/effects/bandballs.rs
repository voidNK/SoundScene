use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, add, hsv, mix, rgb, scale};

const MAX_BALLS: usize = 8;
/// Gravity in units of the jump height per second².
const GRAVITY: f32 = 4.5;

#[derive(Clone, Copy, Default)]
struct Ball {
    /// Height above the floor, 0..1 of the available travel.
    y: f32,
    vy: f32,
    squash: f32,
    slow: f32,
}

/// A row of glossy balls, one per group of spectrum bands, bouncing on a
/// floor as high as their band is loud. Band onsets kick them mid-air,
/// landings squash them, fast motion stretches them and leaves trails.
pub struct BandBalls {
    balls: [Ball; MAX_BALLS],
}

impl BandBalls {
    pub fn new() -> Self {
        Self {
            balls: [Ball::default(); MAX_BALLS],
        }
    }
}

fn jump_speed(e: f32) -> f32 {
    (2.0 * GRAVITY * (0.06 + 0.9 * e)).sqrt()
}

impl Effect for BandBalls {
    fn name(&self) -> &'static str {
        "Band Balls"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.6);
        let n = if c.w < 32 {
            4
        } else if c.w < 90 {
            6
        } else {
            MAX_BALLS
        };
        let bins = f.spectrum.len();
        for (i, b) in self.balls.iter_mut().take(n).enumerate() {
            let (lo, hi) = (i * bins / n, ((i + 1) * bins / n).max(i * bins / n + 1));
            let e = if bins == 0 {
                0.0
            } else {
                let s = &f.spectrum[lo.min(bins - 1)..hi.min(bins)];
                s.iter().sum::<f32>() / s.len().max(1) as f32
            };
            let onset = e - b.slow;
            b.slow += (e - b.slow) * (1.0 - (-6.0 * dt).exp());

            b.vy -= GRAVITY * dt;
            b.y += b.vy * dt;
            if b.y <= 0.0 {
                b.y = 0.0;
                if b.vy < 0.0 {
                    b.squash = (-b.vy * 0.12).min(0.4);
                    b.vy = jump_speed(e);
                }
            }
            if onset > 0.12 && b.vy < jump_speed(e) * 0.7 {
                b.vy = jump_speed(e);
            }
            b.y = b.y.min(1.2);
            b.squash *= (-12.0 * dt).exp();
        }
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        let (wf, hf) = (w as f32, h as f32);
        let floor = hf * 0.86;

        // The floor is redrawn every frame so the trails only smear the air.
        let floor_col = hsv(0.65, 0.6, 0.12 + 0.15 * f.pulse);
        for y in (floor as usize).min(h)..h {
            let k = 1.0 - (y as f32 - floor) / (hf - floor).max(1.0) * 0.7;
            for x in 0..w {
                c.set(x, y, scale(floor_col, k));
            }
        }
        if (floor as usize) < h {
            let line = hsv(0.55, 0.5, 0.4 + 0.5 * f.pulse);
            for x in 0..w {
                c.set(x, floor as usize, line);
            }
        }

        let slot = wf / n as f32;
        let r = (slot * 0.34).min(hf * 0.13).max(0.8);
        let travel = (floor - 2.0 * r - hf * 0.03).max(1.0);
        for (i, b) in self.balls.iter().take(n).enumerate() {
            let hue = i as f32 / n as f32 * 0.8 + t * 0.02;
            let stretch = b.squash - (b.vy.abs() * 0.05).min(0.2);
            let (rx, ry) = (r * (1.0 + stretch), r * (1.0 - stretch));
            let cx = (i as f32 + 0.5) * slot;
            let cy = floor - ry - b.y.min(1.0) * travel;

            // Shadow shrinks as the ball rises.
            let sw = r * (1.3 - 0.7 * b.y.min(1.0));
            let sy = floor as i32 + 1;
            for dx in -(sw as i32)..=sw as i32 {
                let k = 1.0 - (dx as f32 / sw.max(1.0)).abs();
                c.plot(cx as i32 + dx, sy, scale(floor_col, 0.4 + 0.3 * (1.0 - k)));
            }

            let base = hsv(hue, 0.8, 1.0);
            let (x0, x1) = ((cx - rx).floor() as i32, (cx + rx).ceil() as i32);
            let (y0, y1) = ((cy - ry).floor() as i32, (cy + ry).ceil() as i32);
            for py in y0..=y1 {
                for px in x0..=x1 {
                    let nx = (px as f32 + 0.5 - cx) / rx;
                    let ny = (py as f32 + 0.5 - cy) / ry;
                    let d2 = nx * nx + ny * ny;
                    if d2 > 1.0 {
                        continue;
                    }
                    let nz = (1.0 - d2).sqrt();
                    let diffuse = (-0.45 * nx - 0.55 * ny + 0.7 * nz).max(0.0);
                    let spec = diffuse.powi(12) * 0.8;
                    let col = add(scale(base, 0.15 + 0.75 * diffuse), rgb(spec, spec, spec));
                    c.plot(px, py, mix(col, [255, 255, 255], f.pulse * 0.15));
                }
            }
        }
    }
}
