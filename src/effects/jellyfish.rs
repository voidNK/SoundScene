use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, rgb, scale};
use std::f32::consts::{PI, TAU};

const JELLIES: usize = 6;
const SNOW: usize = 160;
const TENTACLES: usize = 7;

/// Position in units of the canvas height, `size` is the bell radius.
struct Jelly {
    x: f32,
    y: f32,
    vy: f32,
    size: f32,
    hue: f32,
    phase: f32,
    /// Bell contraction 0 (relaxed) .. 1 (squeezed).
    squeeze: f32,
    drift: f32,
}

/// Glowing translucent jellyfish drifting through the deep sea among marine
/// snow. Their bells contract with the bass, each squeeze pushing them up,
/// and long tentacles trail and wave behind; beats light them up.
pub struct Jellyfish {
    jellies: Vec<Jelly>,
    /// Marine snow as fractions of the canvas.
    snow: Vec<(f32, f32, f32)>,
    time: f32,
    rng: Rng,
}

impl Jellyfish {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x1E11_7F15);
        let jellies = (0..JELLIES)
            .map(|i| Jelly {
                x: (i as f32 + 0.5) / JELLIES as f32 * 1.8,
                y: rng.range(0.2, 1.0),
                vy: 0.0,
                size: rng.range(0.09, 0.15),
                hue: [0.55, 0.8, 0.95, 0.5, 0.7, 0.12][i],
                phase: rng.range(0.0, TAU),
                squeeze: 0.0,
                drift: rng.range(-0.03, 0.03),
            })
            .collect();
        let snow = (0..SNOW).map(|_| (rng.f32(), rng.f32(), rng.f32())).collect();
        Self {
            jellies,
            snow,
            time: 0.0,
            rng,
        }
    }
}

impl Effect for Jellyfish {
    fn name(&self) -> &'static str {
        "Jellyfish"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let now = self.time;
        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if c.h > 0 { w / h } else { 1.0 };

        for j in &mut self.jellies {
            // Own slow rhythm, overridden by the bass.
            let own = (0.5 + 0.5 * (now * 1.1 + j.phase).sin()).powi(4) * 0.6;
            let target = own.max(f.bass * 0.9 + f.pulse * 0.3).min(1.0);
            let prev = j.squeeze;
            let rate = if target > j.squeeze { 12.0 } else { 3.0 };
            j.squeeze += (target - j.squeeze) * (1.0 - (-dt * rate).exp());
            // Squeezing thrusts upwards, otherwise they slowly sink.
            j.vy -= (j.squeeze - prev).max(0.0) * j.size * 1.5;
            j.vy += dt * 0.03;
            j.vy *= (-dt * 1.2).exp();
            j.y += j.vy * dt;
            j.x += (j.drift + 0.02 * (now * 0.2 + j.phase).sin()) * dt;
            let span = aspect + 0.4;
            j.x = (j.x + 0.2).rem_euclid(span.max(0.1)) - 0.2;
            if j.y < -0.6 {
                j.y = 1.0 + j.size;
                j.x = self.rng.range(0.0, aspect.max(0.1));
            } else if j.y > 1.0 + j.size * 2.0 {
                j.y = -0.5;
            }
        }
        for s in &mut self.snow {
            s.1 = (s.1 + dt * (0.01 + 0.02 * s.2)).rem_euclid(1.0);
            s.0 = (s.0 + dt * 0.005 * (now * 0.3 + s.2 * 9.0).sin()).rem_euclid(1.0);
        }

        if c.w == 0 || c.h == 0 {
            return;
        }

        for y in 0..c.h {
            let g = y as f32 / h;
            let col = mix(rgb(0.0, 0.06, 0.12), rgb(0.0, 0.0, 0.03), g);
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }
        let visible = ((w * h / 150.0) as usize).clamp(8, SNOW);
        for &(sx, sy, k) in self.snow.iter().take(visible) {
            c.plot_add(
                (sx * w) as i32,
                (sy * h) as i32,
                scale(rgb(0.5, 0.6, 0.6), 0.2 + 0.3 * k),
            );
        }

        let glow = 0.7 + 0.5 * f.pulse + 0.3 * f.level;
        for j in &self.jellies {
            let col = hsv(j.hue, 0.6, 1.0);
            let core = hsv(j.hue + 0.05, 0.3, 1.0);
            let bw = j.size * h * (1.0 - 0.28 * j.squeeze);
            let bh = j.size * h * (0.75 + 0.3 * j.squeeze);
            let (cx, cy) = (j.x * h, j.y * h);

            // Tentacles hang from the rim and lag behind the motion.
            let tlen = j.size * h * (2.5 + 0.8 * (1.0 - j.squeeze));
            for k in 0..TENTACLES {
                let u = (k as f32 + 0.5) / TENTACLES as f32 * 2.0 - 1.0;
                let steps = tlen.ceil().clamp(2.0, 300.0) as usize;
                for s in 0..steps {
                    let v = s as f32 / steps as f32;
                    let wave = (now * 2.0 - v * 6.0 + k as f32 * 1.3 + j.phase).sin();
                    let x = cx + u * bw * 0.85 * (1.0 - 0.3 * v) + wave * v * bw * 0.35;
                    let y = cy + v * tlen + j.vy * h * v * v * 0.6;
                    let b = (1.0 - v) * 0.5 * glow;
                    c.plot_add(x.round() as i32, y.round() as i32, scale(col, b));
                }
            }
            // Frilly oral arms in the middle.
            for k in 0..3 {
                let steps = (tlen * 0.6).ceil().clamp(2.0, 200.0) as usize;
                for s in 0..steps {
                    let v = s as f32 / steps as f32;
                    let x = cx + (k as f32 - 1.0) * bw * 0.15 + (now * 3.0 - v * 10.0 + k as f32).sin() * bw * 0.12;
                    let y = cy + v * tlen * 0.6;
                    let b = (1.0 - v) * 0.6 * glow;
                    for dx in [-1, 0, 1] {
                        let kk = if dx == 0 { b } else { b * 0.3 };
                        c.plot_add(x.round() as i32 + dx, y.round() as i32, scale(core, kk));
                    }
                }
            }

            // Translucent dome: bright rim, ribbed, glowing core.
            let x0 = ((cx - bw * 1.6 - 1.0).max(0.0) as usize).min(c.w);
            let x1 = ((cx + bw * 1.6 + 2.0).max(0.0) as usize).min(c.w);
            let y0 = ((cy - bh * 1.6 - 1.0).max(0.0) as usize).min(c.h);
            let y1 = ((cy + bh * 0.6 + 2.0).max(0.0) as usize).min(c.h);
            for y in y0..y1 {
                for x in x0..x1 {
                    let u = (x as f32 + 0.5 - cx) / bw.max(0.5);
                    // Wavy skirt along the bottom edge.
                    let skirt = 0.12 * (u * PI * 4.0 + now * 4.0).sin().abs() + 0.15 * j.squeeze;
                    let v = (y as f32 + 0.5 - cy) / bh.max(0.5);
                    let d = (u * u + v.min(0.0).powi(2)).sqrt();
                    if d > 1.0 || v > skirt {
                        // Soft halo around the bell.
                        let dd = (u * u + v * v * 0.6).sqrt();
                        if dd < 1.6 {
                            c.plot_add(x as i32, y as i32, scale(col, 0.12 * (1.6 - dd) * glow));
                        }
                        continue;
                    }
                    let rim = (-((1.0 - d) / 0.15).powi(2)).exp();
                    let rib = 0.5 + 0.5 * (u * 9.0).cos();
                    let inner = (-((u * u + (v + 0.4).powi(2)) / 0.15)).exp();
                    let k = (0.12 + 0.5 * rim + 0.12 * rib * (1.0 - d) + 0.5 * inner) * glow;
                    c.plot_add(x as i32, y as i32, scale(mix(col, core, inner), k));
                }
            }
        }
    }
}
