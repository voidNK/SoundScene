//! 3×5 pixel digits for scores and readouts.

use crate::gfx::{Canvas, Rgb};

/// One row per entry, bit 2 is the leftmost pixel.
const GLYPHS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 1, 1],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];

/// Pixel width of `n` drawn at `scale`.
pub fn width(n: u32, scale: i32) -> i32 {
    let digits = n.checked_ilog10().unwrap_or(0) as i32 + 1;
    digits * 4 * scale - scale
}

/// Draws `n` in decimal with its top-left corner at (x, y), clipped to the canvas.
pub fn draw(c: &mut Canvas, x: i32, y: i32, n: u32, scale: i32, col: Rgb) {
    for (k, ch) in n.to_string().bytes().enumerate() {
        let glyph = &GLYPHS[(ch - b'0') as usize];
        let gx = x + k as i32 * 4 * scale;
        for (row, bits) in glyph.iter().enumerate() {
            for bit in 0..3 {
                if bits & (4 >> bit) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        c.plot(gx + bit * scale + dx, y + row as i32 * scale + dy, col);
                    }
                }
            }
        }
    }
}
