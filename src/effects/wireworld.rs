use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, mix, rgb, scale};

const EMPTY: u8 = 0;
const WIRE: u8 = 1;
const HEAD: u8 = 2;
const TAIL: u8 = 3;
/// Seconds before a new circuit is generated.
const LIFETIME: f32 = 40.0;

/// Axis-aligned wire loop, inclusive corners.
struct Loop {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Loop {
    /// Perimeter cells in clockwise order.
    fn perimeter(&self) -> Vec<(usize, usize)> {
        let mut p = Vec::new();
        p.extend((self.x0..self.x1).map(|x| (x, self.y0)));
        p.extend((self.y0..self.y1).map(|y| (self.x1, y)));
        p.extend((self.x0 + 1..=self.x1).rev().map(|x| (x, self.y1)));
        p.extend((self.y0 + 1..=self.y1).rev().map(|y| (self.x0, y)));
        p
    }
}

/// Wireworld on a procedurally generated circuit board: rectangular wire
/// loops linked by straight traces, some of them with diodes, carry
/// electrons that circle, split at junctions and cancel out head-on. Beats
/// inject new electrons, loudness sets the clock rate and electrons leave a
/// brief glow on the copper.
pub struct Wireworld {
    cells: Vec<u8>,
    next: Vec<u8>,
    glow: Vec<f32>,
    /// Indices of all conductor cells, the only ones that can change.
    wires: Vec<usize>,
    loops: Vec<Vec<(usize, usize)>>,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    heads: usize,
    rng: Rng,
    acc: f32,
    age: f32,
}

impl Wireworld {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            next: Vec::new(),
            glow: Vec::new(),
            wires: Vec::new(),
            loops: Vec::new(),
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            heads: 0,
            rng: Rng::new(0x3E3),
            acc: 0.0,
            age: 0.0,
        }
    }

    fn rand(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.rng.next_u32() as usize % (hi - lo + 1).max(1)
    }

    fn generate(&mut self) {
        let (gw, gh) = (self.gw, self.gh);
        self.cells.fill(EMPTY);
        self.glow.fill(0.0);
        self.age = 0.0;
        self.heads = 0;
        let mut rects: Vec<Loop> = Vec::new();
        let max_w = (gw / 4).max(4);
        let max_h = (gh / 3).max(4);
        for _ in 0..300 {
            let w = self.rand(3, max_w);
            let h = self.rand(3, max_h);
            if w + 4 > gw || h + 4 > gh {
                continue;
            }
            let x0 = self.rand(2, gw - w - 2);
            let y0 = self.rand(2, gh - h - 2);
            let (x1, y1) = (x0 + w - 1, y0 + h - 1);
            // Keep at least two empty cells between loops.
            let clear = rects
                .iter()
                .all(|r| x0 > r.x1 + 2 || r.x0 > x1 + 2 || y0 > r.y1 + 2 || r.y0 > y1 + 2);
            if clear {
                rects.push(Loop { x0, y0, x1, y1 });
            }
        }
        self.loops = rects.iter().map(Loop::perimeter).collect();
        for p in &self.loops {
            for &(x, y) in p {
                self.cells[y * gw + x] = WIRE;
            }
        }
        // Traces between loops facing each other, horizontal then vertical.
        for a in 0..rects.len() {
            for b in 0..rects.len() {
                let (ra, rb) = (&rects[a], &rects[b]);
                let (lo, hi) = (ra.y0.max(rb.y0) + 1, ra.y1.min(rb.y1));
                if rb.x0 > ra.x1 + 3 && lo < hi && rb.x0 - ra.x1 < gw / 3 {
                    let y = lo + self.rng.next_u32() as usize % (hi - lo);
                    self.trace(ra.x1 + 1, rb.x0 - 1, y, true);
                }
                let (lo, hi) = (ra.x0.max(rb.x0) + 1, ra.x1.min(rb.x1));
                if rb.y0 > ra.y1 + 3 && lo < hi && rb.y0 - ra.y1 < gh / 2 {
                    let x = lo + self.rng.next_u32() as usize % (hi - lo);
                    self.trace(ra.y1 + 1, rb.y0 - 1, x, false);
                }
            }
        }
        self.wires = (0..self.cells.len()).filter(|&i| self.cells[i] != EMPTY).collect();
        for _ in 0..self.loops.len().div_ceil(2) {
            self.inject();
        }
    }

    /// Lays a straight trace from `a` to `b` (inclusive) along row/column
    /// `k`, if the five-cell-wide corridor around it is empty. Long traces
    /// sometimes get a diode in the middle.
    fn trace(&mut self, a: usize, b: usize, k: usize, horizontal: bool) {
        let gw = self.gw;
        let at = |u: usize, v: usize| if horizontal { v * gw + u } else { u * gw + v };
        if k < 2 || k + 2 >= if horizontal { self.gh } else { self.gw } {
            return;
        }
        for u in a..=b {
            for v in k - 2..=k + 2 {
                // The trace ends touch their loops; only the corridor between
                // must be empty.
                let end = (u == a || u == b) && v != k;
                if !end && self.cells[at(u, v)] != EMPTY {
                    return;
                }
            }
        }
        for u in a..=b {
            self.cells[at(u, k)] = WIRE;
        }
        if b >= a + 7 && self.rng.f32() < 0.85 {
            // Diode: the wire splits around a one-cell gap and only lets
            // electrons through in one direction.
            let m = (a + b) / 2 - 1;
            let (head, gap) = if self.rng.f32() < 0.5 {
                (m + 1, m + 2)
            } else {
                (m + 2, m + 1)
            };
            self.cells[at(gap, k)] = EMPTY;
            for v in [k - 1, k + 1] {
                self.cells[at(head, v)] = WIRE;
                self.cells[at(gap, v)] = WIRE;
            }
        }
    }

    /// Starts an electron at a random spot of a random loop.
    fn inject(&mut self) {
        if self.loops.is_empty() {
            return;
        }
        let l = self.rng.next_u32() as usize % self.loops.len();
        let p = &self.loops[l];
        let k = self.rng.next_u32() as usize % p.len();
        let (hx, hy) = p[k];
        let (tx, ty) = p[(k + p.len() - 1) % p.len()];
        let (h, t) = (hy * self.gw + hx, ty * self.gw + tx);
        if self.cells[h] == WIRE && self.cells[t] == WIRE {
            self.cells[h] = HEAD;
            self.cells[t] = TAIL;
        }
    }

    /// Returns the number of electron heads after the step.
    fn step(&mut self) -> usize {
        let w = self.gw;
        // Junctions multiply electrons; past this density some fizzle out so
        // the board never clogs up completely.
        let limit = self.wires.len() / 24 + 1;
        let kill = if self.heads > limit {
            0.3 * (self.heads - limit) as f32 / self.heads as f32
        } else {
            0.0
        };
        let mut heads = 0;
        for &i in &self.wires {
            self.next[i] = match self.cells[i] {
                HEAD => TAIL,
                TAIL => WIRE,
                _ => {
                    // Wires never touch the border, so all neighbors exist.
                    let c = &self.cells;
                    let n = [i - w - 1, i - w, i - w + 1, i - 1, i + 1, i + w - 1, i + w, i + w + 1]
                        .iter()
                        .filter(|&&j| c[j] == HEAD)
                        .count();
                    if (n == 1 || n == 2) && (kill == 0.0 || self.rng.f32() >= kill) {
                        heads += 1;
                        HEAD
                    } else {
                        WIRE
                    }
                }
            };
        }
        for &i in &self.wires {
            self.cells[i] = self.next[i];
            if self.cells[i] == HEAD {
                self.glow[i] = 1.0;
            }
        }
        self.heads = heads;
        heads
    }
}

impl Effect for Wireworld {
    fn name(&self) -> &'static str {
        "Wireworld"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.scale = if c.w >= 160 && c.h >= 80 { 2 } else { 1 };
            self.gw = c.w.div_ceil(self.scale);
            self.gh = c.h.div_ceil(self.scale);
            let n = self.gw * self.gh;
            self.cells = vec![EMPTY; n];
            self.next = vec![EMPTY; n];
            self.glow = vec![0.0; n];
            self.generate();
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        self.age += dt;
        if self.age > LIFETIME {
            self.generate();
        }
        if f.beat && self.heads < self.wires.len() / 16 {
            for _ in 0..1 + (f.bass * 2.0) as usize {
                self.inject();
            }
        }
        let decay = (-dt * 4.0).exp();
        for g in &mut self.glow {
            *g *= decay;
        }
        self.acc = (self.acc + dt * (12.0 + f.level * 25.0 + f.bass * 10.0)).min(3.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            if self.step() == 0 {
                self.inject();
            }
        }

        let board = rgb(0.02, 0.07, 0.04);
        let copper = rgb(0.5, 0.28, 0.08);
        let spark = rgb(0.45, 0.85, 1.0);
        let head = scale(rgb(0.85, 0.95, 1.0), 0.85 + 0.15 * f.pulse);
        let tail = rgb(0.2, 0.45, 1.0);
        let s = self.scale;
        for y in 0..c.h {
            let row = (y / s) * self.gw;
            for x in 0..c.w {
                let i = row + x / s;
                let col = match self.cells[i] {
                    HEAD => head,
                    TAIL => tail,
                    WIRE => mix(copper, spark, self.glow[i] * 0.8),
                    _ => board,
                };
                c.set(x, y, col);
            }
        }
    }
}
