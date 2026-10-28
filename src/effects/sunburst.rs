use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, rgb};

const PALETTES: [([f32; 3], [f32; 3]); 5] = [
    ([0.98, 0.45, 0.08], [1.0, 0.88, 0.62]),
    ([0.85, 0.12, 0.15], [1.0, 0.8, 0.2]),
    ([0.05, 0.55, 0.55], [0.96, 0.9, 0.75]),
    ([0.75, 0.15, 0.55], [0.15, 0.1, 0.35]),
    ([0.2, 0.35, 0.85], [0.95, 0.75, 0.3]),
];
const COUNTS: [usize; 6] = [6, 8, 10, 12, 16, 20];

/// Retro sunburst: two-tone rays wheel around a wandering center with a
/// glowing sun in the middle and spectrum ripples running outward. Beats
/// pick the ray count from the bass and switch the palette every few beats,
/// bass and pulse sharpen the contrast and speed up the rotation.
pub struct Sunburst {
    rot: f32,
    rays: usize,
    beats: usize,
    palette: usize,
    colors: ([f32; 3], [f32; 3]),
}

impl Sunburst {
    pub fn new() -> Self {
        Self {
            rot: 0.0,
            rays: 12,
            beats: 0,
            palette: 0,
            colors: PALETTES[0],
        }
    }
}

impl Effect for Sunburst {
    fn name(&self) -> &'static str {
        "Sunburst"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.beats += 1;
            let i = ((f.bass * COUNTS.len() as f32) as usize).min(COUNTS.len() - 1);
            self.rays = COUNTS[i];
            if self.beats.is_multiple_of(8) {
                self.palette = (self.palette + 1) % PALETTES.len();
            }
        }
        let k = 1.0 - (-3.0 * dt).exp();
        let target = PALETTES[self.palette];
        for j in 0..3 {
            self.colors.0[j] += (target.0[j] - self.colors.0[j]) * k;
            self.colors.1[j] += (target.1[j] - self.colors.1[j]) * k;
        }
        self.rot += dt * (0.15 + 0.9 * f.bass + 0.6 * f.pulse);

        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * (0.5 + 0.18 * (t * 0.37).sin()), h * (0.5 + 0.15 * (t * 0.29).cos()));
        let m = w.min(h).max(1.0);
        let rmax = (w * w + h * h).sqrt().max(1.0);
        let n = self.rays as f32;
        let contrast = (0.35 + 0.45 * f.bass + 0.3 * f.pulse).min(1.0);
        let curl = 0.25 * (t * 0.11).sin();
        let sun = m * (0.05 + 0.03 * f.bass + 0.04 * f.pulse);
        let (ca, cb) = self.colors;
        let bins = f.spectrum.len();

        for y in 0..c.h {
            let dy = y as f32 + 0.5 - cy;
            for x in 0..c.w {
                let dx = x as f32 + 0.5 - cx;
                let r = (dx * dx + dy * dy).sqrt();
                let a = dy.atan2(dx) + self.rot + curl * (r / m * 4.0);
                // Antialiased two-tone stripes; near the center they blend to the mean.
                let s = (a * n * 0.5).sin() * (r / (n * 0.6)).max(0.0);
                let tone = 0.5 + 0.5 * contrast * s.clamp(-1.0, 1.0);
                let ripple = if bins > 0 {
                    f.spectrum[((r / rmax * bins as f32 * 1.6) as usize).min(bins - 1)]
                } else {
                    0.0
                };
                let vign = (1.0 - r / rmax * 0.9) * (0.75 + 0.35 * ripple);
                let glow = (sun / r.max(0.5)).powi(2).min(1.0) * 0.85;
                let ch = |j: usize| (ca[j] + (cb[j] - ca[j]) * tone) * vign + glow;
                c.set(x, y, rgb(ch(0), ch(1), ch(2)));
            }
        }
    }
}
