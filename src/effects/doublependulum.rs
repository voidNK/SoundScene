use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, rgb, scale};

const COUNT: usize = 4;
const G: f32 = 9.81;
/// Largest integration step in seconds; the motion is chaotic, so small steps
/// keep it from blowing up.
const STEP: f32 = 0.002;
const MAX_STEPS: usize = 60;

struct Pendulum {
    a1: f32,
    a2: f32,
    w1: f32,
    w2: f32,
    /// Last end bob position in pixels, for continuous trail segments.
    last: Option<(f32, f32)>,
    hue: f32,
}

impl Pendulum {
    /// Angular accelerations for unit masses and unit arm lengths.
    fn accel(&self) -> (f32, f32) {
        let d = self.a1 - self.a2;
        let (sd, cd) = d.sin_cos();
        let den = 3.0 - (2.0 * d).cos();
        let n1 = -3.0 * G * self.a1.sin()
            - G * (self.a1 - 2.0 * self.a2).sin()
            - 2.0 * sd * (self.w2 * self.w2 + self.w1 * self.w1 * cd);
        let n2 = 2.0 * sd * (2.0 * self.w1 * self.w1 + 2.0 * G * self.a1.cos() + self.w2 * self.w2 * cd);
        (n1 / den, n2 / den)
    }

    fn energy(&self) -> f32 {
        let kinetic =
            0.5 * (2.0 * self.w1 * self.w1 + self.w2 * self.w2) + self.w1 * self.w2 * (self.a1 - self.a2).cos();
        kinetic - G * (2.0 * self.a1.cos() + self.a2.cos())
    }
}

/// A bundle of chaotic double pendulums started almost identically, so
/// they drift apart and paint glowing trails with their end bobs. Beats
/// kick them, loudness keeps them energetic, treble brightens the trails.
pub struct DoublePendulum {
    pends: Vec<Pendulum>,
    trail: Canvas,
    rng: Rng,
    hue: f32,
}

impl DoublePendulum {
    pub fn new() -> Self {
        let pends = (0..COUNT)
            .map(|i| Pendulum {
                a1: 2.2 + i as f32 * 0.002,
                a2: 2.6,
                w1: 0.0,
                w2: 0.0,
                last: None,
                hue: i as f32 / COUNT as f32,
            })
            .collect();
        Self {
            pends,
            trail: Canvas::new(),
            rng: Rng::new(0xD0B1),
            hue: 0.0,
        }
    }
}

impl Effect for DoublePendulum {
    fn name(&self) -> &'static str {
        "Double Pendulum"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if self.trail.resize(c.w, c.h) {
            for p in &mut self.pends {
                p.last = None;
            }
        }
        self.hue += dt * 0.03;

        if f.beat {
            let kick = 2.0 + 5.0 * f.bass;
            for p in &mut self.pends {
                p.w1 += self.rng.range(-1.0, 1.0) * kick;
                p.w2 += self.rng.range(-1.0, 1.0) * kick;
            }
            self.hue += 0.05;
        }

        // Pull the energy towards a target: enough to flip over, more when loud.
        let target = -3.0 * G + G * (2.2 + 2.5 * f.level);
        let steps = ((dt / STEP).ceil() as usize).min(MAX_STEPS);
        let h = if steps > 0 { dt / steps as f32 } else { 0.0 };
        for p in &mut self.pends {
            let e = p.energy();
            let gain = ((target - e) * 0.02 * dt).clamp(-0.05, 0.05);
            p.w1 *= 1.0 + gain;
            p.w2 *= 1.0 + gain;
            if e < target - G && p.w1.abs() + p.w2.abs() < 0.05 {
                p.w1 += 0.5;
            }
            for _ in 0..steps {
                let (al1, al2) = p.accel();
                p.w1 = (p.w1 + al1 * h).clamp(-30.0, 30.0);
                p.w2 = (p.w2 + al2 * h).clamp(-30.0, 30.0);
                p.a1 = (p.a1 + p.w1 * h).rem_euclid(std::f32::consts::TAU);
                p.a2 = (p.a2 + p.w2 * h).rem_euclid(std::f32::consts::TAU);
            }
        }

        // Trails live in their own buffer so the arms don't smear.
        self.trail.fade(0.965 - 0.03 * f.pulse);
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let arm = c.w.min(c.h) as f32 * 0.235;
        let bright = 0.55 + 0.3 * f.treble + 0.3 * f.pulse;
        let mut joints = Vec::with_capacity(COUNT);
        for p in &mut self.pends {
            let (x1, y1) = (cx + p.a1.sin() * arm, cy + p.a1.cos() * arm);
            let (x2, y2) = (x1 + p.a2.sin() * arm, y1 + p.a2.cos() * arm);
            let col = hsv(self.hue + p.hue, 0.75, bright);
            if let Some((lx, ly)) = p.last {
                self.trail.line_add(lx, ly, x2, y2, col);
            }
            p.last = Some((x2, y2));
            joints.push((x1, y1, x2, y2, p.hue));
        }

        c.px.copy_from_slice(&self.trail.px);
        let rod = rgb(0.18, 0.2, 0.25);
        let r = (arm * 0.06).clamp(0.6, 3.0);
        for &(x1, y1, x2, y2, hue) in &joints {
            c.line_add(cx, cy, x1, y1, rod);
            c.line_add(x1, y1, x2, y2, rod);
            disc(c, x1, y1, r * 0.8, scale(hsv(self.hue + hue, 0.4, 1.0), 0.6));
            disc(c, x2, y2, r, hsv(self.hue + hue, 0.3, 1.0));
        }
        disc(c, cx, cy, r * 0.7, rgb(0.6, 0.6, 0.65));
    }
}

/// Filled circle, additive.
fn disc(c: &mut Canvas, x: f32, y: f32, r: f32, col: [u8; 3]) {
    let ri = r.ceil() as i32;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            if (dx * dx + dy * dy) as f32 <= r * r + 0.5 {
                c.plot_add(x.round() as i32 + dx, y.round() as i32 + dy, col);
            }
        }
    }
}
