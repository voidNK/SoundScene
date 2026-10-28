use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

/// Gear radius ratios r/R that close into pleasing figures.
const RATIOS: [f32; 12] = [
    3.0 / 5.0,
    2.0 / 7.0,
    3.0 / 7.0,
    5.0 / 8.0,
    4.0 / 9.0,
    2.0 / 5.0,
    5.0 / 12.0,
    7.0 / 11.0,
    3.0 / 8.0,
    1.0 / 3.0,
    5.0 / 7.0,
    4.0 / 11.0,
];

/// A spirograph pen tracing hypotrochoids and epitrochoids with a rainbow
/// ink and a slowly fading trail. A beat (at most every few seconds) picks a
/// new gear ratio and pen offset that the curve morphs into; bass drives the
/// pen faster and beats brighten the ink.
pub struct Spirograph {
    theta: f32,
    r: f32,
    d: f32,
    target_r: f32,
    target_d: f32,
    epi: bool,
    changes: u32,
    since: f32,
    size: (usize, usize),
    last: Option<(f32, f32)>,
    spin: f32,
    hue: f32,
    rng: Rng,
}

impl Spirograph {
    pub fn new() -> Self {
        Self {
            theta: 0.0,
            r: RATIOS[1],
            d: 0.85,
            target_r: RATIOS[1],
            target_d: 0.85,
            epi: false,
            changes: 0,
            since: 0.0,
            size: (0, 0),
            last: None,
            spin: 0.0,
            hue: 0.0,
            rng: Rng::new(0x5B1F),
        }
    }

    /// Pen position for the fixed ring radius R = 1, normalized to fit in a unit circle.
    fn point(&self, theta: f32) -> (f32, f32) {
        let r = self.r.max(0.05);
        let (x, y, extent) = if self.epi {
            let k = (1.0 + r) / r;
            (
                (1.0 + r) * theta.cos() - self.d * (k * theta).cos(),
                (1.0 + r) * theta.sin() - self.d * (k * theta).sin(),
                1.0 + r + self.d,
            )
        } else {
            let k = (1.0 - r) / r;
            (
                (1.0 - r) * theta.cos() + self.d * (k * theta).cos(),
                (1.0 - r) * theta.sin() - self.d * (k * theta).sin(),
                (1.0 - r).abs() + self.d,
            )
        };
        let e = extent.max(0.1);
        let (s, c) = self.spin.sin_cos();
        ((x * c - y * s) / e, (x * s + y * c) / e)
    }
}

impl Effect for Spirograph {
    fn name(&self) -> &'static str {
        "Spirograph"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            self.last = None;
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.last = None;
        }
        c.fade((-dt * 0.5).exp());

        // Changing gears on every beat would never let a figure close.
        self.since += dt;
        if f.beat && self.since > 5.0 {
            self.since = 0.0;
            self.changes += 1;
            self.target_r = RATIOS[self.rng.next_u32() as usize % RATIOS.len()];
            // A pen outside the gear (d > r) draws loops, inside it draws waves.
            self.target_d = (self.target_r * self.rng.range(0.6, 1.8)).min(1.0);
            // Every fourth change switches between inner and outer gears.
            if self.changes.is_multiple_of(4) {
                self.epi = !self.epi;
                self.last = None;
            }
            if self.epi {
                self.target_r *= 0.5;
            }
        }
        let k = 1.0 - (-dt * 1.5).exp();
        self.r += (self.target_r - self.r) * k;
        self.d += (self.target_d - self.d) * k;
        self.spin += dt * 0.08;
        self.hue += dt * 0.05;

        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let radius = c.w.min(c.h) as f32 * 0.47;
        let advance = dt * (4.0 + 9.0 * f.bass + 3.0 * f.pulse);
        let steps = (advance / 0.03).ceil().clamp(1.0, 400.0) as usize;
        let value = 0.5 + 0.3 * f.level + 0.2 * f.pulse;
        for i in 1..=steps {
            let th = self.theta + advance * i as f32 / steps as f32;
            let (x, y) = self.point(th);
            let p = (cx + x * radius, cy + y * radius);
            if let Some((lx, ly)) = self.last {
                c.line_add(lx, ly, p.0, p.1, hsv(self.hue + th * 0.015, 0.8, value));
            }
            self.last = Some(p);
        }
        self.theta += advance;
        // Keep f32 precision; the jump would draw a stray line, so lift the pen.
        if self.theta > 1.0e4 {
            self.theta -= 1.0e4;
            self.last = None;
        }
        if let Some((x, y)) = self.last {
            c.plot_add(x as i32, y as i32, [255, 255, 255]);
        }
    }
}
