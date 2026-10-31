use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix};

/// Elementary rules worth watching from a random row: chaos, nested
/// triangles, gliders on a lattice, traffic flow and stripes.
const RULES: [u8; 12] = [30, 90, 110, 150, 184, 54, 18, 106, 62, 41, 137, 169];
/// Seconds a band lasts without a beat; and at least, with beats.
const MAX_BAND: f32 = 6.0;
const MIN_BAND: f32 = 0.8;

/// Elementary one-dimensional automata printed as rows that enter at the
/// top and scroll down. Beats switch to another rule, so the history turns
/// into colored bands, one hue per rule, each continuing from the row the
/// previous rule left behind. Loudness sets the scroll speed, bass flips a
/// few cells and the spectrum lights up the columns.
pub struct Rules {
    /// Ring buffer of `gh` rows; `head` is the newest one.
    rows: Vec<u8>,
    /// Rule index and band number for each row.
    row_rule: Vec<(usize, u32)>,
    head: usize,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    rule: usize,
    band: u32,
    band_age: f32,
    rng: Rng,
    acc: f32,
}

impl Rules {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            row_rule: Vec::new(),
            head: 0,
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            rule: 0,
            band: 0,
            band_age: 0.0,
            rng: Rng::new(0x0110),
            acc: 0.0,
        }
    }

    fn switch_rule(&mut self) {
        let k = 1 + self.rng.next_u32() as usize % (RULES.len() - 1);
        self.rule = (self.rule + k) % RULES.len();
        self.band += 1;
        self.band_age = 0.0;
    }

    /// Flips `n` random cells of the newest row.
    fn disturb(&mut self, n: usize) {
        let row = self.head * self.gw;
        for _ in 0..n {
            let x = self.rng.next_u32() as usize % self.gw;
            self.rows[row + x] ^= 1;
        }
    }

    fn step(&mut self) {
        let w = self.gw;
        let rule = RULES[self.rule];
        let prev = self.head * w;
        self.head = (self.head + 1) % self.gh;
        let cur = self.head * w;
        let mut same = true;
        let mut alive = 0;
        for x in 0..w {
            let l = self.rows[prev + (x + w - 1) % w];
            let m = self.rows[prev + x];
            let r = self.rows[prev + (x + 1) % w];
            let v = (rule >> (l << 2 | m << 1 | r)) & 1;
            same &= v == m;
            alive += v as usize;
            self.rows[cur + x] = v;
        }
        self.row_rule[self.head] = (self.rule, self.band);
        // A frozen or empty row would print a flat band; stir it up.
        if same || alive == 0 || alive == w {
            self.disturb(w / 6 + 1);
        }
    }
}

impl Effect for Rules {
    fn name(&self) -> &'static str {
        "Rule Mix"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.scale = (c.w / 120).clamp(1, 3);
            self.gw = c.w.div_ceil(self.scale);
            self.gh = c.h.div_ceil(self.scale);
            self.rows = vec![0; self.gw * self.gh];
            self.row_rule = vec![(self.rule, self.band); self.gh];
            self.head = 0;
            if self.gw > 0 && self.gh > 0 {
                self.disturb(self.gw / 2 + 1);
                // Fill the screen right away instead of scrolling in.
                for _ in 0..self.gh {
                    self.step();
                }
            }
        }
        if self.gw == 0 || self.gh == 0 {
            return;
        }

        self.band_age += dt;
        if (f.beat && self.band_age > MIN_BAND) || self.band_age > MAX_BAND {
            self.switch_rule();
        }
        if f.beat && f.bass > 0.5 {
            self.disturb(1 + (f.bass * 4.0) as usize);
        }
        self.acc = (self.acc + dt * (14.0 + f.level * 30.0 + f.pulse * 12.0)).min(4.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.step();
        }

        let bins = f.spectrum.len();
        let column: Vec<f32> = (0..self.gw)
            .map(|x| if bins == 0 { 0.0 } else { f.spectrum[x * bins / self.gw] })
            .collect();
        let s = self.scale;
        for y in 0..c.h {
            let age = y / s;
            let r = (self.head + self.gh - age % self.gh) % self.gh;
            let (rule, band) = self.row_rule[r];
            // Golden-ratio hue steps keep neighboring rules apart.
            let hue = rule as f32 * 0.618;
            // The first row of a band (its oldest, at the bottom) is a seam.
            let older = (r + self.gh - 1) % self.gh;
            let seam = age + 1 < self.gh && self.row_rule[older].1 != band;
            let fade = 1.0 - 0.45 * age as f32 / self.gh as f32;
            let on = hsv(hue, 0.55, 0.95 * fade);
            let off = hsv(hue, 0.9, if seam { 0.45 } else { 0.1 * fade });
            let row = r * self.gw;
            for x in 0..c.w {
                let col = if self.rows[row + x / s] != 0 {
                    mix(on, [255, 255, 255], column[x / s] * 0.35)
                } else {
                    off
                };
                c.set(x, y, col);
            }
        }
    }
}
