use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, scale};

/// A colorful tiled texture twisted around a wandering center. Pulse sets
/// the twist strength, bass scrolls the texture, mid widens the swirl.
pub struct Twirl {
    twist: f32,
    scroll: f32,
    hue: f32,
}

impl Twirl {
    pub fn new() -> Self {
        Self {
            twist: 0.0,
            scroll: 0.0,
            hue: 0.3,
        }
    }
}

impl Effect for Twirl {
    fn name(&self) -> &'static str {
        "Twirl"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        let target = 2.0 * (t * 0.3).sin() + 5.0 * f.pulse * (t * 0.13).cos().signum();
        self.twist += (target - self.twist) * (dt * 4.0).min(1.0);
        self.scroll += dt * (0.3 + f.bass * 1.5);
        self.hue += dt * 0.02;
        if c.w == 0 || c.h == 0 {
            return;
        }

        let unit = 6.0 / c.h as f32;
        let (w, h) = (c.w as f32 * unit, c.h as f32 * unit);
        let (cx, cy) = (w * (0.5 + 0.25 * (t * 0.37).sin()), h * (0.5 + 0.25 * (t * 0.29).cos()));
        let radius = h * (0.45 + 0.2 * f.mid);
        let inv_r2 = 1.0 / (radius * radius).max(1e-6);
        for y in 0..c.h {
            let dy = y as f32 * unit - cy;
            for x in 0..c.w {
                let dx = x as f32 * unit - cx;
                let k = 1.0 - (dx * dx + dy * dy) * inv_r2;
                let (u, v) = if k > 0.0 {
                    let (s, co) = (self.twist * k * k).sin_cos();
                    (dx * co - dy * s, dx * s + dy * co)
                } else {
                    (dx, dy)
                };
                let (u, v) = (u + cx + self.scroll, v + cy + self.scroll * 0.4);
                // Checker tiles with a hue per tile, a rounded inner bevel and thin borders.
                let (fu, fv) = (u - u.floor(), v - v.floor());
                let tile = u.floor() * 0.11 + v.floor() * 0.07;
                let border = fu.min(1.0 - fu).min(fv.min(1.0 - fv));
                let bevel = (border * 6.0).min(1.0);
                let odd = ((u.floor() as i64 + v.floor() as i64) & 1) == 1;
                let base = hsv(
                    self.hue + tile,
                    if odd { 0.85 } else { 0.5 },
                    if odd { 0.95 } else { 0.55 },
                );
                let glow = 0.75 + 0.25 * k.max(0.0);
                c.set(x, y, scale(base, (0.25 + 0.75 * bevel) * glow));
            }
        }
    }
}
