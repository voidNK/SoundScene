use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, rgb};
use std::f32::consts::PI;

/// Pixels per sample of the source image along each axis.
const CELL: usize = 2;
/// Screen angles of the cyan, magenta, yellow and black plates (radians).
const ANGLES: [f32; 4] = [PI / 12.0, PI * 5.0 / 12.0, 0.0, PI / 4.0];
/// What each ink lets through of red, green and blue.
const INKS: [[f32; 3]; 4] = [
    [0.0, 0.66, 0.92],
    [0.9, 0.1, 0.55],
    [1.0, 0.92, 0.05],
    [0.12, 0.12, 0.14],
];
const PAPER: [f32; 3] = [0.97, 0.95, 0.89];

/// Four-color halftone print of moving plasma blobs: each ink plate is a
/// rotated dot grid whose dot sizes follow the image. Bass swells the dots
/// and the blobs, beats punch the screen ruling, the level spins the grids.
pub struct Halftone {
    /// CMYK coverage of the source image, 0..1 per plate.
    img: Vec<[f32; 4]>,
    time: f32,
    rot: f32,
    punch: f32,
}

impl Halftone {
    pub fn new() -> Self {
        Self {
            img: Vec::new(),
            time: 0.0,
            rot: 0.0,
            punch: 0.0,
        }
    }
}

impl Effect for Halftone {
    fn name(&self) -> &'static str {
        "Halftone"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt * (0.4 + 0.8 * f.bass);
        self.rot += dt * (0.03 + 0.12 * f.level);
        if f.beat {
            self.punch = 1.0;
        }
        self.punch *= (-dt * 5.0).exp();

        // Source image: a hue plasma under a few bright blobs, sampled coarsely.
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        self.img.resize(gw * gh, [0.0; 4]);
        let s = (gw.min(gh) as f32).max(1.0);
        let t = self.time;
        let blobs: Vec<(f32, f32, f32)> = (0..4)
            .map(|i| {
                let k = i as f32;
                (
                    gw as f32 * (0.5 + 0.38 * (t * (0.31 + 0.07 * k) + k * 1.9).sin()),
                    gh as f32 * (0.5 + 0.36 * (t * (0.23 + 0.05 * k) + k * 2.7).cos()),
                    s * (0.22 + 0.1 * f.bass),
                )
            })
            .collect();
        for y in 0..gh {
            for x in 0..gw {
                let (fx, fy) = (x as f32 / s, y as f32 / s);
                let v = ((fx * 3.1 + t).sin() + (fy * 2.7 - t * 0.8).sin() + ((fx + fy) * 2.0 + t * 0.6).sin()) / 3.0;
                let mut glow = 0.0;
                for &(bx, by, br) in &blobs {
                    let d2 = ((x as f32 - bx).powi(2) + (y as f32 - by).powi(2)) / (br * br);
                    glow += (-d2).exp();
                }
                let col = hsv(
                    0.6 + v * 0.45 + t * 0.03,
                    0.65 - 0.35 * glow.min(1.0),
                    0.6 + 0.4 * glow.min(1.0),
                );
                let (cy, mg, ye) = (
                    1.0 - col[0] as f32 / 255.0,
                    1.0 - col[1] as f32 / 255.0,
                    1.0 - col[2] as f32 / 255.0,
                );
                let k = cy.min(mg).min(ye) * 0.4;
                self.img[y * gw + x] = [cy - k, mg - k, ye - k, k];
            }
        }

        // Rotated dot screens. Cell size follows the canvas so tiles keep a few dots.
        let cell = ((c.w.min(c.h * 2) as f32 / 20.0).max(4.0)) * (1.0 - 0.25 * self.punch);
        let gain = 0.56 + 0.12 * f.bass;
        let plates: Vec<(f32, f32)> = ANGLES.iter().map(|a| (a + self.rot).sin_cos()).collect();
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        for y in 0..c.h {
            let py = y as f32 + 0.5 - cy;
            for x in 0..c.w {
                let px = x as f32 + 0.5 - cx;
                let mut out = PAPER;
                for (p, &(sa, ca)) in plates.iter().enumerate() {
                    let u = (px * ca + py * sa) / cell;
                    let v = (-px * sa + py * ca) / cell;
                    let (cu, cv) = (u.floor() + 0.5, v.floor() + 0.5);
                    // Dot center back in pixel space, looked up in the source image.
                    let sx = ((cu * ca - cv * sa) * cell + cx) / CELL as f32;
                    let sy = ((cu * sa + cv * ca) * cell + cy) / CELL as f32;
                    let ix = (sx.max(0.0) as usize).min(gw - 1);
                    let iy = (sy.max(0.0) as usize).min(gh - 1);
                    let val = self.img[iy * gw + ix][p];
                    let r = val.max(0.0).sqrt() * gain;
                    let d = ((u - cu).powi(2) + (v - cv).powi(2)).sqrt();
                    let cover = ((r - d) * cell + 0.5).clamp(0.0, 1.0);
                    if cover > 0.0 {
                        for (o, ink) in out.iter_mut().zip(INKS[p]) {
                            *o *= 1.0 - cover * (1.0 - ink);
                        }
                    }
                }
                c.set(x, y, rgb(out[0], out[1], out[2]));
            }
        }
    }
}
