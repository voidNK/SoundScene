use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};
use std::collections::VecDeque;

type V3 = [f32; 3];

const CORNERS: [V3; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];
const FACES: [([usize; 4], V3); 6] = [
    ([0, 1, 2, 3], [0.0, 0.0, -1.0]),
    ([5, 4, 7, 6], [0.0, 0.0, 1.0]),
    ([4, 0, 3, 7], [-1.0, 0.0, 0.0]),
    ([1, 5, 6, 2], [1.0, 0.0, 0.0]),
    ([4, 5, 1, 0], [0.0, -1.0, 0.0]),
    ([3, 2, 6, 7], [0.0, 1.0, 0.0]),
];
const CAM: f32 = 5.0;
/// Seconds of rotation history kept for the delayed scanlines.
const HISTORY: f32 = 1.5;

fn rotate(v: V3, s: &[(f32, f32); 3]) -> V3 {
    let (y, z) = (v[1] * s[0].1 - v[2] * s[0].0, v[1] * s[0].0 + v[2] * s[0].1);
    let (x, z) = (v[0] * s[1].1 + z * s[1].0, -v[0] * s[1].0 + z * s[1].1);
    let (x, y) = (x * s[2].1 - y * s[2].0, x * s[2].0 + y * s[2].1);
    [x, y, z]
}

/// Rubber vector cube: a filled, checker-textured cube where every scanline
/// is drawn with the rotation from a moment ago, the further from the middle
/// the older, so it wobbles like jelly. Bass spins it harder and lengthens
/// the delay, beats kick it.
pub struct RubberCube {
    clock: f32,
    angles: V3,
    kick: f32,
    hue: f32,
    history: VecDeque<(f32, V3)>,
}

impl RubberCube {
    pub fn new() -> Self {
        Self {
            clock: 0.0,
            angles: [0.4, 0.0, 0.2],
            kick: 0.0,
            hue: 0.0,
            history: VecDeque::new(),
        }
    }

    /// Rotation at `time`, interpolated from the history.
    fn angles_at(&self, time: f32) -> V3 {
        let mut prev = match self.history.back() {
            Some(&(_, a)) if self.history.len() == 1 => return a,
            Some(&e) => e,
            None => return self.angles,
        };
        for &(t, a) in self.history.iter().rev().skip(1) {
            if t <= time {
                let span = prev.0 - t;
                let k = if span > 1e-6 {
                    ((time - t) / span).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                return [0, 1, 2].map(|j| a[j] + (prev.1[j] - a[j]) * k);
            }
            prev = (t, a);
        }
        prev.1
    }
}

impl Effect for RubberCube {
    fn name(&self) -> &'static str {
        "Rubber Cube"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.kick = 1.0;
        }
        self.kick *= (-4.0 * dt).exp();
        self.clock += dt;
        let boost = 1.0 + 1.5 * self.kick;
        self.angles[0] += dt * (0.5 + 0.6 * f.mid) * boost;
        self.angles[1] += dt * (0.7 + 1.6 * f.bass) * boost;
        self.angles[2] += dt * 0.3;
        self.hue += dt * 0.025;
        self.history.push_back((self.clock, self.angles));
        while self.history.len() > 2 && self.history.front().is_some_and(|e| e.0 < self.clock - HISTORY) {
            self.history.pop_front();
        }
        while self.history.len() > 600 {
            self.history.pop_front();
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let size = w.min(h) * 1.15 * (1.0 + 0.08 * f.pulse);
        let max_delay = (0.05 + 0.25 * f.bass + 0.1 * self.kick).min(HISTORY);
        let light = {
            let l = [-0.4f32, -0.6, -1.0];
            let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
            l.map(|v| v / n)
        };
        let faces: Vec<_> = FACES
            .iter()
            .enumerate()
            .map(|(i, _)| hsv(self.hue + i as f32 / 6.0, 0.7, 1.0).map(|v| v as f32 / 255.0))
            .collect();
        let glow = 0.05 * f.pulse;

        for y in 0..c.h {
            let yc = y as f32 + 0.5;
            let stripe = ((yc / h.max(1.0) * 12.0 - self.clock * 0.5).fract() < 0.5) as u8 as f32;
            let bg = rgb(0.03 + glow, 0.02 + 0.015 * stripe, 0.07 + 0.03 * stripe + glow);
            for x in 0..c.w {
                c.set(x, y, bg);
            }

            let delay = ((yc - cy).abs() / cy.max(1.0)) * max_delay;
            let sc = self.angles_at(self.clock - delay).map(|a| a.sin_cos());
            let pts = CORNERS.map(|v| {
                let r = rotate(v, &sc);
                let d = size / (r[2] + CAM).max(0.1);
                (cx + r[0] * d, cy + r[1] * d)
            });

            for (fi, (idx, axis)) in FACES.iter().enumerate() {
                // The face center equals its normal, so the view test is n·(n + cam) < 0.
                let n = rotate(*axis, &sc);
                if 1.0 + n[2] * CAM >= 0.0 {
                    continue;
                }
                let (mut x0, mut x1) = (f32::MAX, f32::MIN);
                for k in 0..4 {
                    let (p, q) = (pts[idx[k]], pts[idx[(k + 1) % 4]]);
                    if (p.1 <= yc && yc < q.1) || (q.1 <= yc && yc < p.1) {
                        let x = p.0 + (yc - p.1) * (q.0 - p.0) / (q.1 - p.1);
                        x0 = x0.min(x);
                        x1 = x1.max(x);
                    }
                }
                if x0 > x1 || x1 < 0.0 || x0 >= w {
                    continue;
                }
                // Affine face coordinates from the projected corners 0, 1 and 3.
                let o = pts[idx[0]];
                let e1 = (pts[idx[1]].0 - o.0, pts[idx[1]].1 - o.1);
                let e2 = (pts[idx[3]].0 - o.0, pts[idx[3]].1 - o.1);
                let det = e1.0 * e2.1 - e1.1 * e2.0;
                let inv = if det.abs() > 1e-6 { 1.0 / det } else { 0.0 };
                let lam = (n[0] * light[0] + n[1] * light[1] + n[2] * light[2]).max(0.0);
                let base = faces[fi];
                let py = yc - o.1;
                let xs = (x0 - 0.5).ceil().max(0.0) as usize;
                let xe = ((x1 - 0.5).floor().min(w - 1.0)).max(-1.0);
                if xe < 0.0 {
                    continue;
                }
                for x in xs..=xe as usize {
                    let px = x as f32 + 0.5 - o.0;
                    let u = ((e2.1 * px - e2.0 * py) * inv).clamp(0.0, 1.0);
                    let v = ((-e1.1 * px + e1.0 * py) * inv).clamp(0.0, 1.0);
                    let border = u.min(1.0 - u).min(v).min(1.0 - v) < 0.05;
                    let check = ((u * 4.0) as i32 + (v * 4.0) as i32) & 1 == 0;
                    let k = (0.2 + 0.8 * lam) * if check { 1.0 } else { 0.55 } + glow * 4.0;
                    let col = if border {
                        rgb(0.9 * k + 0.1, 0.9 * k + 0.1, 0.9 * k + 0.1)
                    } else {
                        rgb(base[0] * k, base[1] * k, base[2] * k)
                    };
                    c.set(x, y, col);
                }
            }
        }
    }
}
