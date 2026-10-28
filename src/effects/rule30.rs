use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix};

/// Elementary rules the automaton switches between on beats.
const RULES: [u8; 10] = [30, 90, 110, 150, 45, 73, 105, 22, 126, 182];
/// Rules that draw nested triangles when grown from a single cell.
const NESTED: [u8; 5] = [90, 150, 22, 126, 182];

/// A one-dimensional elementary cellular automaton printed line by line and
/// scrolling upward. Bass sets the print speed, every beat switches to the
/// next rule (and sometimes restarts from a single cell), each row keeps the
/// color it was printed in, and the spectrum lights up the columns.
pub struct Rule30 {
    /// Ring buffer of `gh` rows, `head` is the oldest one.
    rows: Vec<u8>,
    row_hue: Vec<f32>,
    head: usize,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    rule: usize,
    rng: Rng,
    acc: f32,
    hue: f32,
}

impl Rule30 {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            row_hue: Vec::new(),
            head: 0,
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            rule: 0,
            rng: Rng::new(0x30),
            acc: 0.0,
            hue: 0.0,
        }
    }

    fn newest(&self) -> usize {
        (self.head + self.gh - 1) % self.gh
    }

    /// Replaces the newest row with a single live cell or random noise.
    fn seed(&mut self, single: bool) {
        let r = self.newest() * self.gw;
        for x in 0..self.gw {
            self.rows[r + x] = if single { 0 } else { (self.rng.f32() < 0.5) as u8 };
        }
        if single {
            self.rows[r + self.gw / 2] = 1;
        }
    }

    fn print_row(&mut self) {
        let gw = self.gw;
        let rule = RULES[self.rule];
        let src = self.newest() * gw;
        let dst = self.head * gw;
        let mut live = 0;
        for x in 0..gw {
            let l = self.rows[src + (x + gw - 1) % gw];
            let m = self.rows[src + x];
            let r = self.rows[src + (x + 1) % gw];
            let v = (rule >> ((l << 2) | (m << 1) | r)) & 1;
            self.rows[dst + x] = v;
            live += v as usize;
        }
        self.row_hue[self.head] = self.hue;
        self.head = (self.head + 1) % self.gh;
        // A dead or saturated row would print the same line forever.
        if live == 0 || live == gw {
            self.seed(false);
        }
    }
}

impl Effect for Rule30 {
    fn name(&self) -> &'static str {
        "Rule 30"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.scale = if c.w >= 120 && c.h >= 60 { 2 } else { 1 };
            self.gw = c.w.div_ceil(self.scale);
            self.gh = c.h.div_ceil(self.scale);
            self.rows = vec![0; self.gw * self.gh];
            self.row_hue = vec![self.hue; self.gh];
            self.head = 0;
            if self.gw > 0 && self.gh > 0 {
                self.seed(true);
            }
        }
        if self.gw < 3 || self.gh < 2 {
            c.clear();
            return;
        }

        self.hue += dt * 0.02;
        if f.beat {
            self.rule = (self.rule + 1) % RULES.len();
            self.hue += 0.11;
            if NESTED.contains(&RULES[self.rule]) && self.rng.f32() < 0.5 {
                self.seed(true);
            }
        }
        self.acc = (self.acc + dt * (18.0 + f.bass * 60.0 + f.pulse * 25.0)).min(self.gh as f32);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.print_row();
        }

        let (gw, gh, s) = (self.gw, self.gh, self.scale);
        let bins = f.spectrum.len();
        let col_level: Vec<f32> = (0..gw)
            .map(|x| {
                if bins == 0 {
                    0.5
                } else {
                    f.spectrum[(x * bins / gw).min(bins - 1)]
                }
            })
            .collect();
        for y in 0..c.h {
            let r = y / s;
            let ring = (self.head + r) % gh;
            let age = r as f32 / (gh - 1) as f32;
            let fade = 0.25 + 0.75 * age * age;
            let hue = self.row_hue[ring];
            // The newest rows glow like a print head.
            let head_glow = ((r as f32 - (gh as f32 - 4.0)) / 4.0).clamp(0.0, 1.0);
            let off = hsv(hue + 0.5, 0.6, 0.04 + 0.03 * (t * 2.0).sin().abs() * f.level);
            for x in 0..c.w {
                let cx = x / s;
                let col = if self.rows[ring * gw + cx] != 0 {
                    let v = (0.35 + 0.65 * col_level[cx]) * fade;
                    let base = hsv(hue + cx as f32 / gw as f32 * 0.3, 0.8, v);
                    mix(base, [255, 255, 255], head_glow * 0.6)
                } else {
                    off
                };
                c.set(x, y, col);
            }
        }
    }
}
