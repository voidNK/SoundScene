use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, add, mix, scale};

/// Segment bits abcdefg (bit 0 = a) for 0..9.
const FONT: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];
const BANDS: usize = 8;
const COUNTER: usize = 4;
const SLOTS: usize = BANDS * 2 + COUNTER;
const RED: Rgb = [255, 40, 30];
const GREEN: Rgb = [40, 255, 90];
/// Italic lean, as a fraction of the digit height.
const SLANT: f32 = 0.1;

/// Seven-segment LED displays on a smoked panel: a red row of two-digit
/// spectrum band readouts and a big green beat counter whose decimal points
/// blink with the pulse. Segments glow, unlit ones stay faintly visible, and
/// switched-off segments fade out with a short afterglow.
pub struct SevenSeg {
    /// Per digit slot: brightness of segments a..g and the decimal point.
    seg: [[f32; 8]; SLOTS],
    values: [u32; SLOTS],
    timer: f32,
    beats: u32,
}

impl SevenSeg {
    pub fn new() -> Self {
        Self {
            seg: [[0.0; 8]; SLOTS],
            values: [0; SLOTS],
            timer: 0.0,
            beats: 0,
        }
    }
}

/// Draws one digit with its top-left corner at (x0, y0).
fn digit(c: &mut Canvas, x0: f32, y0: f32, dw: f32, dh: f32, seg: &[f32; 8], col: Rgb) {
    let t = (dw * 0.2).max(1.0);
    let gap = t * 0.22;
    let (yu, yl) = ((t * 0.5 + dh * 0.5) * 0.5, (dh * 0.5 + dh - t * 0.5) * 0.5);
    let (lh, lv) = (dw - t - gap, dh * 0.5 - t * 0.5 - gap);
    // Center, half length along the axis, horizontal?
    let segs = [
        (dw * 0.5, t * 0.5, lh, true),
        (dw - t * 0.5, yu, lv, false),
        (dw - t * 0.5, yl, lv, false),
        (dw * 0.5, dh - t * 0.5, lh, true),
        (t * 0.5, yl, lv, false),
        (t * 0.5, yu, lv, false),
        (dw * 0.5, dh * 0.5, lh, true),
    ];
    let dp = (dw + t * 0.9, dh - t * 0.5);
    let ghost = scale(col, 0.07);
    let soft = (t * 0.7).max(0.7);
    let logs = seg.map(|s| if s > 1e-4 { s.ln() } else { -1e9 });
    let m = t * 2.0;
    let (xa, xb) = (
        (x0 - m).max(0.0) as i32,
        ((x0 + dw + t * 2.0 + m + SLANT * dh).max(0.0) as i32).min(c.w as i32),
    );
    let (ya, yb) = (
        (y0 - m).max(0.0) as i32,
        ((y0 + dh + m).max(0.0) as i32).min(c.h as i32),
    );
    for py in ya..yb {
        let ly = py as f32 + 0.5 - y0;
        let shift = SLANT * (dh - ly);
        for px in xa..xb {
            let lx = px as f32 + 0.5 - x0 - shift;
            // The glow is max(seg · e^(-d/soft)); compared in log space so
            // only one exp is needed per pixel.
            let (mut core, mut glow, mut off) = (0.0f32, f32::MIN, 0.0f32);
            for (i, &(cx, cy, len, horiz)) in segs.iter().enumerate() {
                let (a, b) = if horiz { (lx - cx, ly - cy) } else { (ly - cy, lx - cx) };
                // Hexagonal segment: pointed ends.
                let d = b.abs() + (a.abs() - (len * 0.5 - t * 0.5)).max(0.0) - t * 0.5;
                let inside = (0.5 - d).clamp(0.0, 1.0);
                core = core.max(inside * seg[i]);
                off = off.max(inside * (1.0 - seg[i]));
                glow = glow.max(logs[i] - d.max(0.0) / soft);
            }
            let d = (lx - dp.0).hypot(ly - dp.1) - t * 0.55;
            let inside = (0.5 - d).clamp(0.0, 1.0);
            core = core.max(inside * seg[7]);
            off = off.max(inside * (1.0 - seg[7]));
            glow = glow.max(logs[7] - d.max(0.0) / soft);
            let glow = if glow > -30.0 { glow.exp() } else { 0.0 };
            if core + glow + off < 0.01 {
                continue;
            }
            let lit = scale(mix(col, [255, 255, 255], 0.35 * core), core);
            let px_col = add(add(lit, scale(col, glow * 0.3)), scale(ghost, off));
            let i = py as usize * c.w + px as usize;
            c.px[i] = add(c.px[i], px_col);
        }
    }
}

impl Effect for SevenSeg {
    fn name(&self) -> &'static str {
        "Seven Segment"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.beats = (self.beats + 1) % 10_000;
        }
        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 0.07;
            let bins = f.spectrum.len();
            for b in 0..BANDS {
                let v = if bins == 0 {
                    0.0
                } else {
                    let (a, z) = (b * bins / BANDS, ((b + 1) * bins / BANDS).max(b * bins / BANDS + 1));
                    f.spectrum[a..z.min(bins)].iter().sum::<f32>() / (z.min(bins) - a).max(1) as f32
                };
                let v = (v * 99.0) as u32;
                self.values[2 * b] = v / 10;
                self.values[2 * b + 1] = v % 10;
            }
            for k in 0..COUNTER {
                self.values[BANDS * 2 + k] = self.beats / 10u32.pow((COUNTER - 1 - k) as u32) % 10;
            }
        }
        let decay = (-dt * 18.0).exp();
        for (slot, seg) in self.seg.iter_mut().enumerate() {
            let bits = FONT[self.values[slot] as usize % 10];
            for (i, s) in seg.iter_mut().enumerate() {
                let on = if i == 7 {
                    slot >= BANDS * 2 && slot % 2 == 1 && f.pulse > 0.5
                } else {
                    bits & (1 << i) != 0
                };
                *s = if on { 1.0 } else { *s * decay };
            }
        }

        // Smoked panel with a soft reflection band.
        for y in 0..c.h {
            let v = y as f32 / c.h.max(1) as f32;
            let refl = (1.0 - ((v - 0.3) * 6.0).abs()).max(0.0) * 6.0;
            let col = [(12.0 + refl) as u8, (8.0 + refl) as u8, (10.0 + refl) as u8];
            c.px[y * c.w..(y + 1) * c.w].fill(col);
        }
        if c.w < 6 || c.h < 6 {
            return;
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let top_h = h * 0.58;
        let bands = if w >= 110.0 { BANDS } else { 4 };
        // Pick one or two rows of band readouts, whichever gives bigger digits.
        let mut best = (0.0, 1);
        for rows in [1, 2] {
            let g = bands.div_ceil(rows) as f32;
            let by_w = w * 0.92 / (g * 2.45 + (g - 1.0) * 0.9 + 0.3);
            let by_h = top_h * 0.82 / (rows as f32 * 1.75 + (rows - 1) as f32 * 0.5);
            let dw = by_w.min(by_h / 1.0);
            if dw > best.0 {
                best = (dw, rows);
            }
        }
        let (dw, rows) = best;
        let dh = dw * 1.75;
        let per_row = bands.div_ceil(rows);
        let group_w = 2.45 * dw;
        let row_w = per_row as f32 * group_w + (per_row - 1) as f32 * 0.9 * dw;
        let block_h = rows as f32 * dh + (rows - 1) as f32 * 0.5 * dw;
        let ty = (top_h - block_h) * 0.5;
        let skip = BANDS / bands;
        for k in 0..bands {
            let (row, colm) = (k / per_row, k % per_row);
            let gx = (w - row_w) * 0.5 + colm as f32 * (group_w + 0.9 * dw);
            let gy = ty + row as f32 * (dh + 0.5 * dw);
            let slot = 2 * (k * skip);
            for j in 0..2 {
                digit(c, gx + j as f32 * 1.25 * dw, gy, dw, dh, &self.seg[slot + j], RED);
            }
        }

        let bottom_h = h - top_h;
        let dw2 = (w * 0.8 / (COUNTER as f32 * 1.3)).min(bottom_h * 0.8 / 1.75);
        let dh2 = dw2 * 1.75;
        let bx = (w - (COUNTER as f32 * 1.3 - 0.3) * dw2) * 0.5;
        let by = top_h + (bottom_h - dh2) * 0.5;
        for k in 0..COUNTER {
            digit(
                c,
                bx + k as f32 * 1.3 * dw2,
                by,
                dw2,
                dh2,
                &self.seg[BANDS * 2 + k],
                GREEN,
            );
        }
    }
}
