//! Turns a `Canvas` into one ANSI byte stream per frame.

use crate::gfx::{Canvas, Rgb};
use std::io::Write;

/// Writes the canvas as truecolor upper-half-blocks: foreground = top pixel,
/// background = bottom pixel. Color escapes are only emitted when they change.
/// The frame is wrapped in a synchronized update (DEC 2026) to avoid tearing
/// in terminals that support it; others ignore the sequence.
pub fn compose(out: &mut Vec<u8>, c: &Canvas, clear_screen: bool, hud: Option<(&str, u16)>) {
    out.clear();
    out.extend_from_slice(b"\x1b[?2026h");
    if clear_screen {
        out.extend_from_slice(b"\x1b[0m\x1b[2J");
    }
    for row in 0..c.h / 2 {
        write!(out, "\x1b[{};1H", row + 1).unwrap();
        let mut fg: Option<Rgb> = None;
        let mut bg: Option<Rgb> = None;
        let top_row = &c.px[2 * row * c.w..(2 * row + 1) * c.w];
        let bottom_row = &c.px[(2 * row + 1) * c.w..(2 * row + 2) * c.w];
        for (&top, &bottom) in top_row.iter().zip(bottom_row) {
            if fg != Some(top) {
                write!(out, "\x1b[38;2;{};{};{}m", top[0], top[1], top[2]).unwrap();
                fg = Some(top);
            }
            if bg != Some(bottom) {
                write!(out, "\x1b[48;2;{};{};{}m", bottom[0], bottom[1], bottom[2]).unwrap();
                bg = Some(bottom);
            }
            out.extend_from_slice("▀".as_bytes());
        }
    }
    out.extend_from_slice(b"\x1b[0m");
    if let Some((text, row)) = hud {
        write!(out, "\x1b[{row};1H{text}\x1b[0m").unwrap();
    }
    out.extend_from_slice(b"\x1b[?2026l");
}
