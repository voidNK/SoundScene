use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, scale};
use std::f32::consts::TAU;

const MAX_LEVEL: usize = 5;

/// Rotating Koch snowflake with a counter-rotating anti-snowflake inside.
/// Bass raises the iteration level (new spikes grow in smoothly) and the
/// spike height, beats flash the glowing outline.
pub struct Koch {
    rot: f32,
    hue: f32,
    bass: f32,
    pts: Vec<(f32, f32)>,
    next: Vec<(f32, f32)>,
}

impl Koch {
    pub fn new() -> Self {
        Self {
            rot: 0.0,
            hue: 0.0,
            bass: 0.0,
            pts: Vec::new(),
            next: Vec::new(),
        }
    }

    /// Builds the closed outline: `levels` full subdivisions, the last one
    /// with its spikes scaled by `frac`. `spike` < 0 points them inwards.
    fn build(&mut self, levels: usize, frac: f32, spike: f32, rot: f32) {
        self.pts.clear();
        for i in 0..3 {
            let a = rot + i as f32 * TAU / 3.0;
            self.pts.push((a.sin(), -a.cos()));
        }
        for l in 0..levels {
            let h = spike * if l + 1 == levels { frac } else { 1.0 } * 0.288_675;
            self.next.clear();
            let n = self.pts.len();
            for i in 0..n {
                let (p, q) = (self.pts[i], self.pts[(i + 1) % n]);
                let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                let (mx, my) = (p.0 + dx * 0.5, p.1 + dy * 0.5);
                self.next.push(p);
                self.next.push((p.0 + dx / 3.0, p.1 + dy / 3.0));
                // Outward normal of a clockwise outline (screen y points down).
                self.next.push((mx + dy * h, my - dx * h));
                self.next.push((p.0 + dx * 2.0 / 3.0, p.1 + dy * 2.0 / 3.0));
            }
            std::mem::swap(&mut self.pts, &mut self.next);
        }
    }

    fn draw(&self, c: &mut Canvas, cx: f32, cy: f32, r: f32, hue: f32, v: f32) {
        let n = self.pts.len();
        for i in 0..n {
            let (p, q) = (self.pts[i], self.pts[(i + 1) % n]);
            let col = hsv(hue + i as f32 / n as f32, 0.75, v);
            glow_line(c, cx + p.0 * r, cy + p.1 * r, cx + q.0 * r, cy + q.1 * r, col);
        }
    }
}

/// Additive line with a dim one-pixel halo.
pub(super) fn glow_line(c: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, col: Rgb) {
    let halo = scale(col, 0.22);
    for (ox, oy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
        c.line_add(x0 + ox, y0 + oy, x1 + ox, y1 + oy, halo);
    }
    c.line_add(x0, y0, x1, y1, col);
}

impl Effect for Koch {
    fn name(&self) -> &'static str {
        "Koch Snowflake"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.45);
        self.rot += dt * (0.15 + 0.3 * f.mid);
        self.hue += dt * (0.04 + 0.08 * f.treble);
        self.bass += (f.bass - self.bass) * (1.0 - (-5.0 * dt).exp());

        let size = c.w.min(c.h) as f32;
        let r = size * 0.48 * (0.9 + 0.1 * f.pulse);
        if r < 1.0 {
            return;
        }
        // Small tiles get fewer levels, the extra spikes would be sub-pixel.
        let cap = ((size / 6.0).log(3.0).floor().max(1.0) as usize).min(MAX_LEVEL) as f32;
        let level = (1.3 + 3.2 * self.bass + 0.5 * (t * 0.4).sin()).clamp(1.0, cap);
        let (full, frac) = (level.ceil() as usize, level - level.ceil() + 1.0);
        let spike = 0.75 + 0.45 * self.bass;
        let v = 0.55 + 0.35 * f.level + 0.4 * f.pulse;
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.52);

        self.build(full, frac, spike, self.rot);
        self.draw(c, cx, cy, r, self.hue, v);
        let inner = (level - 1.0).max(1.0);
        let (full, frac) = (inner.ceil() as usize, inner - inner.ceil() + 1.0);
        self.build(full, frac, -spike, -self.rot * 1.6);
        self.draw(c, cx, cy, r * 0.42, self.hue + 0.5, v * 0.8);
    }
}
