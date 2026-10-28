use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix, rgb, scale};

const FAR: f32 = 80.0;
const CAMERA_HEIGHT: f32 = 8.0;

/// Sum of sines; roughly -1.75..1.75.
fn terrain(x: f32, z: f32) -> f32 {
    (x * 0.07).sin() * (z * 0.05).cos()
        + (x * 0.15 + z * 0.11).sin() * 0.5
        + (x * 0.31 - z * 0.27).sin() * 0.25
        + (x * 0.73 + z * 0.61).sin() * 0.08
}

/// Height 0..1 → water, beach, grass, rock, snow.
fn ground_color(n: f32) -> Rgb {
    let stops = [
        (0.0, rgb(0.03, 0.10, 0.35)),
        (0.30, rgb(0.10, 0.35, 0.60)),
        (0.34, rgb(0.80, 0.72, 0.45)),
        (0.42, rgb(0.20, 0.55, 0.18)),
        (0.68, rgb(0.42, 0.32, 0.22)),
        (0.85, rgb(0.55, 0.52, 0.52)),
        (1.0, rgb(1.0, 1.0, 1.0)),
    ];
    let n = n.clamp(0.0, 1.0);
    let i = stops.iter().rposition(|&(at, _)| at <= n).unwrap_or(0).min(stops.len() - 2);
    let ((a, ca), (b, cb)) = (stops[i], stops[i + 1]);
    mix(ca, cb, (n - a) / (b - a))
}

/// Comanche-style voxel landscape flyover. Bass speeds up the flight and
/// raises the mountains.
pub struct Voxel {
    z: f32,
    hue: f32,
}

impl Voxel {
    pub fn new() -> Self {
        Self { z: 0.0, hue: 0.0 }
    }
}

impl Effect for Voxel {
    fn name(&self) -> &'static str {
        "Voxel"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.z += dt * (6.0 + f.bass * 25.0);
        self.hue += dt * 0.01 + if f.beat { 0.03 } else { 0.0 };
        let (w, h) = (c.w as f32, c.h as f32);
        let horizon = h * 0.35;
        let sky_top = rgb(0.05, 0.05, 0.2);
        let sky_low = hsv(self.hue + 0.9, 0.5, 0.6);

        for y in 0..c.h {
            let col = mix(sky_top, sky_low, y as f32 / horizon.max(1.0));
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }

        let amp = 6.0 * (1.0 + 0.8 * f.bass);
        let cam_x = (t * 0.2).sin() * 20.0;
        let height_scale = h * 0.3;
        // Front to back per column; only draw what pokes above what's
        // already drawn (`ybuf`).
        for sx in 0..c.w {
            let ray = (sx as f32 / w - 0.5) * 1.4;
            let mut ybuf = h;
            let mut d = 1.0;
            let mut last_height = terrain(cam_x + ray * d, self.z + d);
            while d < FAR {
                let height = terrain(cam_x + ray * d, self.z + d);
                // Slopes facing the camera (rising along the ray) are lit.
                let shade = (0.8 + (height - last_height) * amp * 0.4).clamp(0.45, 1.3);
                last_height = height;
                let sy = horizon + (CAMERA_HEIGHT - height * amp) / d * height_scale;
                if sy < ybuf {
                    let ground = scale(ground_color((height / 1.83 + 1.0) * 0.5), shade);
                    let col = mix(ground, sky_low, (d / FAR).powf(1.5));
                    for y in sy.max(0.0) as usize..(ybuf as usize).min(c.h) {
                        c.set(sx, y, col);
                    }
                    ybuf = sy.max(0.0);
                }
                d += 0.02 * d + 0.3;
            }
        }
    }
}
