use super::Effect;
use super::koch::glow_line;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, scale};
use std::f32::consts::PI;

const MAX_ORDER: u32 = 14;

/// Lévy C curve: every segment is replaced by two that bend the same way,
/// with a fold angle that morphs from a gentle wave through the classic 45°
/// into dense overlapping knots. Drawn additively so overlaps glow. Bass
/// bends the folds, the spectrum runs along the curve, beats flash it.
pub struct LevyCurve {
    phase: f32,
    rot: f32,
    hue: f32,
    bass: f32,
    pts: Vec<(f32, f32)>,
}

impl LevyCurve {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            rot: 0.0,
            hue: 0.0,
            bass: 0.0,
            pts: Vec::new(),
        }
    }
}

impl Effect for LevyCurve {
    fn name(&self) -> &'static str {
        "Lévy Curve"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fade(0.4);
        self.phase += dt * (0.12 + 0.2 * f.mid);
        self.rot += dt * 0.05;
        self.hue += dt * (0.04 + 0.08 * f.treble);
        self.bass += (f.bass - self.bass) * (1.0 - (-6.0 * dt).exp());
        if c.w == 0 || c.h == 0 {
            return;
        }

        let area = (c.w * c.h).max(1) as f32;
        let order = ((area.log2() - 1.0).floor().max(4.0) as u32).min(MAX_ORDER);
        let n = 1usize << order;
        let deg = 40.0 - 13.0 * self.phase.cos() + 8.0 * (self.bass - 0.3);
        let fold = deg.clamp(20.0, 56.0) * PI / 180.0;

        // Segment i turns by fold·(order − 2·popcount(i)): each level sends
        // the first half left and the second half right.
        self.pts.clear();
        let (mut x, mut y) = (0.0f32, 0.0f32);
        self.pts.push((x, y));
        for i in 0..n {
            let a = self.rot + fold * (order as f32 - 2.0 * i.count_ones() as f32);
            let (s, co) = a.sin_cos();
            x += co;
            y -= s;
            self.pts.push((x, y));
        }

        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &self.pts {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
        let (w, h) = (c.w as f32, c.h as f32);
        let sc = (w * 0.9 / (x1 - x0).max(1e-3)).min(h * 0.9 / (y1 - y0).max(1e-3)) * (1.0 + 0.05 * f.pulse);
        let (mx, my) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
        let screen = |p: (f32, f32)| (w * 0.5 + (p.0 - mx) * sc, h * 0.5 + (p.1 - my) * sc);

        // Vertices of a coarser level are every 2^k-th vertex of this one;
        // draw that level where segments would otherwise be sub-pixel.
        let grow = (2.0 * fold.cos()).max(1.0001);
        let k = ((1.5 / sc.max(1e-6)).ln() / grow.ln()).ceil().clamp(0.0, order as f32) as u32;
        let stride = 1usize << k;
        let gain = 0.45 + 0.2 * f.level + 0.5 * f.pulse;
        let bins = f.spectrum.len();
        let mut prev = screen(self.pts[0]);
        for i in (stride..=n).step_by(stride) {
            let u = i as f32 / n as f32;
            let e = if bins > 0 {
                f.spectrum[((u * bins as f32) as usize).min(bins - 1)]
            } else {
                0.0
            };
            let cur = screen(self.pts[i]);
            let col = scale(hsv(self.hue + u * 0.8, 0.85, 0.45 + 0.55 * e), gain);
            glow_line(c, prev.0, prev.1, cur.0, cur.1, col);
            prev = cur;
        }
    }
}
