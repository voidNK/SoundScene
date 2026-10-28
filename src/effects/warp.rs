use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, add, hsv, rgb, scale};

const STARS: usize = 450;

struct Streak {
    x: f32,
    y: f32,
    z: f32,
    hue: f32,
}

/// Hyperspace jump: long rainbow streaks tear outward from the vanishing
/// point, smeared by a radial zoom blur. Beats kick the ship into a surge
/// with a white flash, the bass sets the cruising speed.
pub struct Warp {
    stars: Vec<Streak>,
    rng: Rng,
    prev: Vec<Rgb>,
    boost: f32,
    flash: f32,
    hue: f32,
}

impl Warp {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x3A2B);
        let stars = (0..STARS)
            .map(|_| Streak {
                x: rng.range(-1.0, 1.0),
                y: rng.range(-1.0, 1.0),
                z: rng.range(0.05, 1.0),
                hue: rng.range(0.0, 1.0),
            })
            .collect();
        Self {
            stars,
            rng,
            prev: Vec::new(),
            boost: 0.0,
            flash: 0.0,
            hue: 0.55,
        }
    }
}

impl Effect for Warp {
    fn name(&self) -> &'static str {
        "Warp"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.boost = (self.boost + 0.9 + 1.2 * f.bass).min(3.0);
            self.flash = self.flash.max(0.35 + 0.65 * f.bass);
            self.hue += 0.08;
        }
        self.boost *= (-dt * 2.2).exp();
        self.flash *= (-dt * 6.0).exp();
        self.hue += dt * 0.02;
        let speed = 0.15 + 0.5 * f.bass + 0.3 * f.level + self.boost;
        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);

        // Radial zoom blur: each pixel takes some light from closer to the center.
        self.prev.clone_from(&c.px);
        let zoom = 0.9 - 0.04 * self.boost.min(1.5);
        for y in 0..c.h {
            let sy = ((cy + (y as f32 + 0.5 - cy) * zoom) as usize).min(c.h.saturating_sub(1));
            for x in 0..c.w {
                let sx = ((cx + (x as f32 + 0.5 - cx) * zoom) as usize).min(c.w.saturating_sub(1));
                let i = y * c.w + x;
                c.px[i] = add(scale(self.prev[i], 0.45), scale(self.prev[sy * c.w + sx], 0.42));
            }
        }

        let sc = w.max(h) * 0.5;
        let tail = (0.05 + speed * 0.12).min(0.6);
        let sat = (0.85 - 0.25 * speed).clamp(0.25, 0.85);
        for s in &mut self.stars {
            s.z -= speed * dt;
            let x1 = cx + s.x / s.z.max(0.01) * sc;
            let y1 = cy + s.y / s.z.max(0.01) * sc;
            if s.z <= 0.01 || !(-w..2.0 * w).contains(&x1) || !(-h..2.0 * h).contains(&y1) {
                s.x = self.rng.range(-1.0, 1.0);
                s.y = self.rng.range(-1.0, 1.0);
                s.z = self.rng.range(0.6, 1.0);
                s.hue = self.rng.range(0.0, 1.0);
                continue;
            }
            let z0 = s.z + tail;
            let x0 = cx + s.x / z0 * sc;
            let y0 = cy + s.y / z0 * sc;
            let near = (1.0 - s.z).clamp(0.0, 1.0);
            let head = hsv(self.hue + s.hue * 0.35, sat, 0.25 + 0.75 * near);
            let back = hsv(self.hue + s.hue * 0.35 + 0.15, 1.0, 0.1 + 0.2 * near);
            let steps = (x1 - x0).abs().max((y1 - y0).abs()).ceil().clamp(1.0, 600.0) as i32;
            for i in 0..=steps {
                let k = i as f32 / steps as f32;
                let col = crate::gfx::mix(back, head, k * k);
                c.plot_add(
                    (x0 + (x1 - x0) * k).round() as i32,
                    (y0 + (y1 - y0) * k).round() as i32,
                    col,
                );
            }
        }

        // Vanishing-point glow and beat flash.
        let rmax = w.min(h) * 0.3;
        if rmax < 1.0 {
            return;
        }
        // The blur keeps most of the previous frame, so the glow builds up over a few frames.
        let glow = 0.03 + 0.05 * f.level + 0.06 * self.boost.min(1.0);
        let tint = hsv(self.hue + 0.5, 0.4, 1.0);
        let flash = self.flash * 0.12;
        for y in 0..c.h {
            let dy = y as f32 + 0.5 - cy;
            for x in 0..c.w {
                let dx = x as f32 + 0.5 - cx;
                let d2 = (dx * dx + dy * dy) / (rmax * rmax);
                let g = (-d2 * 6.0).exp() * glow + flash;
                if g > 0.004 {
                    let i = y * c.w + x;
                    let add_c = rgb(
                        g * tint[0] as f32 / 255.0,
                        g * tint[1] as f32 / 255.0,
                        g * tint[2] as f32 / 255.0,
                    );
                    c.px[i] = add(c.px[i], add_c);
                }
            }
        }
    }
}
