use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, scale};

const BALLS: usize = 14;
const GRAVITY: f32 = 1.8;
const BOUNCE: f32 = 0.88;
const SUBSTEP: f32 = 1.0 / 240.0;

struct Ball {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r: f32,
    hue: f32,
    band: usize,
}

/// Glowing balls with gravity, wall bounces and elastic collisions, drawn
/// with motion blur. Beats kick them upwards (harder with more bass) and
/// each ball's glow follows one spectrum band.
pub struct BouncingBalls {
    balls: Vec<Ball>,
    rng: Rng,
    hue: f32,
}

impl BouncingBalls {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xBA11);
        let balls = (0..BALLS)
            .map(|i| Ball {
                x: rng.range(0.1, 1.6),
                y: rng.range(0.1, 0.6),
                vx: rng.range(-0.6, 0.6),
                vy: rng.range(-0.5, 0.5),
                r: rng.range(0.035, 0.08),
                hue: i as f32 / BALLS as f32,
                band: (i * 5 + 2) % 60,
            })
            .collect();
        Self { balls, rng, hue: 0.0 }
    }

    fn physics(&mut self, dt: f32, aspect: f32) {
        for b in &mut self.balls {
            b.vy += GRAVITY * dt;
            b.x += b.vx * dt;
            b.y += b.vy * dt;
            let (xmin, xmax) = (b.r.min(aspect * 0.5), (aspect - b.r).max(aspect * 0.5));
            if b.x < xmin {
                b.x = xmin;
                b.vx = b.vx.abs() * BOUNCE;
            } else if b.x > xmax {
                b.x = xmax;
                b.vx = -b.vx.abs() * BOUNCE;
            }
            if b.y > 1.0 - b.r {
                b.y = 1.0 - b.r;
                b.vy = -b.vy.abs() * BOUNCE;
                b.vx *= 0.995;
            } else if b.y < b.r {
                b.y = b.r;
                b.vy = b.vy.abs() * BOUNCE;
            }
        }
        let n = self.balls.len();
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = self.balls.split_at_mut(j);
                let (a, b) = (&mut a[i], &mut b[0]);
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let d2 = dx * dx + dy * dy;
                let rr = a.r + b.r;
                if d2 >= rr * rr || d2 < 1e-12 {
                    continue;
                }
                let d = d2.sqrt();
                let (nx, ny) = (dx / d, dy / d);
                let (ma, mb) = (a.r * a.r, b.r * b.r);
                let total = ma + mb;
                // Push apart by mass, then exchange momentum along the normal.
                let overlap = rr - d;
                a.x -= nx * overlap * mb / total;
                a.y -= ny * overlap * mb / total;
                b.x += nx * overlap * ma / total;
                b.y += ny * overlap * ma / total;
                let rel = (b.vx - a.vx) * nx + (b.vy - a.vy) * ny;
                if rel < 0.0 {
                    let j = -(1.0 + BOUNCE) * rel / (1.0 / ma + 1.0 / mb);
                    a.vx -= j / ma * nx;
                    a.vy -= j / ma * ny;
                    b.vx += j / mb * nx;
                    b.vy += j / mb * ny;
                }
            }
        }
        for b in &mut self.balls {
            b.vx = b.vx.clamp(-4.0, 4.0);
            b.vy = b.vy.clamp(-5.0, 5.0);
        }
    }
}

impl Effect for BouncingBalls {
    fn name(&self) -> &'static str {
        "Bouncing Balls"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        c.fade(0.55);
        let unit = c.h as f32;
        let aspect = c.w as f32 / unit;

        self.hue += dt * 0.02;
        if f.beat {
            self.hue += 0.05;
            for b in &mut self.balls {
                b.vy -= (0.7 + 1.3 * f.bass) * self.rng.range(0.6, 1.2);
                b.vx += self.rng.range(-0.5, 0.5);
            }
        }

        let steps = (dt / SUBSTEP).ceil().clamp(1.0, 30.0) as usize;
        for _ in 0..steps {
            self.physics(dt / steps as f32, aspect);
        }

        let bins = f.spectrum.len();
        for pass in 0..2 {
            for b in &self.balls {
                let band = if bins > 0 { f.spectrum[b.band % bins] } else { 0.0 };
                let (cx, cy, r) = (b.x * unit, b.y * unit, (b.r * unit).max(0.5));
                let reach = if pass == 0 { r * (2.0 + 1.5 * band) } else { r };
                let (x0, x1) = ((cx - reach).floor().max(0.0), (cx + reach).ceil().min(c.w as f32 - 1.0));
                let (y0, y1) = ((cy - reach).floor().max(0.0), (cy + reach).ceil().min(c.h as f32 - 1.0));
                if x1 < x0 || y1 < y0 {
                    continue;
                }
                let hue = self.hue + b.hue;
                let glow = hsv(hue, 0.8, 0.35 + 0.65 * band);
                for y in y0 as usize..=y1 as usize {
                    for x in x0 as usize..=x1 as usize {
                        let (dx, dy) = ((x as f32 + 0.5 - cx) / r, (y as f32 + 0.5 - cy) / r);
                        let d2 = dx * dx + dy * dy;
                        if pass == 0 {
                            if d2 >= 1.0 {
                                let k = (0.45 * (1.0 / d2 - (r / reach).powi(2))).max(0.0);
                                c.plot_add(x as i32, y as i32, scale(glow, k));
                            }
                        } else if d2 < 1.0 {
                            // Shaded sphere with a highlight towards the top left.
                            let z = (1.0 - d2).sqrt();
                            let light = (z * 0.6 - dx * 0.3 - dy * 0.4).clamp(0.0, 1.0);
                            let spec = light.powi(12);
                            let col = hsv(hue, 0.75 * (1.0 - spec), (0.25 + 0.75 * light + 0.3 * band).min(1.0));
                            c.set(x, y, col);
                        }
                    }
                }
            }
        }
    }
}
