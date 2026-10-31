use super::Effect;
use super::mirrorbars::sample;
use crate::analysis::Features;
use crate::gfx::{Canvas, add, hsv, rgb, scale};

/// Light direction (normalized), from the upper left towards the viewer.
const LIGHT: [f32; 3] = [-0.48, -0.56, 0.67];

/// A loudspeaker seen from the front: frame, rubber surround, ribbed cone
/// and domed dust cap, lit from the upper left. Bass drives the cone on a
/// springy suspension, the waveform ripples over its surface and beats send
/// shockwave rings across the baffle.
pub struct Speaker {
    exc: f32,
    vel: f32,
    waves: Vec<f32>,
    hue: f32,
}

impl Speaker {
    pub fn new() -> Self {
        Self {
            exc: 0.0,
            vel: 0.0,
            waves: Vec::new(),
            hue: 0.58,
        }
    }
}

fn shade(n: [f32; 3]) -> f32 {
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
    ((n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]) / len).max(0.0)
}

impl Effect for Speaker {
    fn name(&self) -> &'static str {
        "Speaker"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        // Damped spring towards the bass level, in small substeps.
        let target = f.bass * 0.8 + f.pulse * 0.5 + 0.05 * (t * 2.0).sin();
        let steps = (dt / 0.005).ceil().clamp(0.0, 40.0) as usize;
        let h_dt = if steps > 0 { dt / steps as f32 } else { 0.0 };
        for _ in 0..steps {
            let acc = (target - self.exc) * 900.0 - self.vel * 30.0;
            self.vel += acc * h_dt;
            self.exc += self.vel * h_dt;
        }
        self.exc = self.exc.clamp(-0.5, 1.5);
        if f.beat {
            self.waves.push(0.95);
        }
        for r in &mut self.waves {
            *r += dt * (1.2 + f.bass);
        }
        self.waves.retain(|&r| r < 3.0);
        self.hue += dt * 0.02;

        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let big_r = w.min(h) as f32 * 0.46;
        let baffle = hsv(self.hue + 0.5, 0.25, 0.07);
        if big_r < 1.0 {
            c.fill(baffle);
            return;
        }
        let e = self.exc;
        let cap_r = 0.3 * (1.0 + e * 0.12);
        let slope = 0.5 + e * 0.6;
        let cone_col = hsv(self.hue, 0.35, 0.75);
        let cap_col = hsv(self.hue + 0.06, 0.45, 0.95);
        let wave_col = hsv(self.hue + 0.45, 0.7, 1.0);
        let glow = 0.15 * f.pulse;

        for y in 0..h {
            let dy = (y as f32 + 0.5 - cy) / big_r;
            for x in 0..w {
                let dx = (x as f32 + 0.5 - cx) / big_r;
                let r = (dx * dx + dy * dy).sqrt();
                let (ux, uy) = if r > 1e-4 { (dx / r, dy / r) } else { (0.0, 0.0) };
                let col = if r > 1.0 {
                    let mut col = scale(baffle, 1.0 - (r - 1.0).min(1.0) * 0.4);
                    for &rw in &self.waves {
                        let k = (-((r - rw) * 14.0).powi(2)).exp() * (1.0 - (rw - 0.95) / 2.05);
                        col = add(col, scale(wave_col, k * 0.8));
                    }
                    col
                } else if r > 0.93 {
                    // Metal frame: a rounded ring.
                    let s = (r - 0.965) / 0.035;
                    let l = shade([ux * s, uy * s, 1.0]);
                    rgb(0.2 + 0.6 * l, 0.2 + 0.6 * l, 0.22 + 0.62 * l)
                } else if r > 0.8 {
                    // Rubber surround: a half torus that rolls with the cone.
                    let s = (r - 0.8) / 0.13;
                    let tilt = -(s * std::f32::consts::PI).cos() * (1.2 + e * 0.5);
                    let l = shade([ux * tilt, uy * tilt, 1.0]);
                    scale(hsv(self.hue, 0.2, 1.0), 0.06 + 0.4 * l)
                } else if r > cap_r {
                    // Cone: slopes towards the center, deeper with excursion.
                    let l = shade([-ux * slope, -uy * slope, 1.0]);
                    let ribs = 0.05 * (r * 70.0).sin();
                    let u = (r - cap_r) / (0.8 - cap_r).max(0.01);
                    let ripple = 0.12 * sample(&f.waveform, u) * (0.3 + f.level);
                    scale(cone_col, (0.12 + 0.7 * l + ribs + ripple + glow).clamp(0.0, 1.0))
                } else {
                    // Dust cap: a dome with a specular highlight.
                    let s = (r / cap_r).min(1.0);
                    let nz = (1.0 - s * s).sqrt();
                    let n = [dx / cap_r, dy / cap_r, nz * (1.2 - e * 0.4)];
                    let l = shade(n);
                    let spec = l.powi(16) * 0.9;
                    add(scale(cap_col, (0.12 + 0.65 * l + glow).min(1.0)), rgb(spec, spec, spec))
                };
                c.set(x, y, col);
            }
        }
    }
}
