use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, mix, rgb};
use std::f32::consts::{PI, TAU};

const IMAGES: u32 = 3;

/// A station briefly coming through while tuning.
struct Fragment {
    image: u32,
    age: f32,
    life: f32,
    roll: f32,
    roll_speed: f32,
    skew: f32,
}

/// Analog TV snow between channels: horizontally smeared noise, rolling
/// brightness bars and white streaks. Loudness sets the noise intensity and
/// bass the bar speed; now and then, and more often on beats, a torn and
/// rolling picture flickers through as if a station were almost tuned in.
pub struct Static {
    rng: Rng,
    bars: f32,
    fragment: Option<Fragment>,
    flash: f32,
}

impl Static {
    pub fn new() -> Self {
        Self {
            rng: Rng::new(0x5A0),
            bars: 0.0,
            fragment: None,
            flash: 0.0,
        }
    }
}

/// The pictures that come through, sampled at u, v in 0..1.
fn image(kind: u32, u: f32, v: f32, t: f32) -> Rgb {
    match kind {
        0 => {
            const BARS: [Rgb; 7] = [
                [200, 200, 200],
                [200, 200, 0],
                [0, 200, 200],
                [0, 200, 0],
                [200, 0, 200],
                [200, 0, 0],
                [0, 0, 200],
            ];
            if v < 0.7 {
                BARS[((u * 7.0) as usize).min(6)]
            } else if v < 0.78 {
                BARS[6 - ((u * 7.0) as usize).min(6)]
            } else {
                let g = if u < 0.6 { 0.05 } else { 0.9 };
                rgb(g, g, g)
            }
        }
        1 => {
            // Sunset: sky gradient, striped sun, grid floor.
            let horizon = 0.62;
            if v < horizon {
                let (dx, dy) = ((u - 0.5) * 1.8, v - horizon + 0.05);
                let d = (dx * dx + dy * dy).sqrt();
                let cut = v > 0.4 && ((v * 40.0) as i32) % 3 == 0;
                if d < 0.25 && !cut {
                    mix([255, 230, 80], [255, 60, 120], (v - 0.3) / 0.3)
                } else {
                    mix([20, 10, 70], [230, 70, 120], v / horizon)
                }
            } else {
                let depth = (v - horizon) / (1.0 - horizon);
                let gx = ((u - 0.5) / (depth + 0.05) * 4.0).rem_euclid(1.0);
                let gz = (1.0 / (depth + 0.05) + t * 3.0).rem_euclid(1.0);
                if gx < 0.08 || gz < 0.12 {
                    [240, 60, 220]
                } else {
                    [25, 5, 40]
                }
            }
        }
        _ => {
            // Target rings.
            let (dx, dy) = ((u - 0.5) * 2.0, (v - 0.5) * 1.2);
            let d = (dx * dx + dy * dy).sqrt();
            if d > 0.95 {
                [30, 30, 40]
            } else {
                hsv(
                    d * 0.8 + 0.6,
                    0.85,
                    if (d * 10.0) as i32 % 2 == 0 { 0.95 } else { 0.25 },
                )
            }
        }
    }
}

impl Effect for Static {
    fn name(&self) -> &'static str {
        "Static"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (w, h) = (c.w as f32, c.h as f32);

        self.bars = (self.bars + dt * (0.08 + f.bass * 0.35)).rem_euclid(1.0);
        self.flash = (self.flash - dt * 4.0).max(0.0);
        if f.beat {
            self.flash = 1.0;
        }
        let chance = dt * 0.12 + if f.beat { 0.3 } else { 0.0 };
        if self.fragment.is_none() && self.rng.f32() < chance {
            self.fragment = Some(Fragment {
                image: self.rng.next_u32() % IMAGES,
                age: 0.0,
                life: self.rng.range(0.4, 1.4),
                roll: self.rng.f32(),
                roll_speed: self.rng.range(-1.5, 1.5),
                skew: self.rng.range(0.02, 0.12),
            });
        }
        let mut strength = 0.0;
        if let Some(fr) = &mut self.fragment {
            fr.age += dt;
            fr.roll = (fr.roll + fr.roll_speed * dt).rem_euclid(1.0);
            strength = (fr.age / fr.life * PI).sin().max(0.0) * 0.85;
            if fr.age >= fr.life {
                self.fragment = None;
            }
        }

        let intensity = 0.3 + 0.7 * f.level + 0.2 * self.flash;
        let tint = f.treble * 0.25;
        for y in 0..c.h {
            let v = y as f32 / h;
            let bar = 0.7 + 0.3 * ((v * 1.5 - self.bars) * TAU).sin();
            let line = self.rng.range(0.8, 1.2);
            let streak = self.rng.f32() < 0.004 + f.treble * 0.02;
            let (s0, s1) = if streak {
                let a = self.rng.range(0.0, w);
                (a, a + self.rng.range(4.0, w * 0.4))
            } else {
                (0.0, 0.0)
            };
            // A picture comes in clean on some lines and torn on others.
            let (frag, sv, shift) = match &self.fragment {
                Some(fr) => {
                    let sv = (v + fr.roll).rem_euclid(1.0);
                    let tear = (sv * 9.0 + t * 5.0).sin() * fr.skew + if line > 1.15 { fr.skew * 2.0 } else { 0.0 };
                    (Some(fr.image), sv, tear)
                }
                None => (None, 0.0, 0.0),
            };
            let mut prev = 0.5;
            for x in 0..c.w {
                let n = 0.55 * self.rng.f32() + 0.45 * prev;
                prev = n;
                let g = (n * 1.3 * intensity * bar * line).min(1.0);
                let mut col = if tint > 0.0 {
                    let (r, b) = (self.rng.range(-tint, tint), self.rng.range(-tint, tint));
                    rgb(g + r * g, g, g * 1.05 + b * g)
                } else {
                    rgb(g, g, g * 1.05)
                };
                let xf = x as f32;
                if streak && xf >= s0 && xf < s1 {
                    col = mix(col, [255, 255, 255], 0.8);
                }
                if let Some(kind) = frag {
                    let u = (xf / w + shift).rem_euclid(1.0);
                    col = mix(col, image(kind, u, sv, t), strength * (0.6 + 0.4 * n));
                }
                c.set(x, y, col);
            }
        }
    }
}
