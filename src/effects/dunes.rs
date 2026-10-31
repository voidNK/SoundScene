use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, mix, rgb, scale};

const LAYERS: usize = 4;
const MAX_GRAINS: usize = 900;

/// Blown sand grain, position in units of the canvas height.
struct Grain {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
}

/// Height 0..1 of a dune profile at phase `p`: a long gentle windward slope
/// up to a sharp crest at 0.7, then a steep slip face.
fn dune(p: f32) -> f32 {
    let t = p.rem_euclid(1.0);
    if t < 0.7 {
        let s = t / 0.7;
        s * s * (3.0 - 2.0 * s)
    } else {
        let s = (t - 0.7) / 0.3;
        1.0 - s * s * (3.0 - 2.0 * s) * 0.97
    }
}

/// Layer `i` profile: dunes of varying height. Returns height 0..~1.
fn profile(x: f32, i: usize, offset: f32) -> f32 {
    let freq = 0.8 + 0.35 * i as f32;
    let p = x * freq + offset + i as f32 * 0.37;
    let swell = 0.65 + 0.35 * (p * 0.37 + i as f32).sin();
    dune(p) * swell
}

/// Desert dunes under a blazing sky, shaded by the sun with gentle lit
/// windward slopes and dark slip faces. Sand blows off the crests and heat
/// shimmer wobbles the horizon; bass raises the wind and beats blow gusts
/// of sand.
pub struct Dunes {
    grains: Vec<Grain>,
    offset: f32,
    wind: f32,
    time: f32,
    rng: Rng,
}

impl Dunes {
    pub fn new() -> Self {
        Self {
            grains: Vec::new(),
            offset: 0.0,
            wind: 0.2,
            time: 0.0,
            rng: Rng::new(0xD0_4E5),
        }
    }

    /// Top of layer `i` at canvas column `x` in units of the height.
    fn top(&self, x: f32, i: usize) -> f32 {
        let base = 0.55 + 0.13 * i as f32;
        let amp = 0.08 + 0.06 * i as f32;
        base - amp * profile(x, i, self.offset * (0.4 + 0.3 * i as f32))
    }
}

impl Effect for Dunes {
    fn name(&self) -> &'static str {
        "Dunes"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let target = 0.2 + 0.8 * f.bass;
        self.wind += (target - self.wind) * (1.0 - (-dt * 3.0).exp());
        self.offset -= dt * 0.01 * (0.5 + self.wind);
        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if c.h > 0 { (w / h).min(20.0) } else { 1.0 };

        // Sand lifts off the crests of each layer.
        let gusts = if f.beat { 30.0 + 60.0 * f.bass } else { 0.0 };
        let rate = self.wind * self.wind * 120.0 * dt + gusts;
        let mut n = rate as usize + usize::from(self.rng.f32() < rate.fract());
        let cap = ((w * h / 30.0) as usize).clamp(40, MAX_GRAINS);
        while n > 0 && self.grains.len() < cap {
            n -= 1;
            let i = (self.rng.f32() * LAYERS as f32) as usize % LAYERS;
            // Pick a crest: scan for the local maximum near a random column.
            let mut x = self.rng.f32() * aspect;
            let step = 0.005;
            for _ in 0..80 {
                if self.top(x + step, i) < self.top(x, i) {
                    x += step;
                } else if self.top(x - step, i) < self.top(x, i) {
                    x -= step;
                } else {
                    break;
                }
            }
            let y = self.top(x, i);
            let (vx, vy) = (self.rng.range(0.1, 0.3), self.rng.range(-0.06, 0.0));
            self.grains.push(Grain {
                x,
                y,
                vx: vx * (0.5 + self.wind),
                vy,
                life: self.rng.range(0.6, 1.6),
            });
        }
        for g in &mut self.grains {
            g.x += g.vx * dt;
            g.y += g.vy * dt;
            g.vy += dt * 0.08;
            g.life -= dt;
        }
        self.grains.retain(|g| g.life > 0.0 && g.x < aspect + 0.1);

        if c.w == 0 || c.h == 0 {
            return;
        }

        // Sky.
        let sun = (w * 0.2, h * 0.12);
        for y in 0..c.h {
            let g = (y as f32 / (h * 0.55)).min(1.0);
            let sky = mix(rgb(0.25, 0.5, 0.85), rgb(0.95, 0.88, 0.75), g * g);
            for x in 0..c.w {
                let (dx, dy) = (x as f32 - sun.0, y as f32 - sun.1);
                let d = (dx * dx + dy * dy).sqrt() / h;
                let col = mix(
                    sky,
                    rgb(1.0, 1.0, 0.9),
                    (-d * 10.0).exp() + if d < 0.05 { 1.0 } else { 0.0 },
                );
                c.set(x, y, col);
            }
        }

        // Dunes far to near; the sun sits on the left, so slopes facing left
        // (rising with x) are lit.
        let lit = [
            rgb(0.95, 0.75, 0.5),
            rgb(1.0, 0.72, 0.42),
            rgb(0.98, 0.66, 0.35),
            rgb(0.95, 0.6, 0.3),
        ];
        let dark = [
            rgb(0.7, 0.55, 0.5),
            rgb(0.6, 0.4, 0.3),
            rgb(0.5, 0.3, 0.2),
            rgb(0.4, 0.22, 0.13),
        ];
        let ex = 1.0 / h.max(1.0);
        for i in 0..LAYERS {
            let depth = i as f32 / (LAYERS - 1) as f32;
            for x in 0..c.w {
                let u = x as f32 / h;
                let top = self.top(u, i);
                let y0 = ((top * h).max(0.0) as usize).min(c.h);
                for y in y0..c.h {
                    let fy = y as f32 / h;
                    // Shade by the surface slope a bit upwind of this pixel,
                    // so faces end in diagonal edges like seen in 3D.
                    let us = u - (fy - top) * 1.2;
                    let slope = (self.top(us + ex, i) - self.top(us - ex, i)) / (2.0 * ex);
                    let light = (0.6 - slope * 2.5).clamp(0.0, 1.0);
                    let col = mix(dark[i], lit[i], light);
                    // Wind ripples on the near layers.
                    let ripple = ((u * 60.0 + fy * 150.0 + u.sin() * 8.0).sin() * 0.5 + 0.5) * 0.12 * depth;
                    let below = ((fy - top) * 4.0).min(1.0);
                    c.set(x, y, scale(col, 1.0 - ripple - 0.15 * below * depth));
                }
            }
        }

        // Heat shimmer: wobble the rows around the horizon sideways.
        let (band0, band1) = ((h * 0.38) as usize, ((h * 0.6) as usize).min(c.h));
        let mut row = vec![[0u8; 3]; c.w];
        for y in band0.min(band1)..band1 {
            let k = 1.0 - ((y as f32 / h - 0.49) / 0.11).abs();
            let shift = k.max(0.0) * (h / 60.0) * (0.6 + self.wind) * (y as f32 * 0.9 + self.time * 7.0).sin();
            let (s0, frac) = (shift.floor() as i32, shift - shift.floor());
            for (x, p) in row.iter_mut().enumerate() {
                let sx = |o: i32| (x as i32 + o).clamp(0, c.w as i32 - 1) as usize;
                *p = mix(c.get(sx(s0), y), c.get(sx(s0 + 1), y), frac);
            }
            c.px[y * c.w..(y + 1) * c.w].copy_from_slice(&row);
        }

        for g in &self.grains {
            let k = (g.life * 1.5).min(1.0) * 0.35;
            c.plot_add((g.x * h) as i32, (g.y * h) as i32, scale(rgb(1.0, 0.85, 0.6), k));
        }
    }
}
