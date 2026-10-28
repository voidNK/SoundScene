use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, rgb};
use std::f32::consts::PI;

const BEAMS: usize = 6;
const EMITTERS: [f32; 3] = [0.5, 0.06, 0.94];
const PALETTES: [[[f32; 3]; 2]; 5] = [
    [[0.1, 1.0, 0.2], [0.4, 1.0, 0.1]],
    [[1.0, 0.1, 0.15], [0.2, 0.3, 1.0]],
    [[0.0, 0.9, 1.0], [1.0, 0.1, 0.9]],
    [[1.0, 0.8, 0.3], [1.0, 0.95, 0.9]],
    [[0.6, 0.1, 1.0], [0.1, 1.0, 0.6]],
];

/// Laser show in a hazy hall: fans of beams sweep from the stage behind a
/// silhouetted crowd. Each beam's brightness follows a spectrum band, beats
/// flash all beams and bounce the crowd, and every few beats the show
/// switches to a new formation and color scheme.
pub struct Lasers {
    pattern: usize,
    palette: usize,
    beats: u32,
    sweep: f32,
    flash: f32,
    rng: Rng,
    buf: Vec<[f32; 3]>,
}

impl Lasers {
    pub fn new() -> Self {
        Self {
            pattern: 0,
            palette: 0,
            beats: 0,
            sweep: 0.0,
            flash: 0.0,
            rng: Rng::new(0x1A5E),
            buf: Vec::new(),
        }
    }

    /// Angle (0 = straight up, positive = right) of beam `j` of emitter `e`.
    fn angle(&self, e: usize, j: usize) -> Option<f32> {
        let u = j as f32 / (BEAMS - 1) as f32 - 0.5;
        let s = self.sweep;
        let side = [0.0, 1.0, -1.0][e];
        match self.pattern {
            // Three fans breathing open and closed.
            0 => Some(u * (0.8 + 0.5 * (s * 0.7).sin()) + side * 0.45 + 0.3 * (s + e as f32).sin()),
            // Corner scissors.
            1 => (e > 0).then(|| side * (0.55 + 0.4 * (s * 1.3).sin()) + u * 0.35),
            // One wide spinning fan from the center.
            2 => (e == 0).then(|| (u * 2.6 + s * 0.9).rem_euclid(2.6) - 1.3),
            // Parallel curtain sweeping side to side.
            _ => Some(0.6 * (s * 0.8).sin() + u * 0.3 + side * 0.15),
        }
    }
}

impl Effect for Lasers {
    fn name(&self) -> &'static str {
        "Lasers"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.sweep += dt * (0.5 + 1.2 * f.mid + 1.5 * f.pulse);
        self.flash = (self.flash - dt * 4.0).max(0.0);
        if f.beat {
            self.flash = 1.0;
            self.beats += 1;
            if self.beats.is_multiple_of(8) {
                self.pattern = (self.pattern + 1 + self.rng.next_u32() as usize % 3) % 4;
                self.palette = (self.palette + 1 + self.rng.next_u32() as usize % 4) % PALETTES.len();
            }
        }

        let (w, h) = (c.w, c.h);
        let (wf, hf) = (w as f32, h as f32);
        self.buf.clear();
        self.buf.resize(w * h, [0.0; 3]);
        if w == 0 || h == 0 {
            return;
        }
        let bins = f.spectrum.len();
        let width = 1.0 + 0.5 * f.bass;
        let k = 1.5 / (width * width);
        let reach = 8.0 * width;

        // Slowly drifting smoke that lights up where beams pass through.
        let col_haze: Vec<f32> = (0..w).map(|x| (x as f32 * 0.06 + t * 0.3).sin()).collect();
        let row_haze: Vec<f32> = (0..h).map(|y| (y as f32 * 0.09 - t * 0.2).sin()).collect();

        for (e, &ex) in EMITTERS.iter().enumerate() {
            let (ox, oy) = (ex * wf, hf * 0.98);
            for j in 0..BEAMS {
                let Some(a) = self.angle(e, j) else { continue };
                let a = a.clamp(-PI * 0.48, PI * 0.48);
                let (dx, dy) = (a.sin(), -a.cos());
                let band = if bins == 0 {
                    0.5
                } else {
                    f.spectrum[(e * BEAMS + j) * 3 % bins]
                };
                let power = 0.25 + 0.75 * band + 0.8 * self.flash;
                let pal = PALETTES[self.palette][(j + e) % 2];
                for (y, &rh) in row_haze.iter().enumerate() {
                    let py = y as f32 + 0.5 - oy;
                    // Columns where this row is within reach of the beam.
                    let (lo, hi) = if dy.abs() > 1e-4 {
                        let tpar = py / dy;
                        if tpar < 0.0 {
                            continue;
                        }
                        let xc = ox + dx * tpar;
                        let spread = reach / dy.abs().max(0.05);
                        (xc - spread, xc + spread)
                    } else {
                        (0.0, wf)
                    };
                    let (lo, hi) = (lo.max(0.0) as usize, (hi.max(0.0) as usize + 1).min(w));
                    for (x, &ch) in col_haze.iter().enumerate().take(hi).skip(lo) {
                        let px = x as f32 + 0.5 - ox;
                        let along = px * dx + py * dy;
                        if along < 0.0 {
                            continue;
                        }
                        let d = px * dy - py * dx;
                        let g = 1.0 / (1.0 + d * d * k) - 0.01;
                        if g <= 0.0 {
                            continue;
                        }
                        let haze = 0.55 + 0.45 * ch * rh;
                        let v = g * power * haze / (1.0 + along * 0.004);
                        let p = &mut self.buf[y * w + x];
                        p[0] += pal[0] * v;
                        p[1] += pal[1] * v;
                        p[2] += pal[2] * v;
                    }
                }
            }
        }

        // Crowd silhouette: a row of bouncing heads and shoulders.
        let spacing = (hf * 0.12).max(2.0);
        let base = hf * 0.95;
        let crowd = |x: f32, y: f32| -> bool {
            if y > base {
                return true;
            }
            let i = (x / spacing).floor();
            let r = ((i * 12.9898).sin() * 43_758.547).fract().abs();
            let cx = (i + 0.5 + (r - 0.5) * 0.3) * spacing;
            let bounce = f.pulse * spacing * 0.5 * r;
            let cy = base - spacing * (0.35 + 0.3 * r) - bounce;
            let rad = spacing * 0.38;
            let (dx, dy) = (x - cx, y - cy);
            dx * dx + dy * dy < rad * rad || (y > cy + rad * 0.5 && dx.abs() < rad * 0.5)
        };

        for y in 0..h {
            for x in 0..w {
                let col = if crowd(x as f32 + 0.5, y as f32 + 0.5) {
                    [0, 0, 0]
                } else {
                    let p = self.buf[y * w + x];
                    // Soft saturation, so crossing beams burn towards white.
                    let tone = |v: f32| 1.0 - (-v * 1.2).exp();
                    let white = tone((p[0] + p[1] + p[2]) * 0.15);
                    let bg = 0.02 + 0.03 * self.flash;
                    rgb(
                        tone(p[0]) + white + bg,
                        tone(p[1]) + white + bg * 0.5,
                        tone(p[2]) + white + bg * 1.5,
                    )
                };
                c.set(x, y, col);
            }
        }
    }
}
