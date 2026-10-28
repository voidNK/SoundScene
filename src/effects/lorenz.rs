use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, scale};
use std::collections::VecDeque;

const TRAIL: usize = 3000;
const STEP: f32 = 0.004;
const MAX_STEPS: usize = 400;

/// Lorenz attractor traced in 3D by a glowing head with a long fading trail,
/// seen from a camera that circles it. Loudness speeds up the flow, bass and
/// mids nudge rho and sigma, the hue shifts along the trail over time and
/// beats flare the head.
pub struct Lorenz {
    p: (f32, f32, f32),
    trail: VecDeque<(f32, f32, f32, f32)>,
    clock: f32,
    acc: f32,
    yaw: f32,
}

impl Lorenz {
    pub fn new() -> Self {
        Self {
            p: (0.1, 0.0, 20.0),
            trail: VecDeque::with_capacity(TRAIL + MAX_STEPS),
            clock: 0.0,
            acc: 0.0,
            yaw: 0.0,
        }
    }
}

fn deriv(p: (f32, f32, f32), sigma: f32, rho: f32, beta: f32) -> (f32, f32, f32) {
    (sigma * (p.1 - p.0), p.0 * (rho - p.2) - p.1, p.0 * p.1 - beta * p.2)
}

impl Effect for Lorenz {
    fn name(&self) -> &'static str {
        "Lorenz"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.35);
        let sigma = 10.0 + 3.0 * f.mid;
        let rho = 28.0 + 6.0 * f.bass;
        let beta = 8.0 / 3.0;
        self.yaw += dt * (0.12 + 0.2 * f.treble);

        // Integrate with RK4 in fixed steps, carrying the remainder over.
        self.acc += dt * (0.45 + 0.9 * f.level);
        let mut steps = 0;
        while self.acc >= STEP && steps < MAX_STEPS {
            self.acc -= STEP;
            steps += 1;
            let h = STEP;
            let p = self.p;
            let k1 = deriv(p, sigma, rho, beta);
            let at = |k: (f32, f32, f32), s: f32| (p.0 + k.0 * s, p.1 + k.1 * s, p.2 + k.2 * s);
            let k2 = deriv(at(k1, h * 0.5), sigma, rho, beta);
            let k3 = deriv(at(k2, h * 0.5), sigma, rho, beta);
            let k4 = deriv(at(k3, h), sigma, rho, beta);
            self.p = (
                p.0 + h / 6.0 * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0),
                p.1 + h / 6.0 * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1),
                p.2 + h / 6.0 * (k1.2 + 2.0 * k2.2 + 2.0 * k3.2 + k4.2),
            );
            if !(self.p.0.is_finite() && self.p.1.is_finite() && self.p.2.is_finite()) || self.p.2.abs() > 500.0 {
                self.p = (0.1, 0.0, 20.0);
            }
            self.clock += STEP;
            self.trail.push_back((self.p.0, self.p.1, self.p.2, self.clock));
        }
        if steps == MAX_STEPS {
            self.acc = 0.0;
        }
        while self.trail.len() > TRAIL {
            self.trail.pop_front();
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let sc = (h / 52.0).min(w / 58.0);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let (ys, yc) = self.yaw.sin_cos();
        let (ps, pc) = (0.35 + 0.15 * (t * 0.13).sin()).sin_cos();
        // Rotate around the attractor's vertical z axis, then tilt, then a mild perspective.
        let project = |p: &(f32, f32, f32, f32)| {
            let (x, y, z) = (p.0, p.1, p.2 - 25.0);
            let (rx, ry) = (x * yc - y * ys, x * ys + y * yc);
            let (vy, depth) = (z * pc - ry * ps, z * ps + ry * pc);
            let k = 60.0 / (60.0 + depth).max(10.0);
            (cx + rx * sc * k, cy - vy * sc * k, depth)
        };

        let n = self.trail.len();
        let mut prev: Option<(f32, f32, f32)> = None;
        for (i, p) in self.trail.iter().enumerate() {
            let q = project(p);
            if let Some(a) = prev {
                let age = i as f32 / n as f32;
                let near = (1.2 - (a.2 + 25.0) / 50.0).clamp(0.4, 1.0);
                let v = age * age * near * 0.8;
                c.line_add(a.0, a.1, q.0, q.1, hsv(p.3 * 0.03, 0.8 - 0.3 * age, v));
            }
            prev = Some(q);
        }
        if let Some(head) = self.trail.back() {
            let (x, y, _) = project(head);
            let r = 1 + (f.pulse * 2.5) as i32;
            let col = scale(hsv(head.3 * 0.03, 0.3, 1.0), 0.6 + 0.4 * f.pulse);
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r {
                        c.plot_add(x as i32 + dx, y as i32 + dy, col);
                    }
                }
            }
        }
    }
}
