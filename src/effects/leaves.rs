use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, mix, rgb, scale};
use std::f32::consts::TAU;

const LEAVES: usize = 90;

/// Position in units of the canvas height.
struct Leaf {
    x: f32,
    y: f32,
    /// 0.4 (far) .. 1.0 (near), scales size, speed and brightness.
    depth: f32,
    /// Tilt out of the screen plane and spin within it.
    tilt: f32,
    spin: f32,
    tilt_rate: f32,
    spin_rate: f32,
    sway: f32,
    color: Rgb,
    /// Lobed maple-like outline instead of a plain oval.
    lobed: bool,
}

fn hash(n: u32) -> f32 {
    let mut n = n.wrapping_mul(0x9E37_79B9);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 13;
    (n & 0xFFFF) as f32 / 65535.0
}

/// Autumn leaves tumbling down through hazy golden light, each one turning
/// in 3D so it flashes its face, shows its darker back or goes edge-on while
/// fluttering side to side. Bass blows gusts that sweep them sideways and
/// spin them faster, above a colorful carpet of fallen leaves.
pub struct Leaves {
    leaves: Vec<Leaf>,
    wind: f32,
    gust: f32,
    time: f32,
    rng: Rng,
}

impl Leaves {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x1E_AF0F);
        let mut leaves: Vec<Leaf> = (0..LEAVES)
            .map(|_| {
                let y = rng.range(-0.2, 1.0);
                Self::make(&mut rng, y)
            })
            .collect();
        leaves.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        Self {
            leaves,
            wind: 0.0,
            gust: 0.0,
            time: 0.0,
            rng,
        }
    }

    fn make(rng: &mut Rng, y: f32) -> Leaf {
        let palette = [
            rgb(0.9, 0.35, 0.05),
            rgb(0.95, 0.6, 0.1),
            rgb(0.75, 0.15, 0.05),
            rgb(0.95, 0.8, 0.2),
            rgb(0.55, 0.25, 0.08),
        ];
        Leaf {
            x: rng.range(-0.2, 3.0),
            y,
            depth: rng.range(0.4, 1.0),
            tilt: rng.range(0.0, TAU),
            spin: rng.range(0.0, TAU),
            tilt_rate: rng.range(1.0, 3.0) * if rng.f32() < 0.5 { 1.0 } else { -1.0 },
            spin_rate: rng.range(-1.5, 1.5),
            sway: rng.range(0.0, TAU),
            color: palette[rng.next_u32() as usize % palette.len()],
            lobed: rng.f32() < 0.5,
        }
    }
}

impl Effect for Leaves {
    fn name(&self) -> &'static str {
        "Leaves"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if c.h > 0 { (w / h).clamp(0.2, 10.0) } else { 1.0 };
        if f.beat {
            self.gust += 0.3 + 0.6 * f.bass;
        }
        self.gust = (self.gust * (-dt * 1.0).exp()).min(1.5);
        let target = 0.05 + 0.3 * f.bass + self.gust + 0.05 * (self.time * 0.2).sin();
        self.wind += (target - self.wind) * (1.0 - (-dt * 2.0).exp());

        let wind = self.wind;
        for l in &mut self.leaves {
            let spin_boost = 1.0 + 2.0 * wind;
            l.tilt += dt * l.tilt_rate * spin_boost;
            l.spin += dt * (l.spin_rate + l.tilt_rate.signum() * wind);
            l.sway += dt * 1.5;
            // Edge-on leaves fall faster, flat ones glide.
            let flat = l.tilt.cos().abs();
            let fall = (0.06 + 0.08 * (1.0 - flat)) * l.depth;
            l.y += dt * fall;
            l.x += dt * (l.sway.sin() * 0.12 * flat + wind * 0.5) * l.depth;
            if l.y > 1.05 || l.x > aspect + 0.15 || l.x < -0.3 {
                let depth = l.depth;
                *l = Self::make(&mut self.rng, -0.1);
                l.depth = depth;
                // Re-enter at the top, or upwind on the left during gusts.
                if wind > 0.3 && self.rng.f32() < 0.5 {
                    l.x = -0.1;
                    l.y = self.rng.range(0.0, 0.8);
                } else {
                    l.x = self.rng.range(-0.1, aspect);
                }
            }
        }

        if c.w == 0 || c.h == 0 {
            return;
        }

        // Golden haze with blurry tree shapes and a leaf carpet below.
        let ground = h * 0.88;
        for y in 0..c.h {
            let g = y as f32 / h;
            let sky = mix(rgb(0.95, 0.7, 0.35), rgb(0.35, 0.2, 0.15), g);
            for x in 0..c.w {
                let u = x as f32 / h;
                let trunk = ((u * 2.3 + 0.5).sin() * (u * 1.1).cos()).max(0.0).powi(8);
                c.set(x, y, mix(sky, rgb(0.25, 0.12, 0.08), trunk * 0.5));
            }
        }
        for y in (ground.max(0.0) as usize).min(c.h)..c.h {
            for x in 0..c.w {
                let n = hash((x / 2) as u32 * 7919 + (y / 2) as u32 * 104_729);
                let col = [rgb(0.6, 0.2, 0.05), rgb(0.75, 0.45, 0.08), rgb(0.45, 0.15, 0.05)][(n * 2.99) as usize];
                let k = 0.6 + 0.4 * hash((x * 31 + y * 17) as u32);
                c.set(x, y, scale(col, k));
            }
        }

        for l in &self.leaves {
            let len = h * 0.07 * l.depth * if l.lobed { 1.1 } else { 1.0 };
            let ct = l.tilt.cos();
            let (cs, ss) = (l.spin.cos(), l.spin.sin());
            // Front lit, back darker; far leaves sink into the haze.
            let face = if ct >= 0.0 { 1.0 } else { 0.6 };
            let light = face * (0.5 + 0.5 * ct.abs());
            let col = mix(scale(l.color, light), rgb(0.85, 0.65, 0.4), (1.0 - l.depth) * 0.6);
            let vein = scale(col, 0.7);
            let (ox, oy) = (l.x * h, l.y * h);
            let ri = len.ceil() as i32 + 1;
            let (ix, iy) = (ox.round() as i32, oy.round() as i32);
            let thin = ct.abs().max(0.15);
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let (px, py) = ((ix + dx) as f32 - ox, (iy + dy) as f32 - oy);
                    // Undo the spin, then the tilt (which squashes one axis).
                    let u = (px * cs + py * ss) / len;
                    let v = (-px * ss + py * cs) / len / thin;
                    let inside = if l.lobed {
                        let r = (u * u + v * v).sqrt();
                        let a = v.atan2(u);
                        r < 0.5 + 0.4 * (5.0 * a).cos().max(0.0).sqrt()
                    } else {
                        (-1.0..=1.0).contains(&u) && v.abs() < 0.4 * (1.0 - u * u).sqrt() * (1.0 - 0.2 * u)
                    };
                    if inside {
                        let on_vein = v.abs() * thin < 0.05 && u > -0.9;
                        c.plot(ix + dx, iy + dy, if on_vein { vein } else { col });
                    }
                }
            }
            if len < 1.5 {
                c.plot(ix, iy, col);
            }
        }
    }
}
