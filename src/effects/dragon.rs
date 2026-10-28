use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv};
use std::f32::consts::FRAC_PI_2;

const MAX_ORDER: u32 = 13;
const WAVE_TIME: f32 = 1.6;

/// Heighway dragon as a strip of paper that keeps folding and unfolding:
/// every crease turns by the same angle, from a loose spiral up to the
/// right-angled dragon. Bass bends the creases, the rainbow runs along the
/// curve and each beat sends a bright wave from one end to the other.
pub struct DragonCurve {
    phase: f32,
    hue: f32,
    rot: f32,
    waves: Vec<f32>,
    pts: Vec<(f32, f32)>,
    bass: f32,
}

impl DragonCurve {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            hue: 0.0,
            rot: 0.0,
            waves: Vec::new(),
            pts: Vec::new(),
            bass: 0.0,
        }
    }
}

impl Effect for DragonCurve {
    fn name(&self) -> &'static str {
        "Dragon Curve"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fade(0.5);
        self.phase += dt * (0.18 + 0.25 * f.mid);
        self.hue += dt * (0.05 + 0.1 * f.treble);
        self.rot += dt * 0.07;
        for w in &mut self.waves {
            *w += dt / WAVE_TIME;
        }
        self.waves.retain(|&w| w < 1.2);
        if f.beat && self.waves.len() < 3 {
            self.waves.push(0.0);
        }

        // Fewer segments on small tiles, where they would just overlap.
        let area = (c.w * c.h).max(1) as f32;
        let order = (area.log2().floor() as u32).clamp(4, MAX_ORDER);
        let n = 1usize << order;

        self.bass += (f.bass - self.bass) * (1.0 - (-6.0 * dt).exp());
        let fold = 0.78 - 0.22 * self.phase.cos();
        let crease = FRAC_PI_2 * fold + 0.2 * (self.bass - 0.3);
        self.pts.clear();
        let (mut x, mut y, mut heading) = (0.0f32, 0.0f32, self.rot);
        self.pts.push((x, y));
        for i in 1..=n {
            let (s, co) = heading.sin_cos();
            x += co;
            y += s;
            self.pts.push((x, y));
            // Paper-folding sequence: the bit above the lowest set bit decides the turn.
            heading += if ((i.isolate_lowest_one() << 1) & i) != 0 {
                -crease
            } else {
                crease
            };
        }

        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &self.pts {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
        let (w, h) = (c.w as f32, c.h as f32);
        let scale = (w * 0.92 / (x1 - x0).max(1e-3)).min(h * 0.92 / (y1 - y0).max(1e-3));
        let (mx, my, sc) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5, scale);
        let screen = |p: (f32, f32)| (w * 0.5 + (p.0 - mx) * sc, h * 0.5 + (p.1 - my) * sc);

        let v = 0.6 + 0.3 * f.level;
        let mut prev = screen(self.pts[0]);
        for (i, &p) in self.pts.iter().enumerate().skip(1) {
            let u = i as f32 / n as f32;
            let flash = self
                .waves
                .iter()
                .fold(0.0f32, |m, &w| m.max(1.0 - (u - w).abs() * 40.0));
            let cur = screen(p);
            let col = hsv(self.hue + u, 0.9 - 0.5 * flash, v + 0.6 * flash);
            line(c, prev, cur, col);
            prev = cur;
        }
    }
}

/// Overwriting line: overlapping parts of the curve keep their color instead
/// of adding up to white.
fn line(c: &mut Canvas, a: (f32, f32), b: (f32, f32), col: Rgb) {
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().clamp(1.0, 2000.0) as i32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        c.plot((a.0 + (b.0 - a.0) * t) as i32, (a.1 + (b.1 - a.1) * t) as i32, col);
    }
}
