use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, mix, rgb, scale};

const TEXT: &str = "SOUNDSCENE * GREETINGS TO ALL DEMOSCENERS * THE BASS DRIVES THE WAVE * \
    RESPECT TO EVERY CODER, PIXELER AND MUSICIAN OUT THERE * KEEP THE SCENE ALIVE! * ";

/// Glyph advance in font pixels (5 wide plus one column of spacing).
const ADVANCE: i32 = 6;

/// 5×7 bitmap font, one byte per row, bit 4 is the leftmost column.
fn glyph(ch: char) -> [u8; 7] {
    match ch {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '*' => [0x00, 0x04, 0x15, 0x0E, 0x15, 0x04, 0x00],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '?' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '\'' => [0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
        _ => [0; 7],
    }
}

/// Classic sine scroller: big rainbow letters scroll along a sine wave over a
/// parallax starfield and are mirrored in rippling water. Bass raises the
/// wave, beats kick it, loudness speeds up the scroll.
pub struct SineScroller {
    glyphs: Vec<[u8; 7]>,
    scroll: f32,
    phase: f32,
    amp: f32,
}

impl SineScroller {
    pub fn new() -> Self {
        Self {
            glyphs: TEXT.chars().map(glyph).collect(),
            scroll: 0.0,
            phase: 0.0,
            amp: 0.0,
        }
    }
}

impl Effect for SineScroller {
    fn name(&self) -> &'static str {
        "Sine Scroller"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        let (w, h) = (c.w, c.h);
        if w == 0 || h == 0 {
            return;
        }
        let hf = h as f32;
        // Size of one font pixel in canvas pixels.
        let s = ((hf * 0.3 / 7.0) as i32).max(1);
        self.scroll += dt * s as f32 * (16.0 + 10.0 * f.level);
        self.phase += dt * (2.0 + 2.5 * f.mid);
        let target = 0.06 + 0.16 * f.bass + 0.08 * f.pulse;
        self.amp += (target - self.amp) * (1.0 - (-6.0 * dt).exp());

        let horizon = (hf * 0.74) as usize;
        let base = hf * 0.4 - 3.5 * s as f32;
        let amp = self.amp * hf;
        let scroll = self.scroll as i64;
        let period = self.glyphs.len().max(1) as i64 * (ADVANCE * s) as i64;
        let hue0 = t * 0.08;
        let flash = f.pulse * 0.35;

        // Returns the letter color at canvas pixel (x, y), if any.
        let text_at = |x: i32, y: i32| -> Option<[u8; 3]> {
            let wave = (x as f32 * 0.028 + self.phase).sin() + 0.2 * (x as f32 * 0.07 - self.phase * 1.7).sin();
            let ty = (y as f32 - base - wave * amp).floor() as i32;
            if ty < 0 || ty >= 7 * s {
                return None;
            }
            let u = (x as i64 + scroll).rem_euclid(period.max(1)) as i32 / s;
            let (ci, col) = ((u / ADVANCE) as usize, u % ADVANCE);
            let row = (ty / s) as usize;
            let bits = self.glyphs.get(ci)?[row.min(6)];
            if col >= 5 || bits & (0x10 >> col) == 0 {
                return None;
            }
            // Copper-style gradient down each letter, hue drifting along the text.
            let k = ty as f32 / (7 * s) as f32;
            let hue = hue0 + x as f32 / w as f32 * 0.5 + k * 0.15;
            let v = 1.0 - 0.45 * k;
            Some(mix(hsv(hue, 0.85 - 0.5 * flash, v), [255, 255, 255], flash * (1.0 - k)))
        };

        let star_shift = (self.scroll * 0.25) as i64;
        for y in 0..h {
            let yf = y as f32 / hf;
            for x in 0..w {
                let col = if y < horizon {
                    let shadow = text_at(x as i32 - s, y as i32 - s).is_some();
                    match text_at(x as i32, y as i32) {
                        Some(col) => col,
                        None => {
                            let sky = rgb(0.05 + 0.12 * yf, 0.0, 0.12 + 0.18 * yf);
                            let sx = x as i64 + star_shift;
                            let hash = (sx.wrapping_mul(73_856_093) ^ (y as i64).wrapping_mul(19_349_663)) & 0x3FF;
                            let sky = if hash < 3 { [200, 200, 230] } else { sky };
                            if shadow { scale(sky, 0.3) } else { sky }
                        }
                    }
                } else {
                    // Mirror around the horizon with a horizontal ripple.
                    let d = (y - horizon) as f32;
                    let ripple = (d * 0.9 - t * 5.0).sin() * (1.0 + d * 0.08);
                    let my = horizon as i32 - 1 - d as i32;
                    let mx = x as i32 + ripple.round() as i32;
                    let water = rgb(0.0, 0.03 + 0.05 * yf, 0.12 + 0.08 * yf);
                    match text_at(mx, my) {
                        Some(col) => mix(scale(col, 0.55), water, 0.35 + d / hf),
                        None => water,
                    }
                };
                c.set(x, y, col);
            }
        }
    }
}
