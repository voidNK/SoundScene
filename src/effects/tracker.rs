use super::{Effect, digits};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, mix, scale};
use std::collections::VecDeque;

const CHANNELS: usize = 8;
/// Rows kept below and above the play position.
const AHEAD: usize = 40;
const BEHIND: usize = 40;

#[derive(Clone, Copy, Default)]
struct Cell {
    /// 0 = empty, otherwise 1 + semitone (0..12).
    note: u8,
    octave: u8,
    instrument: u8,
    volume: u8,
}

/// A music tracker's pattern view: channel columns of note cells scroll up
/// past the highlighted play row, with per-channel volume bars on top. Rows
/// entering at the bottom are written from the spectrum (one band per
/// channel, louder bands play more notes), beats put a kick in the first
/// channel, and the tempo follows the level.
pub struct Tracker {
    rows: VecDeque<[Cell; CHANNELS]>,
    /// Fraction of the current row played; `rows[BEHIND]` is the play row.
    tick: f32,
    row_no: u32,
    env: [f32; CHANNELS],
    rng: Rng,
    kick: bool,
}

impl Tracker {
    pub fn new() -> Self {
        let mut tr = Self {
            rows: VecDeque::new(),
            tick: 0.0,
            row_no: 0,
            env: [0.0; CHANNELS],
            rng: Rng::new(0x7AC4),
            kick: false,
        };
        for _ in 0..BEHIND + AHEAD + 1 {
            tr.write_row(&[], 0.3);
        }
        tr
    }

    fn write_row(&mut self, spectrum: &[f32], level: f32) {
        let n = self.rows.len() as u32;
        let mut row = [Cell::default(); CHANNELS];
        for (ch, cell) in row.iter_mut().enumerate() {
            let band = if spectrum.is_empty() {
                level
            } else {
                let a = ch * spectrum.len() / CHANNELS;
                spectrum[a.min(spectrum.len() - 1)]
            };
            let on_grid = (n + self.row_no).is_multiple_of([4, 2, 1, 2, 4, 1, 3, 2][ch]);
            let chance = if on_grid { 0.15 + 0.8 * band } else { 0.15 * band };
            if self.rng.f32() < chance {
                cell.note = 1 + (self.rng.next_u32() % 12) as u8;
                cell.octave = (1 + ch / 2 + (self.rng.next_u32() % 2) as usize) as u8;
                cell.instrument = 1 + ch as u8;
                cell.volume = (20.0 + 44.0 * band).min(64.0) as u8;
            }
        }
        if self.kick {
            row[0] = Cell {
                note: 1,
                octave: 1,
                instrument: 1,
                volume: 64,
            };
            self.kick = false;
        }
        self.rows.push_back(row);
    }
}

/// Fills a rectangle, clipped to the canvas.
fn rect(c: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, col: Rgb) {
    let (xa, xb) = (x0.max(0) as usize, (x1.max(0) as usize).min(c.w));
    let (ya, yb) = (y0.max(0) as usize, (y1.max(0) as usize).min(c.h));
    for y in ya..yb {
        if xa < xb {
            c.px[y * c.w + xa..y * c.w + xb].fill(col);
        }
    }
}

fn note_color(note: u8) -> Rgb {
    hsv((note as f32 - 1.0) / 12.0, 0.65, 1.0)
}

impl Effect for Tracker {
    fn name(&self) -> &'static str {
        "Tracker"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.kick = true;
        }
        self.tick += dt * (5.0 + 9.0 * f.level);
        let mut steps = 0;
        while self.tick >= 1.0 {
            self.tick -= 1.0;
            if steps < 8 {
                self.rows.pop_front();
                self.write_row(&f.spectrum, f.level);
                self.row_no = (self.row_no + 1) % 64;
                steps += 1;
                // Notes on the new play row strike their channel.
                for (ch, cell) in self.rows[BEHIND].iter().enumerate() {
                    if cell.note != 0 {
                        self.env[ch] = cell.volume as f32 / 64.0;
                    }
                }
            }
        }
        let bins = f.spectrum.len();
        for (ch, e) in self.env.iter_mut().enumerate() {
            let band = if bins == 0 {
                0.0
            } else {
                f.spectrum[(ch * bins / CHANNELS + bins / CHANNELS / 2).min(bins - 1)]
            };
            *e = (*e - dt * 2.5).max(band * 0.8);
        }
        c.fill([10, 10, 16]);
        if c.w < 4 || c.h < 4 {
            return;
        }

        let (w, h) = (c.w as i32, c.h as i32);
        let detailed = w >= 110 && h >= 50;
        let s = if w >= 420 && h >= 180 { 2 } else { 1 };
        let lh = if detailed { 7 * s } else { (h / 16).clamp(2, 4) };
        let num_w = if detailed { 11 * s } else { 0 };
        let cell_min = if detailed { 33 * s } else { 6 };
        let n = ((w - num_w) / cell_min).clamp(1, CHANNELS as i32) as usize;
        let cw = (w - num_w) / n as i32;
        let ox = (w - num_w - cw * n as i32) / 2 + num_w;

        // Volume bars across the top.
        let bar_h = (h as f32 * 0.16).max(3.0) as i32;
        for ch in 0..n {
            let x0 = ox + ch as i32 * cw + 1;
            let x1 = x0 + cw - 2;
            let segs = ((bar_h - 1) / 2).max(1);
            let lit = (self.env[ch] * segs as f32).round() as i32;
            for k in 0..segs {
                let y1 = bar_h - 1 - k * (bar_h - 1) / segs;
                let y0 = bar_h - 1 - (k + 1) * (bar_h - 1) / segs + 1;
                let p = k as f32 / segs as f32;
                let base = if p < 0.6 {
                    [60, 220, 90]
                } else if p < 0.85 {
                    [240, 200, 40]
                } else {
                    [250, 60, 40]
                };
                let col = if k < lit { base } else { scale(base, 0.12) };
                rect(c, x0, y0.min(y1), x1, y1, col);
            }
        }

        // Pattern area with the play row in its middle.
        let top = bar_h + 1;
        let area = h - top;
        let mid = top + (area - lh) / 2;
        let half = area / lh / 2 + 1;
        for k in -half..=half {
            let idx = BEHIND as i32 + k;
            if idx < 0 || idx as usize >= self.rows.len() {
                continue;
            }
            let y = mid + k * lh;
            if y + lh <= top || y >= h {
                continue;
            }
            let play = k == 0;
            let row_no = (self.row_no as i32 + k).rem_euclid(64) as u32;
            let beat_row = row_no.is_multiple_of(4);
            let band = if play {
                [48, 48, 90]
            } else if beat_row {
                [20, 20, 30]
            } else {
                [12, 12, 18]
            };
            rect(c, 0, y.max(top), w, (y + lh).min(h), band);
            if play {
                rect(c, 0, y.max(top), w, (y + 1).max(top), [90, 90, 160]);
            }
            let fade = if play {
                1.0
            } else {
                (1.0 - k.abs() as f32 / (half as f32 + 1.0)).max(0.25)
            };
            if detailed {
                let col = scale(if beat_row { [200, 200, 120] } else { [130, 130, 150] }, fade);
                digits::draw(c, s, y + s, row_no / 10, s, col);
                digits::draw(c, 5 * s, y + s, row_no % 10, s, col);
            }
            for (ch, cell) in self.rows[idx as usize].iter().take(n).enumerate() {
                let x = ox + ch as i32 * cw;
                rect(c, x, y.max(top), x + 1, (y + lh).min(h), [30, 30, 45]);
                if cell.note == 0 {
                    // Empty cell: a dim dash.
                    let dy = y + lh / 2;
                    if dy >= top {
                        rect(c, x + 3, dy, x + 3 + (cw / 4).max(1), dy + 1, scale([60, 60, 80], fade));
                    }
                    continue;
                }
                let nc = scale(note_color(cell.note), fade);
                if detailed {
                    // Note block (pitch as a notch), octave, instrument, volume.
                    let bx = x + 3 * s;
                    let by = y + s;
                    if by >= top {
                        rect(c, bx, by, bx + 8 * s, by + 5 * s, nc);
                        let notch = bx + (cell.note as i32 - 1) * 7 * s / 12;
                        rect(c, notch, by, notch + s, by + 2 * s, scale(nc, 0.3));
                        let dim = scale([200, 200, 210], fade);
                        digits::draw(c, bx + 10 * s, by, cell.octave as u32, s, dim);
                        let ic = scale([120, 200, 255], fade);
                        digits::draw(c, bx + 15 * s, by, cell.instrument as u32 / 10, s, ic);
                        digits::draw(c, bx + 19 * s, by, cell.instrument as u32 % 10, s, ic);
                        let vw = (cell.volume as i32 * 6 * s / 64).max(1);
                        rect(
                            c,
                            bx + 24 * s,
                            by + 3 * s,
                            bx + 24 * s + vw,
                            by + 5 * s,
                            scale([120, 255, 140], fade),
                        );
                    }
                } else {
                    let y0 = y.max(top);
                    let width = ((cw - 2) * cell.volume as i32 / 64).max(1);
                    rect(c, x + 1, y0, x + 1 + width, (y + lh - 1).max(y0 + 1).min(h), nc);
                }
                if play {
                    let glow = mix(nc, [255, 255, 255], 0.5);
                    rect(c, x + 1, y.max(top), x + cw - 1, (y + 1).max(top), glow);
                }
            }
        }
    }
}
