use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};

/// Lissajous frequency pairs, one per light.
const PATHS: [(f32, f32); 6] = [(3.0, 2.0), (5.0, 4.0), (1.0, 2.0), (3.0, 4.0), (2.0, 5.0), (4.0, 3.0)];

struct Light {
    phase: f32,
    hue: f32,
    prev: Option<(f32, f32)>,
}

/// Long-exposure light painting: glowing points race along wobbling
/// Lissajous paths and leave slowly fading trails. Bass speeds them up,
/// each light's brightness follows its own band, beats flare them.
pub struct LightTrails {
    lights: Vec<Light>,
    acc: Vec<[f32; 3]>,
    w: usize,
    h: usize,
    time: f32,
}

impl LightTrails {
    pub fn new() -> Self {
        let lights = (0..PATHS.len())
            .map(|i| Light {
                phase: i as f32 * 1.3,
                hue: i as f32 / PATHS.len() as f32,
                prev: None,
            })
            .collect();
        Self {
            lights,
            acc: Vec::new(),
            w: 0,
            h: 0,
            time: 0.0,
        }
    }
}

/// Position of light `i` at path phase `p`, in canvas pixels.
fn path(i: usize, p: f32, time: f32, w: f32, h: f32) -> (f32, f32) {
    let (a, b) = PATHS[i % PATHS.len()];
    let wob = (p * 0.37 + i as f32).sin() * 0.12;
    let x = (a * p + i as f32).sin() * (0.82 + wob) + (p * 7.1 + time).sin() * 0.04;
    let y = (b * p).sin() * (0.82 - wob) + (p * 5.3 - time).cos() * 0.04;
    // Slow rotation of the whole figure.
    let (s, c) = (time * 0.05 + i as f32 * 0.4).sin_cos();
    let (rx, ry) = (x * c - y * s * 0.3, y * c + x * s * 0.3);
    (w * (0.5 + 0.46 * rx), h * (0.5 + 0.44 * ry))
}

impl Effect for LightTrails {
    fn name(&self) -> &'static str {
        "Light Trails"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (w, h) = (c.w, c.h);
        if (w, h) != (self.w, self.h) {
            (self.w, self.h) = (w, h);
            self.acc = vec![[0.0; 3]; w * h];
            for l in &mut self.lights {
                l.prev = None;
            }
        }
        self.time += dt;

        let keep = (-dt * (0.45 + 0.5 * f.pulse)).exp();
        for a in &mut self.acc {
            for v in a.iter_mut() {
                *v *= keep;
            }
        }

        let (wf, hf) = (w as f32, h as f32);
        let radius = (wf.min(hf) * 0.014).max(0.6);
        let reach = (radius * 2.5).ceil() as i32;
        let inv_r2 = 1.0 / (radius * radius);
        let speed = 0.35 + 1.1 * f.bass + 0.5 * f.pulse;
        let n = self.lights.len();
        let mut heads = Vec::with_capacity(n);
        for (i, l) in self.lights.iter_mut().enumerate() {
            l.hue += dt * 0.03;
            let p0 = l.phase;
            l.phase += dt * speed * (0.8 + 0.08 * i as f32);
            let band = f.spectrum.get(4 + i * 9).copied().unwrap_or(0.0);
            let energy = 0.35 + 0.9 * band + 0.6 * f.pulse;
            let col = hsv(l.hue, 0.75, 1.0).map(|v| v as f32 / 255.0 * energy * 0.2);

            // Walk the curve in pixel-sized steps so trails stay smooth even on long frames.
            let start = l.prev.unwrap_or_else(|| path(i, p0, self.time, wf, hf));
            let end = path(i, l.phase, self.time, wf, hf);
            let span = ((end.0 - start.0).abs() + (end.1 - start.1).abs()).max(1.0);
            let steps = ((span / (radius * 0.4)).ceil() as usize).clamp(1, 400);
            for k in 1..=steps {
                let p = p0 + (l.phase - p0) * k as f32 / steps as f32;
                let (sx, sy) = path(i, p, self.time, wf, hf);
                for dy in -reach..=reach {
                    let py = sy as i32 + dy;
                    if py < 0 || py >= h as i32 {
                        continue;
                    }
                    for dx in -reach..=reach {
                        let px = sx as i32 + dx;
                        if px < 0 || px >= w as i32 {
                            continue;
                        }
                        let d2 = (px as f32 + 0.5 - sx).powi(2) + (py as f32 + 0.5 - sy).powi(2);
                        let g = (-d2 * inv_r2).exp();
                        let a = &mut self.acc[py as usize * w + px as usize];
                        for (v, c) in a.iter_mut().zip(col) {
                            *v += c * g;
                        }
                    }
                }
            }
            l.prev = Some(end);
            heads.push((end, col, energy));
        }

        for (p, a) in c.px.iter_mut().zip(&self.acc) {
            *p = rgb(1.0 - (-a[0]).exp(), 1.0 - (-a[1]).exp(), 1.0 - (-a[2]).exp());
        }
        // White-hot light heads on top.
        for ((x, y), col, energy) in heads {
            let r = radius * (1.2 + 0.8 * f.pulse);
            let reach = (r * 2.0).ceil() as i32;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let d2 = (dx * dx + dy * dy) as f32 / (r * r);
                    let g = (-d2 * 1.5).exp() * energy.min(1.5);
                    let tint = scale_rgb(col, 3.0 * g);
                    c.plot_add(
                        x as i32 + dx,
                        y as i32 + dy,
                        rgb(g * 0.8 + tint[0], g * 0.8 + tint[1], g * 0.8 + tint[2]),
                    );
                }
            }
        }
    }
}

fn scale_rgb(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}
