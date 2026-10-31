use super::Effect;
use super::mirrorbars::sample;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix, scale};

const MAX_OCTAVES: usize = 6;
const MAX_KEYS: usize = MAX_OCTAVES * 12;
/// Which of the 12 keys of an octave are black.
const BLACK: [bool; 12] = [
    false, true, false, true, false, false, true, false, true, false, true, false,
];
/// White key index (0..7) for each of the 12 keys; black keys sit on the
/// right edge of that white key.
const WHITE_OF: [usize; 12] = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];

struct Note {
    key: usize,
    /// Distances above the keyboard, in units of the roll height.
    bottom: f32,
    top: f32,
    level: f32,
    active: bool,
}

/// A piano keyboard at the bottom whose keys light up with the spectrum
/// (low notes left, high notes right), while the played notes scroll
/// upwards as colored bars. Bass speeds up the scrolling, beats flash the
/// roll.
pub struct PianoRoll {
    notes: Vec<Note>,
    on: [Option<usize>; MAX_KEYS],
    energy: [f32; MAX_KEYS],
    octaves: usize,
}

impl PianoRoll {
    pub fn new() -> Self {
        Self {
            notes: Vec::new(),
            on: [None; MAX_KEYS],
            energy: [0.0; MAX_KEYS],
            octaves: 0,
        }
    }
}

fn key_color(key: usize) -> Rgb {
    hsv((key % 12) as f32 / 12.0, 0.75, 1.0)
}

impl Effect for PianoRoll {
    fn name(&self) -> &'static str {
        "Piano Roll"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        let (w, h) = (c.w, c.h);
        let octaves = (w / 24).clamp(1, MAX_OCTAVES);
        if octaves != self.octaves {
            self.octaves = octaves;
            self.notes.clear();
            self.on = [None; MAX_KEYS];
        }
        let keys = octaves * 12;

        // Key energies and note on/off with hysteresis.
        let mean = f.spectrum.iter().sum::<f32>() / f.spectrum.len().max(1) as f32;
        let th_on = (mean + 0.15).max(0.35);
        for k in 0..keys {
            let p = (k as f32 + 0.5) / keys as f32 * 0.95;
            let e = sample(&f.spectrum, p);
            self.energy[k] = e;
            match self.on[k] {
                None if e > th_on => {
                    self.on[k] = Some(self.notes.len());
                    self.notes.push(Note {
                        key: k,
                        bottom: 0.0,
                        top: 0.0,
                        level: e,
                        active: true,
                    });
                }
                Some(i) if e < th_on - 0.12 => {
                    if let Some(n) = self.notes.get_mut(i) {
                        n.active = false;
                    }
                    self.on[k] = None;
                }
                Some(i) => {
                    if let Some(n) = self.notes.get_mut(i) {
                        n.level = n.level.max(e);
                    }
                }
                None => {}
            }
        }
        let speed = 0.25 + 0.35 * f.bass;
        for n in &mut self.notes {
            n.top += speed * dt;
            if !n.active {
                n.bottom += speed * dt;
            }
        }
        // Drop notes that left the top, and fix up the indices of held keys.
        if self.notes.iter().any(|n| n.bottom > 1.05) {
            self.notes.retain(|n| n.bottom <= 1.05);
            self.on = [None; MAX_KEYS];
            for (i, n) in self.notes.iter().enumerate() {
                if n.active && n.key < MAX_KEYS {
                    self.on[n.key] = Some(i);
                }
            }
        }

        let bg = hsv(0.68, 0.5, 0.05 + 0.06 * f.pulse);
        c.fill(bg);
        if w == 0 || h == 0 {
            return;
        }
        let (wf, hf) = (w as f32, h as f32);
        let kb_h = (hf * 0.2).max(2.0).min(hf);
        let kb_top = hf - kb_h;
        let whites = octaves * 7;
        let ww = wf / whites as f32;
        let key_span = |k: usize| -> (f32, f32) {
            let o = (k / 12) as f32 * 7.0;
            let wi = WHITE_OF[k % 12] as f32;
            if BLACK[k % 12] {
                let x = (o + wi + 1.0) * ww;
                (x - ww * 0.32, x + ww * 0.32)
            } else {
                ((o + wi) * ww, (o + wi + 1.0) * ww)
            }
        };

        // Faint lanes at every C, drifting brightness for idle motion.
        for o in 0..octaves {
            let x = ((o * 7) as f32 * ww) as usize;
            if x < w {
                let v = 0.12 + 0.04 * (t * 0.8 + o as f32).sin();
                for y in 0..(kb_top as usize).min(h) {
                    c.set(x, y, hsv(0.68, 0.4, v));
                }
            }
        }

        // Notes: white-key notes first, black-key notes on top.
        for pass_black in [false, true] {
            for n in self.notes.iter().filter(|n| BLACK[n.key % 12] == pass_black) {
                let (x0, x1) = key_span(n.key);
                let xa = ((x0 + 0.5).max(0.0) as usize).min(w);
                let xb = ((x1 - 0.5).max(0.0) as usize).clamp(xa + 1, w.max(xa + 1)).min(w);
                let ya = (kb_top - n.top * kb_top).max(0.0) as usize;
                let yb = ((kb_top - n.bottom * kb_top).max(0.0) as usize).min(h);
                let base = scale(key_color(n.key), 0.4 + 0.6 * n.level);
                for y in ya..yb {
                    // Bright leading edge, slightly darker body.
                    let col = if y == ya {
                        mix(base, [255, 255, 255], 0.5)
                    } else {
                        scale(base, 0.8)
                    };
                    for x in xa..xb {
                        c.set(x, y, col);
                    }
                }
            }
        }

        // Keyboard.
        let y0 = (kb_top as usize).min(h);
        let black_h = kb_top + kb_h * 0.6;
        for k in (0..keys).filter(|k| !BLACK[k % 12]) {
            let (x0, x1) = key_span(k);
            let col = if self.on[k].is_some() {
                mix(key_color(k), [255, 255, 255], 0.25)
            } else {
                mix([170, 170, 180], key_color(k), self.energy[k] * 0.3)
            };
            for x in (x0 as usize).min(w)..(x1 as usize).min(w) {
                let edge = x == x0 as usize;
                for y in y0..h {
                    let shade = 1.0 - (y as f32 - kb_top) / kb_h * 0.25;
                    c.set(x, y, if edge { [40, 40, 50] } else { scale(col, shade) });
                }
            }
        }
        for k in (0..keys).filter(|k| BLACK[k % 12]) {
            let (x0, x1) = key_span(k);
            let col = if self.on[k].is_some() {
                key_color(k)
            } else {
                mix([20, 20, 26], key_color(k), self.energy[k] * 0.4)
            };
            for x in (x0.max(0.0) as usize).min(w)..(x1.ceil() as usize).min(w) {
                for y in y0..(black_h as usize).min(h) {
                    c.set(x, y, col);
                }
            }
        }
    }
}
