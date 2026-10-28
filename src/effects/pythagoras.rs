use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix, rgb};

const DEPTH: usize = 11;

type Pt = (f32, f32);

/// Pythagoras tree: every square carries a right triangle whose legs are the
/// bases of two smaller squares. Bass opens and closes the triangle angle,
/// which bends the whole tree, the branches sway with time, each depth glows
/// with its own spectrum band and the crown flashes on beats.
pub struct PythagorasTree {
    quads: Vec<([Pt; 4], usize)>,
    fit: Option<(f32, f32, f32, f32)>,
    bass: f32,
    hue: f32,
}

impl PythagorasTree {
    pub fn new() -> Self {
        Self {
            quads: Vec::new(),
            fit: None,
            bass: 0.0,
            hue: 0.0,
        }
    }

    fn grow(&mut self, p0: Pt, p1: Pt, depth: usize, angle: &dyn Fn(usize) -> f32) {
        let v = (p1.0 - p0.0, p1.1 - p0.1);
        // Rotating the base by -90° points away from the parent (y grows down).
        let up = (v.1, -v.0);
        let p2 = (p1.0 + up.0, p1.1 + up.1);
        let p3 = (p0.0 + up.0, p0.1 + up.1);
        self.quads.push(([p0, p1, p2, p3], depth));
        if depth + 1 >= DEPTH {
            return;
        }
        let a = angle(depth);
        let (s, c) = (-a).sin_cos();
        let apex = (
            p3.0 + (v.0 * c - v.1 * s) * a.cos(),
            p3.1 + (v.0 * s + v.1 * c) * a.cos(),
        );
        self.grow(p3, apex, depth + 1, angle);
        self.grow(apex, p2, depth + 1, angle);
    }
}

impl Effect for PythagorasTree {
    fn name(&self) -> &'static str {
        "Pythagoras Tree"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.bass += (f.bass - self.bass) * (1.0 - (-4.0 * dt).exp());
        self.hue += dt * 0.01 + if f.beat { 0.04 } else { 0.0 };

        let base = 0.785 + 0.32 * (self.bass - 0.4) + 0.12 * (t * 0.37).sin();
        let angle = move |d: usize| (base + 0.05 * (t * 1.3 + d as f32 * 0.8).sin()).clamp(0.3, 1.27);
        self.quads.clear();
        self.grow((-0.5, 0.0), (0.5, 0.0), 0, &angle);

        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (q, _) in &self.quads {
            for p in q {
                x0 = x0.min(p.0);
                y0 = y0.min(p.1);
                x1 = x1.max(p.0);
                y1 = y1.max(p.1);
            }
        }
        // Smooth the bounding box so the camera glides instead of jittering.
        let target = (x0, y0, x1, y1);
        let k = 1.0 - (-3.0 * dt).exp();
        let fit = match self.fit {
            Some(b) => (
                b.0 + (target.0 - b.0) * k,
                b.1 + (target.1 - b.1) * k,
                b.2 + (target.2 - b.2) * k,
                b.3 + (target.3 - b.3) * k,
            ),
            None => target,
        };
        self.fit = Some(fit);

        let (w, h) = (c.w as f32, c.h as f32);
        for y in 0..c.h {
            let g = y as f32 / h.max(1.0);
            let sky = rgb(0.02 + 0.05 * g, 0.01 + 0.03 * g, 0.08 + 0.06 * g + 0.05 * f.pulse);
            for x in 0..c.w {
                c.set(x, y, sky);
            }
        }
        let scale = ((w * 0.94) / (fit.2 - fit.0).max(1e-3)).min((h * 0.94) / (fit.3 - fit.1).max(1e-3));
        let ox = w * 0.5 - (fit.0 + fit.2) * 0.5 * scale;
        let oy = h * 0.97 - fit.3 * scale;

        let bins = f.spectrum.len();
        let trunk = rgb(0.35, 0.2, 0.1);
        for (q, d) in &self.quads {
            let band = if bins > 0 {
                f.spectrum[(d * bins / DEPTH).min(bins - 1)]
            } else {
                0.0
            };
            let u = *d as f32 / (DEPTH - 1) as f32;
            let leaf = hsv(self.hue + 0.28 + u * 0.25, 0.75, 0.45 + 0.55 * band);
            let mut col = mix(trunk, leaf, (u * 1.6 - 0.2).clamp(0.0, 1.0));
            if *d + 2 >= DEPTH {
                col = mix(col, hsv(self.hue + 0.95, 0.4, 1.0), f.pulse * 0.8);
            }
            let pts = q.map(|p| (ox + p.0 * scale, oy + p.1 * scale));
            fill_quad(c, &pts, col);
        }
    }
}

/// Fills a convex quad by testing pixel centers against its four edges.
fn fill_quad(c: &mut Canvas, q: &[Pt; 4], col: Rgb) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in q {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return;
    }
    let (xa, xb) = (x0.floor().max(0.0) as i32, x1.ceil().min(c.w as f32) as i32);
    let (ya, yb) = (y0.floor().max(0.0) as i32, y1.ceil().min(c.h as f32) as i32);
    if xb - xa <= 1 && yb - ya <= 1 {
        // Sub-pixel square: plot its center so the crown does not vanish.
        c.plot(((x0 + x1) * 0.5) as i32, ((y0 + y1) * 0.5) as i32, col);
        return;
    }
    // Winding of the quad decides which side of each edge is inside.
    let area = (q[1].0 - q[0].0) * (q[2].1 - q[0].1) - (q[1].1 - q[0].1) * (q[2].0 - q[0].0);
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    for y in ya..yb {
        let py = y as f32 + 0.5;
        for x in xa..xb {
            let px = x as f32 + 0.5;
            let inside = (0..4).all(|i| {
                let (a, b) = (q[i], q[(i + 1) % 4]);
                ((b.0 - a.0) * (py - a.1) - (b.1 - a.1) * (px - a.0)) * sign >= 0.0
            });
            if inside {
                c.set(x as usize, y as usize, col);
            }
        }
    }
}
