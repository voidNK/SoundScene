use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix};

/// Seconds a pile takes to fill the screen at average loudness.
const FILL_TIME: f32 = 40.0;
/// Toppling work allowed per frame; the rest carries over.
const TOPPLE_BUDGET: usize = 300_000;
/// Average height of a stable pile, used to size the grain count.
const MEAN_HEIGHT: f32 = 2.1;

/// Abelian sandpile: grains rain onto the center, any cell holding four
/// topples one onto each neighbor, and the avalanches grow the classic
/// four-color fractal. Loudness sets the grain rate, beats drop a heap at
/// once and flash the cells that are toppling; a full screen starts over
/// with a new palette.
pub struct Sandpile {
    height: Vec<u32>,
    touched: Vec<bool>,
    /// Cells that may hold four or more grains.
    unstable: Vec<usize>,
    gw: usize,
    gh: usize,
    size: (usize, usize),
    pending: f32,
    dropped: f32,
    hue: f32,
}

impl Sandpile {
    pub fn new() -> Self {
        Self {
            height: Vec::new(),
            touched: Vec::new(),
            unstable: Vec::new(),
            gw: 0,
            gh: 0,
            size: (0, 0),
            pending: 0.0,
            dropped: 0.0,
            hue: 0.6,
        }
    }

    fn restart(&mut self) {
        self.height.fill(0);
        self.touched.fill(false);
        self.unstable.clear();
        self.dropped = 0.0;
        self.pending = 0.0;
        self.hue += 0.17;
    }

    fn drop_grains(&mut self, n: u32) {
        let i = self.gh / 2 * self.gw + self.gw / 2;
        self.height[i] += n;
        self.touched[i] = true;
        self.unstable.push(i);
        self.dropped += n as f32;
    }

    /// Topples unstable cells until the pile is stable or the budget is
    /// spent. Grains that fall off the edge are lost.
    fn relax(&mut self) {
        let (w, h) = (self.gw, self.gh);
        let mut work = 0;
        while let Some(i) = self.unstable.pop() {
            let v = self.height[i];
            if v < 4 {
                continue;
            }
            let q = v / 4;
            self.height[i] = v % 4;
            let (x, y) = (i % w, i / w);
            let give = |j: usize, this: &mut Self| {
                this.height[j] += q;
                this.touched[j] = true;
                if this.height[j] >= 4 {
                    this.unstable.push(j);
                }
            };
            if x > 0 {
                give(i - 1, self);
            }
            if x + 1 < w {
                give(i + 1, self);
            }
            if y > 0 {
                give(i - w, self);
            }
            if y + 1 < h {
                give(i + w, self);
            }
            work += 1;
            if work >= TOPPLE_BUDGET {
                break;
            }
        }
    }
}

impl Effect for Sandpile {
    fn name(&self) -> &'static str {
        "Sandpile"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.gw = c.w;
            self.gh = c.h;
            let n = self.gw * self.gh;
            self.height = vec![0; n];
            self.touched = vec![false; n];
            self.restart();
        }
        if self.gw == 0 || self.gh == 0 {
            return;
        }

        let full = self.gw as f32 * self.gh as f32 * MEAN_HEIGHT;
        if self.dropped >= full && self.unstable.is_empty() {
            self.restart();
        }
        self.pending += dt * full / FILL_TIME * (0.3 + 1.6 * f.level);
        if f.beat {
            self.pending += full * (0.004 + 0.01 * f.bass);
        }
        if self.pending >= 1.0 && self.dropped < full {
            let n = self.pending as u32;
            self.pending -= n as f32;
            self.drop_grains(n);
        }
        self.relax();

        let bright = 0.75 + 0.25 * f.pulse;
        let palette: [Rgb; 4] = [
            hsv(self.hue, 0.9, 0.25 * bright),
            hsv(self.hue + 0.1, 0.8, 0.6 * bright),
            hsv(self.hue + 0.35, 0.85, 0.85 * bright),
            hsv(self.hue + 0.55, 0.6, bright),
        ];
        let hot = mix(palette[3], [255, 255, 255], 0.5 + 0.5 * f.pulse);
        for y in 0..c.h {
            for x in 0..c.w {
                let i = y * self.gw + x;
                let col = if !self.touched[i] {
                    [0, 0, 0]
                } else {
                    match self.height[i] {
                        h @ 0..=3 => palette[h as usize],
                        _ => hot,
                    }
                };
                c.set(x, y, col);
            }
        }
    }
}
