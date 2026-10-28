use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, rgb};

/// A glass lens drifts over a scrolling, rotating checkerboard whose tiles
/// glow with the spectrum. Inside, the image is magnified with a little
/// chromatic fringe, a highlight and a drop shadow. Bass pumps the lens size,
/// beats punch the magnification.
pub struct Lens {
    scroll: f32,
    angle: f32,
    phase: f32,
    hue: f32,
}

impl Lens {
    pub fn new() -> Self {
        Self {
            scroll: 0.0,
            angle: 0.0,
            phase: 0.0,
            hue: 0.0,
        }
    }
}

/// Background in pixel space, returns linear RGB 0..1.
struct Background<'a> {
    sin: f32,
    cos: f32,
    cell: f32,
    scroll: f32,
    hue: f32,
    spectrum: &'a [f32],
}

impl Background<'_> {
    fn sample(&self, x: f32, y: f32) -> [f32; 3] {
        let u = (x * self.cos - y * self.sin) / self.cell + self.scroll;
        let v = (x * self.sin + y * self.cos) / self.cell + self.scroll * 0.6;
        let (iu, iv) = (u.floor(), v.floor());
        let odd = (iu as i64 + iv as i64) & 1 == 1;
        let bins = self.spectrum.len();
        let e = if bins > 0 {
            self.spectrum[((iu as i64 * 7 + iv as i64 * 13).rem_euclid(bins as i64)) as usize]
        } else {
            0.0
        };
        let (fu, fv) = (u - iu - 0.5, v - iv - 0.5);
        let dot = 1.0 - (fu * fu + fv * fv) * 8.0;
        let hue = self.hue + (iu + iv) * 0.03;
        let tau = std::f32::consts::TAU;
        let base = [0.0, 1.0 / 3.0, 2.0 / 3.0].map(|o| 0.5 + 0.5 * ((hue + o) * tau).cos());
        if odd {
            let k = 0.35 + 0.55 * e;
            base.map(|b| b * k + dot.max(0.0) * 0.25 * e)
        } else {
            let k = 0.06 + 0.1 * e;
            base.map(|b| (1.0 - b) * k + 0.03)
        }
    }
}

impl Effect for Lens {
    fn name(&self) -> &'static str {
        "Lens"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.scroll += dt * (0.3 + 0.8 * f.level);
        self.angle += dt * 0.07;
        self.phase += dt * (0.35 + 0.5 * f.mid);
        self.hue += dt * 0.03;

        let (w, h) = (c.w as f32, c.h as f32);
        let m = w.min(h);
        let bg = Background {
            sin: self.angle.sin(),
            cos: self.angle.cos(),
            cell: (m / 7.0).max(2.0),
            scroll: self.scroll,
            hue: self.hue,
            spectrum: &f.spectrum,
        };
        let r = (m * (0.24 + 0.14 * f.bass + 0.05 * f.pulse)).max(1.0);
        let lx = w * 0.5 + (w * 0.5 - r * 0.6).max(0.0) * (self.phase * 0.9).sin();
        let ly = h * 0.5 + (h * 0.5 - r * 0.6).max(0.0) * (self.phase * 1.3 + 0.5).cos();
        let mag = 0.55 + 0.25 * f.pulse;
        let shadow = (r * 0.18, r * 0.22);

        for y in 0..c.h {
            let fy = y as f32 + 0.5;
            for x in 0..c.w {
                let fx = x as f32 + 0.5;
                let (dx, dy) = (fx - lx, fy - ly);
                let d2 = (dx * dx + dy * dy) / (r * r);
                let col = if d2 < 1.0 {
                    // Spherical cap: magnification is strongest at the center.
                    let z = (1.0 - d2).sqrt();
                    let ch = |j: usize, k: f32| {
                        let s = 1.0 - (mag + k) * z;
                        bg.sample(lx + dx * s, ly + dy * s)[j]
                    };
                    let (cr, cg, cb) = (ch(0, 0.04), ch(1, 0.0), ch(2, -0.04));
                    let (hx, hy) = (dx / r + 0.38, dy / r + 0.38);
                    let spec = (-(hx * hx + hy * hy) * 18.0).exp() * 0.7;
                    let rim = ((d2 - 0.85) / 0.15).clamp(0.0, 1.0) * 0.35;
                    let k = 1.05 + 0.2 * z;
                    [
                        cr * k + spec + rim * 0.6,
                        cg * k + spec + rim * 0.8,
                        cb * k + spec + rim,
                    ]
                } else {
                    let mut p = bg.sample(fx, fy);
                    let (sx, sy) = (dx - shadow.0, dy - shadow.1);
                    let s2 = (sx * sx + sy * sy) / (r * r);
                    if s2 < 1.15 {
                        let k = 0.45 + 0.55 * ((s2 - 0.8) / 0.35).clamp(0.0, 1.0);
                        p = p.map(|v| v * k);
                    }
                    p
                };
                c.set(x, y, rgb(col[0], col[1], col[2]));
            }
        }
    }
}
