use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};

/// Distance from `p` to the segment `a`–`b`.
fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let k = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy).max(1e-6)).clamp(0.0, 1.0);
    ((p.0 - a.0 - dx * k).powi(2) + (p.1 - a.1 - dy * k).powi(2)).sqrt()
}

/// Signed side of `p` relative to the edge `a`–`b`.
fn side(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
}

fn lerp(a: (f32, f32), b: (f32, f32), k: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k)
}

/// A beam of white light entering a glass prism and fanning out into a
/// rainbow. The level drives the beam, bass and beats widen the fan, and
/// each color of the rainbow is lit by its own slice of the spectrum.
pub struct Prism {
    time: f32,
    spread: f32,
}

impl Prism {
    pub fn new() -> Self {
        Self { time: 0.0, spread: 0.2 }
    }
}

impl Effect for Prism {
    fn name(&self) -> &'static str {
        "Prism"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        let target = 0.22 + 0.35 * f.bass + 0.2 * f.pulse;
        self.spread += (target - self.spread) * (1.0 - (-dt * 8.0).exp());

        let (wf, hf) = (c.w as f32, c.h as f32);
        let s = wf.min(hf);
        let side_len = (wf * 0.4).min(hf * 0.85);
        let tri_h = side_len * 0.866;
        let (cx, cy) = (wf * 0.42, hf * 0.55);
        let rot = 0.07 * (self.time * 0.3).sin();
        let (sr, cr) = rot.sin_cos();
        let place = |x: f32, y: f32| (cx + x * cr - y * sr, cy + x * sr + y * cr);
        let a = place(0.0, -tri_h * 2.0 / 3.0);
        let b = place(-side_len * 0.5, tri_h / 3.0);
        let cc = place(side_len * 0.5, tri_h / 3.0);

        // Beam path: in through the left face, out through the right one.
        let entry = lerp(a, b, 0.55);
        let exit = lerp(a, cc, 0.62);
        let src = (-wf * 0.1, entry.1 + (entry.0 + wf * 0.1) * 0.22);
        let beam_w = (s * 0.012 * (0.7 + 0.8 * f.level)).max(0.5);
        let beam_i = 0.55 + 0.6 * f.level + 0.4 * f.pulse;
        let base = (exit.1 - entry.1).atan2(exit.0 - entry.0) + 0.38 + rot;
        let spread = self.spread;
        let orient = side(cc, a, b).signum();
        let spec = &f.spectrum;
        let band = |u: f32| {
            if spec.is_empty() {
                return 0.0;
            }
            let p = (u.clamp(0.0, 1.0) * 47.0).min(spec.len() as f32 - 1.0);
            let i = p as usize;
            let j = (i + 1).min(spec.len() - 1);
            spec[i] + (spec[j] - spec[i]) * (p - i as f32)
        };

        for y in 0..c.h {
            let fy = y as f32 + 0.5;
            let vig = 0.035 + 0.02 * (1.0 - (fy / hf - 0.5).abs() * 2.0);
            for x in 0..c.w {
                let p = (x as f32 + 0.5, fy);
                let mut col = [vig * 0.6, vig * 0.6, vig];
                let (e0, e1, e2) = (side(p, a, b) * orient, side(p, b, cc) * orient, side(p, cc, a) * orient);
                let inside = e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0;

                if inside {
                    // Glass body and the beam crossing it, already tinted.
                    col = [0.07, 0.09, 0.12];
                    let d = seg_dist(p, entry, exit);
                    let k = (-(d * d) / (beam_w * beam_w * 2.0)).exp() * beam_i;
                    let along = ((p.0 - entry.0) / (exit.0 - entry.0).abs().max(1.0)).clamp(0.0, 1.0);
                    let tint = hsv(0.6 * along, 0.3 * along, 1.0);
                    for (ch, o) in col.iter_mut().enumerate() {
                        *o += k * tint[ch] as f32 / 255.0;
                    }
                } else {
                    // Incoming white beam.
                    if p.0 < entry.0 + beam_w * 2.0 {
                        let d = seg_dist(p, src, entry);
                        let k = (-(d * d) / (beam_w * beam_w)).exp() * beam_i
                            + (-(d * d) / (beam_w * beam_w * 30.0)).exp() * 0.12 * beam_i;
                        for o in col.iter_mut() {
                            *o += k;
                        }
                    }
                    // The rainbow fan, one spectrum slice per color.
                    let (dx, dy) = (p.0 - exit.0, p.1 - exit.1);
                    if dx > 0.0 {
                        let mut ang = dy.atan2(dx) - base;
                        ang = (ang + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                        let u = ang / spread + 0.5;
                        let dist = (dx * dx + dy * dy).sqrt();
                        let edge_px = dist * spread;
                        let soft = (u.min(1.0 - u) * edge_px * 0.8 + 0.5).clamp(0.0, 1.0);
                        if soft > 0.0 {
                            let hue = u.clamp(0.0, 1.0) * 0.78;
                            let k = soft * (0.3 + 1.0 * band(u)) * (0.7 + 0.3 * beam_i);
                            let near = (dist / (s * 0.08)).min(1.0);
                            let rc = hsv(hue, 0.9 - 0.5 * (1.0 - near), 1.0);
                            for (ch, o) in col.iter_mut().enumerate() {
                                *o += k * rc[ch] as f32 / 255.0;
                            }
                        }
                        // Faint glow around the fan.
                        let out = (u.abs().min((u - 1.0).abs())) * edge_px;
                        if !(0.0..=1.0).contains(&u) {
                            let g = (-out * 0.15).exp() * 0.08 * beam_i;
                            for o in col.iter_mut() {
                                *o += g;
                            }
                        }
                    }
                }

                // Bright glass edges: signed distance to the nearest face, in pixels.
                let edge = e0.min(e1).min(e2) / side_len.max(1.0);
                let rim = (1.0 - (edge - 0.2).abs() / 1.0).max(0.0);
                if rim > 0.0 {
                    let g = rim * (0.55 + 0.25 * f.pulse);
                    col[0] += g * 0.7;
                    col[1] += g * 0.85;
                    col[2] += g;
                }
                c.set(x, y, rgb(col[0], col[1], col[2]));
            }
        }
    }
}
