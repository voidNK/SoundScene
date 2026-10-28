use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, scale};
use std::f32::consts::TAU;

/// Petal ratios k = n / d for r = cos(k θ).
const RATIOS: [(u32, u32); 14] = [
    (2, 1),
    (3, 1),
    (4, 1),
    (5, 1),
    (6, 1),
    (7, 1),
    (3, 2),
    (5, 2),
    (7, 2),
    (4, 3),
    (5, 3),
    (7, 3),
    (5, 4),
    (7, 4),
];
const LAYERS: usize = 3;
const MORPH_TIME: f32 = 0.7;

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

struct Layer {
    from: usize,
    to: usize,
    morph: f32,
    rot: f32,
    hue: f32,
}

/// Glowing rhodonea curves r = cos(k θ): three nested roses that rotate in
/// opposite directions and morph smoothly into a new petal count whenever a
/// beat hits. Bass pumps their size, the waveform ripples along the lines.
pub struct Rose {
    layers: Vec<Layer>,
    next: usize,
    idle: f32,
    rng: Rng,
}

impl Rose {
    pub fn new() -> Self {
        let layers = (0..LAYERS)
            .map(|i| Layer {
                from: i * 4,
                to: i * 4,
                morph: 1.0,
                rot: i as f32,
                hue: i as f32 * 0.3,
            })
            .collect();
        Self {
            layers,
            next: 0,
            idle: 0.0,
            rng: Rng::new(0x205E),
        }
    }

    fn retarget(&mut self) {
        let i = self.next % LAYERS;
        self.next += 1;
        self.idle = 0.0;
        let l = &mut self.layers[i];
        let mut k = self.rng.next_u32() as usize % RATIOS.len();
        if k == l.to {
            k = (k + 1) % RATIOS.len();
        }
        // Start from wherever the morph currently is, snapped to the nearer end.
        l.from = if l.morph < 0.5 { l.from } else { l.to };
        l.to = k;
        l.morph = 0.0;
        l.hue += 0.15;
    }
}

/// Additive line with a soft one-pixel halo.
fn glow_line(c: &mut Canvas, (x0, y0): (f32, f32), (x1, y1): (f32, f32), col: Rgb) {
    let halo = scale(col, 0.3);
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).ceil().clamp(1.0, 200.0) as i32;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let x = (x0 + (x1 - x0) * t).round() as i32;
        let y = (y0 + (y1 - y0) * t).round() as i32;
        c.plot_add(x, y, col);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            c.plot_add(x + dx, y + dy, halo);
        }
    }
}

impl Effect for Rose {
    fn name(&self) -> &'static str {
        "Rose"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.idle += dt;
        if f.beat || self.idle > 6.0 {
            self.retarget();
        }
        for (i, l) in self.layers.iter_mut().enumerate() {
            l.morph = (l.morph + dt / MORPH_TIME).min(1.0);
            let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
            l.rot += dt * dir * (0.15 + 0.6 * f.mid) / (1.0 + i as f32 * 0.5);
            l.hue += dt * 0.02;
        }

        c.fade(0.6);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let base = c.w.min(c.h) as f32 * 0.47 * (0.82 + 0.13 * f.bass + 0.08 * f.pulse);
        let wave = &f.waveform;

        for (i, l) in self.layers.iter().enumerate() {
            let (n0, d0) = RATIOS[l.from];
            let (n1, d1) = RATIOS[l.to];
            let (k0, k1) = (n0 as f32 / d0 as f32, n1 as f32 / d1 as f32);
            // Smoothstep morph between the two curves; both close after lcm(d0, d1) turns.
            let m = l.morph * l.morph * (3.0 - 2.0 * l.morph);
            let turns = d0 / gcd(d0, d1).max(1) * d1;
            let samples = (240 * turns).min(2400) as usize;
            let radius = base * [1.0, 0.68, 0.4][i];
            // Dense curves (many petals in a small radius) would saturate, so dim them.
            let density = (radius * 0.15 / (turns as f32 * k0.max(k1))).clamp(0.2, 1.0);
            let b = (0.45 + 0.35 * f.level + 0.3 * f.pulse) * density;
            let mut prev = None;
            for s in 0..=samples {
                let th = s as f32 / samples as f32 * TAU * turns as f32;
                let w = wave.get(s % wave.len().max(1)).copied().unwrap_or(0.0);
                let r = ((1.0 - m) * (k0 * th).cos() + m * (k1 * th).cos()) * radius * (1.0 + 0.05 * w);
                let a = th + l.rot;
                let p = (cx + r * a.cos(), cy + r * a.sin());
                if let Some(q) = prev {
                    let hue = l.hue + (s as f32 / samples as f32) * 0.25;
                    // Every petal passes through the center; dim it there so it doesn't burn out.
                    let core = (r.abs() / (radius * 0.4).max(1.0)).min(1.0);
                    glow_line(c, q, p, hsv(hue, 0.7, b * 0.55 * (0.15 + 0.85 * core)));
                }
                prev = Some(p);
            }
        }
    }
}
