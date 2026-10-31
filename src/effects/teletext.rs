use super::{Effect, digits};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb};

/// The eight broadcast colors, in attribute order.
const PAL: [Rgb; 8] = [
    [0, 0, 0],
    [255, 0, 0],
    [0, 255, 0],
    [255, 255, 0],
    [0, 0, 255],
    [255, 0, 255],
    [0, 255, 255],
    [255, 255, 255],
];

/// A teletext graphics page: each character cell is a 2×3 block mosaic and
/// every row has one foreground color. Star-shaped graphics grow with the
/// bass and change their point count on beats, spectrum bars fill the lower
/// half, rows whose band is loud flip to a colored background, and beats
/// switch between contiguous and separated blocks.
pub struct Teletext {
    spin: f32,
    points: u32,
    beats: u32,
    grid: Vec<u8>,
}

impl Teletext {
    pub fn new() -> Self {
        Self {
            spin: 0.0,
            points: 5,
            beats: 0,
            grid: Vec::new(),
        }
    }
}

impl Effect for Teletext {
    fn name(&self) -> &'static str {
        "Teletext"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.spin += dt * (0.4 + 1.6 * f.level);
        if f.beat {
            self.beats += 1;
            self.points = 3 + (self.points + 1) % 6;
        }
        c.clear();
        // One block is b×b pixels, so a character cell is 2b×3b.
        let b = (c.w / 80).min(c.h / 39).max(1);
        let (cols, rows) = (c.w / (2 * b), c.h / (3 * b));
        if cols == 0 || rows < 3 {
            return;
        }
        let (sw, sh) = (cols * 2, rows * 3);
        let (ox, oy) = ((c.w - sw * b) / 2, (c.h - sh * b) / 2);
        let bins = f.spectrum.len();
        let band = |k: usize, n: usize| -> f32 {
            if bins == 0 || n == 0 {
                return 0.0;
            }
            let (a, z) = (k * bins / n, ((k + 1) * bins / n).clamp(k * bins / n + 1, bins));
            f.spectrum[a.min(bins - 1)..z].iter().sum::<f32>() / (z - a.min(bins - 1)) as f32
        };

        // Blocks: 0 = off, 1 = on, in the row's foreground color.
        self.grid.clear();
        self.grid.resize(sw * sh, 0);
        let body = rows - 2;
        let split = 1 + body * 3 / 5;
        let gcy = (1 + split) as f32 * 1.5;
        let half = (split - 1) as f32 * 1.5;
        let radius = half * (0.8 + 0.25 * f.bass + 0.15 * f.pulse);
        let spikes = 0.18 + 0.17 * f.pulse;
        // As many stars as fit side by side, each spinning its own way.
        let stars = ((sw as f32 / (half * 2.6).max(1.0)) as usize).clamp(1, 3);
        for sy in 3..split * 3 {
            for sx in 0..sw {
                let mut on = false;
                let mut clear = true;
                for k in 0..stars {
                    let cx = sw as f32 * (k as f32 + 0.5) / stars as f32;
                    let (dx, dy) = (sx as f32 + 0.5 - cx, sy as f32 + 0.5 - gcy);
                    let dir = if k % 2 == 0 { 1.0 } else { -1.0 };
                    let a = dy.atan2(dx) + k as f32;
                    let r = radius * (1.0 - spikes + spikes * (a * self.points as f32 + dir * self.spin * 2.0).cos());
                    let d = (dx * dx + dy * dy).sqrt();
                    // A filled star with a hollow ring inside, if big enough.
                    let ring = r > 6.0 && (d - r * 0.45).abs() < 0.9;
                    on |= d < r && !ring;
                    clear &= d > r + 1.5;
                }
                // A wave rolls through behind the stars.
                let wave = gcy + (half - 1.0).max(0.0) * 0.7 * (sx as f32 * 0.12 - t * 2.5).sin() * (0.4 + f.mid);
                let behind = (sy as f32 - wave).abs() < 1.1 && clear;
                if on || behind {
                    self.grid[sy * sw + sx] = 1;
                }
            }
        }
        for sx in 0..sw {
            let v = band(sx * 48 / sw.max(1), 48);
            let height = (v * ((rows - 1 - split) * 3) as f32) as usize;
            for k in 0..height {
                let sy = (rows - 1) * 3 - 1 - k;
                if sy >= split * 3 {
                    self.grid[sy * sw + sx] = 1;
                }
            }
        }

        // Rows flip when their band stands out from the rest.
        let mean = if bins == 0 {
            0.0
        } else {
            f.spectrum.iter().sum::<f32>() / bins as f32
        };
        let flip = (mean + 0.25).max(0.55);
        let separated = b >= 3 && self.beats % 2 == 1;
        let flash_on = (t * 1.5).fract() < 0.6;
        for r in 0..rows {
            // Per row: foreground and background color.
            let (mut fg, mut bg) = (1 + (r + self.beats as usize) % 7, 0);
            if r > 0 && r + 1 < rows {
                let e = band(r - 1, body);
                if e > flip {
                    bg = fg;
                    fg = if fg == 7 || fg == 3 || fg == 6 { 4 } else { 7 };
                }
                if r % 5 == 3 && !flash_on && bg == 0 {
                    fg = 0;
                }
            }
            if r + 1 == rows {
                // The four colored link boxes at the bottom.
                for k in 0..4 {
                    let col = PAL[[1, 2, 3, 6][k]];
                    for y in 0..3 * b {
                        for x in (k * cols / 4 * 2 + 1) * b..(((k + 1) * cols / 4) * 2).saturating_sub(1) * b {
                            c.plot((ox + x) as i32, (oy + r * 3 * b + y) as i32, col);
                        }
                    }
                }
                continue;
            }
            for sy in r * 3..r * 3 + 3 {
                for sx in 0..sw {
                    let on = self.grid[sy * sw + sx] == 1;
                    let col = PAL[if on { fg } else { bg }];
                    let gap = separated && on;
                    for y in 0..b {
                        for x in 0..b {
                            let col = if gap && (x == b - 1 || y == b - 1) {
                                PAL[bg]
                            } else {
                                col
                            };
                            c.set(ox + sx * b + x, oy + sy * b + y, col);
                        }
                    }
                }
            }
        }

        // Header: page number and a running clock, or a color strip if tiny.
        if b >= 2 {
            let ds = (b as i32 / 3).max(1);
            let y = (oy + (3 * b - 5 * ds as usize) / 2) as i32;
            let x0 = (ox + 2 * b) as i32;
            digits::draw(c, x0, y, 100 + self.beats % 800 / 8, ds, PAL[7]);
            let secs = (t.max(0.0) as u32) % 86_400;
            let clock = [secs / 3600, secs / 60 % 60, secs % 60];
            let cw = 4 * ds * 2 + 3 * ds;
            let mut x = (ox + sw * b) as i32 - 3 * cw - 2 * b as i32;
            for v in clock {
                digits::draw(c, x, y, v / 10, ds, PAL[3]);
                digits::draw(c, x + 4 * ds, y, v % 10, ds, PAL[3]);
                x += cw;
            }
        } else {
            for sx in 0..sw {
                let col = PAL[1 + (sx / 3 + self.beats as usize) % 7];
                for sy in 0..2 {
                    c.set(ox + sx, oy + sy, col);
                }
            }
        }
    }
}
