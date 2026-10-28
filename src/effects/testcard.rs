use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, rgb, scale};

/// 75% color bars: white, yellow, cyan, green, magenta, red, blue.
const BARS: [Rgb; 7] = [
    [191, 191, 191],
    [191, 191, 0],
    [0, 191, 191],
    [0, 191, 0],
    [191, 0, 191],
    [191, 0, 0],
    [0, 0, 191],
];
/// Stripe periods of the resolution gratings, in pixels.
const PERIODS: [f32; 5] = [10.0, 7.0, 5.0, 3.0, 2.0];

struct Tear {
    y0: f32,
    y1: f32,
    shift: f32,
}

/// Static geometry of the card for the current canvas size.
struct Layout {
    cx: f32,
    cy: f32,
    r: f32,
    grid: f32,
}

/// A retro TV test card: grid, circle, color bars, gray steps, resolution
/// gratings and a level bar. The gratings' contrast follows the spectrum,
/// the picture wobbles line by line with the bass, and every beat hits it
/// with a signal glitch: torn lines, color fringing, a vertical roll and snow.
pub struct TestCard {
    rng: Rng,
    glitch: f32,
    roll: f32,
    tears: Vec<Tear>,
    hum: f32,
}

impl TestCard {
    pub fn new() -> Self {
        Self {
            rng: Rng::new(0x7E57),
            glitch: 0.0,
            roll: 0.0,
            tears: Vec::new(),
            hum: 0.0,
        }
    }
}

fn card(l: &Layout, x: f32, y: f32, gratings: &[f32; 5], level: f32) -> Rgb {
    let (dx, dy) = (x - l.cx, y - l.cy);
    let r2 = dx * dx + dy * dy;
    let ring = l.r + 1.2;
    if r2 > l.r * l.r {
        if r2 < ring * ring {
            return [235, 235, 235];
        }
        let (gx, gy) = (
            (dx / l.grid).rem_euclid(1.0) * l.grid,
            (dy / l.grid).rem_euclid(1.0) * l.grid,
        );
        if gx < 1.0 || gy < 1.0 {
            return [220, 220, 220];
        }
        // Black and white castellations along the top and bottom edges.
        let edge = l.cy - l.grid * 0.35;
        if dy.abs() > edge {
            let k = ((dx / (l.grid * 0.5)).floor() as i32).rem_euclid(2);
            return if k == 0 { [235, 235, 235] } else { [10, 10, 10] };
        }
        return [96, 96, 104];
    }

    if (dx.abs() < 0.6 && dy.abs() < l.r * 0.3) || (dy.abs() < 0.6 && dx.abs() < l.r * 0.3) {
        return [255, 255, 255];
    }
    let rc = r2.sqrt();
    if (rc - l.r * 0.2).abs() < 0.6 {
        return [255, 255, 255];
    }

    let (u, v) = (dx / l.r, dy / l.r);
    let across = ((u + 1.0) * 0.5).clamp(0.0, 0.999);
    if v < -0.62 {
        let k = (across * 6.0) as usize;
        let g = k as f32 / 5.0;
        rgb(g, g, g)
    } else if v < -0.1 {
        BARS[(across * 7.0) as usize % 7]
    } else if v < 0.08 {
        let k = ((u * 6.0).floor() as i32).rem_euclid(2);
        if k == 0 { [20, 20, 20] } else { [200, 200, 200] }
    } else if v < 0.48 {
        let k = (across * 5.0) as usize % 5;
        let stripe = if (x / PERIODS[k]).floor() as i32 % 2 == 0 {
            0.5
        } else {
            -0.5
        };
        let g = 0.5 + stripe * gratings[k];
        rgb(g, g, g)
    } else if v < 0.75 {
        hsv(across * 0.85, 0.9, 0.85)
    } else if across < level {
        if across > 0.8 { [235, 40, 30] } else { [40, 210, 60] }
    } else {
        [16, 24, 16]
    }
}

impl Effect for TestCard {
    fn name(&self) -> &'static str {
        "Test Card"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let (w, h) = (c.w as f32, c.h as f32);
        let r = (h * 0.44).min(w * 0.3).max(1.0);
        let layout = Layout {
            cx: w * 0.5,
            cy: h * 0.5,
            r,
            grid: (r / 3.0).max(2.0),
        };

        let bins = f.spectrum.len();
        let mut gratings = [0.5; 5];
        for (k, g) in gratings.iter_mut().enumerate() {
            let (a, b) = (k * bins / 5, ((k + 1) * bins / 5).min(bins));
            if a < b {
                let band = f.spectrum[a..b].iter().sum::<f32>() / (b - a) as f32;
                *g = (0.15 + 0.85 * band).clamp(0.0, 1.0);
            }
        }

        self.glitch = (self.glitch - dt * 2.5).max(0.0);
        if f.beat {
            self.glitch = 1.0;
            self.tears.clear();
            for _ in 0..2 + self.rng.next_u32() % 4 {
                let y0 = self.rng.range(0.0, h);
                self.tears.push(Tear {
                    y0,
                    y1: y0 + self.rng.range(1.0, h * 0.12),
                    shift: self.rng.range(-0.25, 0.25) * w,
                });
            }
            if self.rng.f32() < 0.35 {
                self.roll = h * self.rng.range(0.2, 0.6);
            }
        }
        // A rolled picture settles back down.
        self.roll *= (1.0 - dt * 4.0).clamp(0.0, 1.0);
        self.hum = (self.hum + dt * 0.15).rem_euclid(1.0);

        let g = self.glitch;
        let fringe = (g * 4.0 + f.treble * 1.5).round();
        let wobble = w * 0.025 * (0.3 * f.level + f.bass);
        for y in 0..c.h {
            let yf = y as f32;
            let spec = if bins == 0 {
                0.0
            } else {
                f.spectrum[(y * bins / c.h).min(bins - 1)]
            };
            let mut shift = wobble * spec * (yf * 0.35 + t * 9.0).sin();
            for tear in &self.tears {
                if yf >= tear.y0 && yf < tear.y1 {
                    shift += tear.shift * g;
                }
            }
            let sy = (yf + self.roll).rem_euclid(h);
            let hum = 1.0 - 0.18 * (((yf / h - self.hum) * std::f32::consts::TAU).cos() * 0.5 + 0.5).powi(8);
            let snow = g * g * 0.35;
            for x in 0..c.w {
                let sx = x as f32 - shift;
                let col = if fringe > 0.0 {
                    let rr = card(&layout, sx + fringe, sy, &gratings, f.level);
                    let gg = card(&layout, sx, sy, &gratings, f.level);
                    let bb = card(&layout, sx - fringe, sy, &gratings, f.level);
                    [rr[0], gg[1], bb[2]]
                } else {
                    card(&layout, sx, sy, &gratings, f.level)
                };
                let mut col = scale(col, hum);
                if snow > 0.0 && self.rng.f32() < snow {
                    let n = (self.rng.next_u32() & 0xFF) as u8;
                    col = [n, n, n];
                }
                c.set(x, y, col);
            }
        }
    }
}
