use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, scale};
use std::f32::consts::TAU;

const RINGS: usize = 36;
const DOTS: usize = 18;
const SPACING: f32 = 0.45;

/// Center of the wormhole at depth `s`, so the tube bends left and right.
fn path(s: f64, t: f32) -> (f32, f32) {
    let t = t as f64;
    (
        ((s * 0.21 + t * 0.2).sin() * 1.1 + (s * 0.07).sin() * 0.8) as f32,
        ((s * 0.17 + t * 0.15).cos() * 0.8) as f32,
    )
}

/// Flight through a curving wormhole built from rings of glowing dots whose
/// twisted arms form spirals. Each ring takes the brightness of one spectrum
/// band (near = bass), the level sets the speed and every beat snaps the
/// twist to a new value.
pub struct Wormhole {
    /// Distance travelled; f64 so long runs keep their precision.
    offset: f64,
    spin: f32,
    twist: f32,
    target: f32,
    hue: f32,
    rng: Rng,
}

impl Wormhole {
    pub fn new() -> Self {
        Self {
            offset: 0.0,
            spin: 0.0,
            twist: 0.3,
            target: 0.3,
            hue: 0.0,
            rng: Rng::new(0x5EED_0007),
        }
    }
}

impl Effect for Wormhole {
    fn name(&self) -> &'static str {
        "Wormhole"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fade(0.5);
        if f.beat {
            self.target = self.rng.range(-0.9, 0.9);
        }
        self.twist += (self.target - self.twist) * (1.0 - (-4.0 * dt).exp());
        self.offset += (dt * (1.2 + 5.0 * f.level + 2.0 * f.pulse)) as f64;
        self.spin += dt * (0.3 + 1.2 * f.mid);
        self.hue += dt * 0.03;

        let (w, h) = (c.w as f32, c.h as f32);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let focal = w.min(h) * 0.55;
        let cam = path(self.offset, t);
        let first = (self.offset / SPACING as f64).floor() as i64 + 1;
        let bins = f.spectrum.len();
        let zmax = RINGS as f32 * SPACING;

        let mut prev: Option<Vec<(f32, f32)>> = None;
        // Far to near, so arms can connect each ring to the one behind it.
        for k in (0..RINGS).rev() {
            let id = first + k as i64;
            let s = id as f64 * SPACING as f64;
            let z = (s - self.offset) as f32;
            if z < 0.12 {
                prev = None;
                continue;
            }
            let p = path(s, t);
            let (ox, oy) = (p.0 - cam.0, p.1 - cam.1);
            let e = if bins > 0 { f.spectrum[k * bins / RINGS] } else { 0.0 };
            let near = 1.0 - z / zmax;
            let v = (0.25 + 0.75 * e) * near.clamp(0.0, 1.0).powf(0.7);
            let col = hsv(self.hue + (id % 1000) as f32 * 0.021, 0.75, v);
            let d = focal / z;
            let rr = (focal * 0.035 / z * (1.0 + 0.6 * f.pulse)).clamp(0.5, 4.0);
            let pts: Vec<(f32, f32)> = (0..DOTS)
                .map(|j| {
                    let a = j as f32 / DOTS as f32 * TAU + self.spin + self.twist * z;
                    (cx + (ox + a.cos()) * d, cy + (oy + a.sin()) * d)
                })
                .collect();
            if let Some(back) = &prev {
                let arm = scale(col, 0.35);
                for (a, b) in back.iter().zip(&pts) {
                    let far = (a.0 - b.0).abs() + (a.1 - b.1).abs();
                    if far < w + h {
                        c.line_add(a.0, a.1, b.0, b.1, arm);
                    }
                }
            }
            for &(x, y) in &pts {
                if x < -rr || y < -rr || x > w + rr || y > h + rr {
                    continue;
                }
                let r = rr.ceil() as i32;
                for py in -r..=r {
                    for px in -r..=r {
                        let q = (px * px + py * py) as f32 / (rr * rr);
                        if q <= 1.0 {
                            c.plot_add(x as i32 + px, y as i32 + py, scale(col, 1.0 - q * 0.6));
                        }
                    }
                }
            }
            prev = Some(pts);
        }
    }
}
