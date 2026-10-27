use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, rgb};

/// Old-school fire: heat is fed at the bottom and averaged upwards.
/// Bass feeds the flames, beats throw sparks, loudness sets the flame height.
pub struct Fire {
    heat: Vec<f32>,
    w: usize,
    h: usize,
    rng: Rng,
}

impl Fire {
    pub fn new() -> Self {
        Self { heat: Vec::new(), w: 0, h: 0, rng: Rng::new(0xF1AE) }
    }
}

impl Effect for Fire {
    fn name(&self) -> &'static str {
        "Fire"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, _dt: f32, f: &Features) {
        if (self.w, self.h) != (c.w, c.h) {
            (self.w, self.h) = (c.w, c.h);
            self.heat = vec![0.0; c.w * c.h];
        }
        let (w, h) = (self.w, self.h);
        if w == 0 || h < 3 {
            return;
        }

        let fuel = 0.35 + f.bass * 0.9 + f.pulse * 0.5;
        for x in 0..w {
            self.heat[(h - 1) * w + x] = (self.rng.f32() * fuel * 1.6).min(1.0);
        }
        if f.beat {
            for _ in 0..w / 6 {
                let x = self.rng.next_u32() as usize % w.saturating_sub(2).max(1);
                let y = h - 2 - self.rng.next_u32() as usize % (h / 4).max(1);
                for (dx, dy) in [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)] {
                    let (px, py) = (x + dx, y + dy);
                    if px < w && py < h {
                        self.heat[py * w + px] = 1.0;
                    }
                }
            }
        }

        // Rows are processed top-down so each reads last frame's rows below.
        let cool = 1.0 / (h as f32 * (0.35 + 0.5 * f.level.max(f.bass)));
        for y in 0..h - 1 {
            let below = (y + 1) * w;
            let below2 = if y + 2 < h { (y + 2) * w } else { below };
            for x in 0..w {
                let xl = x.saturating_sub(1);
                let xr = (x + 1).min(w - 1);
                let sum = self.heat[below + xl]
                    + self.heat[below + x]
                    + self.heat[below + xr]
                    + self.heat[below2 + x];
                let v = sum * 0.25 - cool * (0.5 + self.rng.f32());
                self.heat[y * w + x] = v.max(0.0);
            }
        }

        for y in 0..h {
            for x in 0..w {
                let v = self.heat[y * w + x] * 3.0;
                c.set(x, y, rgb(v, v - 1.0, v - 2.0));
            }
        }
    }
}
