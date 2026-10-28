use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};
use std::f32::consts::{FRAC_PI_2, TAU};

/// Polygon corners, jump ratio and whether the same corner may not be
/// picked twice in a row (needed for squares, which would fill otherwise).
const VARIANTS: [(usize, f32, bool); 6] = [
    (3, 0.5, false),
    (4, 0.5, true),
    (5, 0.382, false),
    (6, 0.333, false),
    (5, 0.5, true),
    (6, 0.4, true),
];

/// Chaos game: a point jumps part of the way towards a random polygon corner
/// and leaves a dot, which builds Sierpinski-like fractals. Each dot takes the
/// color of its corner. Beats switch to the next polygon (3 to 6 corners),
/// bass breathes the size, the dots fade so old shapes dissolve.
pub struct ChaosGame {
    variant: usize,
    cooldown: f32,
    rng: Rng,
    pt: (f32, f32),
    last: usize,
    rot: f32,
    hue: f32,
}

impl ChaosGame {
    pub fn new() -> Self {
        Self {
            variant: 0,
            cooldown: 0.0,
            rng: Rng::new(0xC4A05),
            pt: (0.0, 0.0),
            last: 0,
            rot: 0.0,
            hue: 0.0,
        }
    }
}

impl Effect for ChaosGame {
    fn name(&self) -> &'static str {
        "Chaos Game"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fade(0.86);
        self.cooldown -= dt;
        if f.beat && self.cooldown <= 0.0 {
            self.variant = (self.variant + 1) % VARIANTS.len();
            self.cooldown = 2.5;
            self.hue += 0.11;
        }
        self.rot += dt * (0.06 + 0.25 * f.mid);
        self.hue += dt * 0.02;

        let (k, ratio, no_repeat) = VARIANTS[self.variant];
        let corners: Vec<(f32, f32)> = (0..k)
            .map(|i| {
                let a = -FRAC_PI_2 + TAU * i as f32 / k as f32;
                (a.cos(), a.sin())
            })
            .collect();
        let colors: Vec<_> = (0..k).map(|i| hsv(self.hue + i as f32 / k as f32, 0.75, 1.0)).collect();

        let (w, h) = (c.w as f32, c.h as f32);
        let radius = w.min(h) * 0.47 * (0.88 + 0.12 * f.bass);
        let (cx, cy) = (w * 0.5, h * 0.5 + radius * 0.06);
        let (sn, cs) = self.rot.sin_cos();
        let points = (c.w * c.h / 2).clamp(100, 16_000);
        // Dot brightness scales with the pixel-to-dot ratio so density looks the
        // same on every canvas size.
        let gain = ((c.w * c.h) as f32 / points as f32 * (0.022 + 0.03 * f.pulse)).clamp(0.01, 1.0);

        let (mut x, mut y) = self.pt;
        for _ in 0..points {
            let mut i = self.rng.next_u32() as usize % k;
            if no_repeat && i == self.last {
                i = (i + 1 + self.rng.next_u32() as usize % (k - 1)) % k;
            }
            self.last = i;
            x += (corners[i].0 - x) * (1.0 - ratio);
            y += (corners[i].1 - y) * (1.0 - ratio);
            let px = cx + (x * cs - y * sn) * radius;
            let py = cy + (x * sn + y * cs) * radius;
            let col = colors[i];
            c.plot_add(
                px as i32,
                py as i32,
                [
                    (col[0] as f32 * gain) as u8,
                    (col[1] as f32 * gain) as u8,
                    (col[2] as f32 * gain) as u8,
                ],
            );
        }
        self.pt = (x, y);
    }
}
