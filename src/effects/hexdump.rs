use super::{Effect, digits};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, mix, scale};
use std::collections::VecDeque;

const MAX_BYTES: usize = 16;
const MAX_ROWS: usize = 256;
const GREEN: Rgb = [70, 255, 110];
const AMBER: Rgb = [255, 180, 50];

struct Row {
    addr: u32,
    bytes: [u8; MAX_BYTES],
    /// Brightness boost for rows written on a beat.
    hot: f32,
}

/// A memory dump scrolling up a phosphor terminal: address, value columns
/// and a column of glyph blocks. New rows are written from the spectrum, the
/// scroll speed follows the level, rows light up with their spectrum band,
/// beats write hot rows, and every 16 beats the screen swaps green for amber.
pub struct HexDump {
    rows: VecDeque<Row>,
    scroll: f32,
    addr: u32,
    rng: Rng,
    beats: u32,
}

impl HexDump {
    pub fn new() -> Self {
        let mut d = Self {
            rows: VecDeque::new(),
            scroll: 0.0,
            addr: 0x4000,
            rng: Rng::new(0x4E7D),
            beats: 0,
        };
        for _ in 0..64 {
            d.push(&[], false);
        }
        d
    }

    fn push(&mut self, spectrum: &[f32], hot: bool) {
        let mut bytes = [0; MAX_BYTES];
        for (i, b) in bytes.iter_mut().enumerate() {
            let band = if spectrum.is_empty() {
                0.0
            } else {
                spectrum[(i * spectrum.len() / MAX_BYTES).min(spectrum.len() - 1)]
            };
            let v = band * 75.0 + self.rng.f32() * 30.0 * (1.0 - band * 0.5);
            *b = v.clamp(0.0, 99.0) as u8;
        }
        self.rows.push_front(Row {
            addr: self.addr,
            bytes,
            hot: if hot { 1.0 } else { 0.0 },
        });
        self.rows.truncate(MAX_ROWS);
        self.addr = (self.addr + MAX_BYTES as u32) % 10_000;
    }
}

/// `n` zero-padded to `len` digits.
fn number(c: &mut Canvas, x: i32, y: i32, n: u32, len: u32, s: i32, col: Rgb) {
    for i in 0..len {
        let d = n / 10u32.pow(len - 1 - i) % 10;
        digits::draw(c, x + i as i32 * 4 * s, y, d, s, col);
    }
}

/// A 3×5 block glyph picked by the value, standing in for a character.
fn glyph(c: &mut Canvas, x: i32, y: i32, v: u8, s: i32, col: Rgb) {
    let bits = (v as u32).wrapping_mul(0x2C1B_3C6D).rotate_left(11) ^ 0x5A5A;
    for k in 0..15 {
        if bits & (1 << k) != 0 {
            for dy in 0..s {
                for dx in 0..s {
                    c.plot(x + (k % 3) * s + dx, y + (k / 3) * s + dy, col);
                }
            }
        }
    }
}

impl Effect for HexDump {
    fn name(&self) -> &'static str {
        "Hex Dump"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.beats += 1;
        }
        let speed = 1.5 + 10.0 * f.level + 5.0 * f.pulse;
        self.scroll += dt * speed;
        let mut pushed = 0;
        while self.scroll >= 1.0 {
            self.scroll -= 1.0;
            if pushed < 8 {
                self.push(&f.spectrum, f.pulse > 0.6);
                pushed += 1;
            }
        }
        for r in &mut self.rows {
            r.hot = (r.hot - dt * 1.5).max(0.0);
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        let ink = if (self.beats / 16).is_multiple_of(2) {
            GREEN
        } else {
            AMBER
        };
        let s = if c.w >= 360 && c.h >= 160 { 2 } else { 1 };
        let lh = 7 * s;
        let (w, h) = (c.w as i32, c.h as i32);
        let show_addr = w >= 90 * s;
        let addr_w = if show_addr { 20 * s } else { 0 };
        let avail = w - 2 * s - addr_w - 4 * s;
        let mut n = (avail / (15 * s)).clamp(1, MAX_BYTES as i32);
        if n >= 8 {
            n -= n % 4;
        }
        let group = |i: i32| i * 11 * s + (i / 4) * 2 * s;
        let bytes_w = group(n) - 4 * s;
        let total = addr_w + bytes_w + 4 * s + n * 4 * s - s;
        let ox = (w - total) / 2;

        // Phosphor background with faint scanlines.
        let bg = scale(ink, 0.05);
        for y in 0..c.h {
            let col = if y % 2 == 0 { bg } else { scale(bg, 0.6) };
            c.px[y * c.w..(y + 1) * c.w].fill(col);
        }

        let bins = f.spectrum.len();
        let visible = (h / lh + 2) as usize;
        // The newest row sits one line above the cursor line at the bottom.
        let newest = h - 2 * lh + s - (self.scroll * lh as f32) as i32;
        for (k, row) in self.rows.iter().take(visible).enumerate() {
            let y = newest - k as i32 * lh;
            if y + lh <= 0 {
                break;
            }
            // Bottom rows follow the bass, top rows the treble.
            let band = if bins == 0 {
                0.0
            } else {
                let b = (y.max(0) as usize * bins / c.h).min(bins - 1);
                f.spectrum[bins - 1 - b]
            };
            let hl = (band * band + row.hot * 0.6).min(1.0);
            if hl > 0.15 {
                let col = scale(ink, 0.22 * hl);
                let (a, b) = (ox.max(0), (ox + total).clamp(ox.max(0), w));
                for yy in (y - s).max(0)..(y + 6 * s).min(h) {
                    let off = yy as usize * c.w;
                    c.px[off + a as usize..off + b as usize].fill(col);
                }
            }
            let dim = 0.7 + 0.3 * hl + 0.3 * row.hot;
            if show_addr {
                number(c, ox, y, row.addr, 4, s, scale(ink, 0.5 * dim));
            }
            let bx = ox + addr_w;
            let gx = bx + bytes_w + 4 * s;
            for i in 0..n {
                let v = row.bytes[i as usize];
                let col = mix(scale(ink, dim * 0.75), [255, 255, 255], (v as f32 / 99.0) * hl * 0.7);
                let col = scale(col, 0.65 + 0.35 * v as f32 / 99.0);
                let x = bx + group(i);
                number(c, x, y, v as u32, 2, s, col);
                glyph(c, gx + i * 4 * s, y, v, s, scale(ink, dim * 0.7));
            }
        }

        // Blinking block cursor under the newest row.
        if (t * 2.0).fract() < 0.55 {
            let y = newest + lh;
            for yy in y..y + 5 * s {
                for xx in ox..ox + 3 * s {
                    c.plot(xx, yy, ink);
                }
            }
        }
    }
}
