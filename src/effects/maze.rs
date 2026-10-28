use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, mix, scale};
use std::collections::VecDeque;

const NONE: u32 = u32::MAX;
const HOLD: f32 = 3.0;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Carve,
    Flood,
    Trace,
    Hold,
}

/// A maze carved live by a recursive backtracker, then flooded from the
/// entrance with a rainbow of distances, and finally solved with a bright
/// path from corner to corner before a new one starts. Loudness sets the
/// carving and solving speed; beats make the walls and the path flash.
pub struct Maze {
    mw: usize,
    mh: usize,
    /// Passage open to the right / below of each cell.
    right: Vec<bool>,
    down: Vec<bool>,
    visited: Vec<bool>,
    in_stack: Vec<bool>,
    stack: Vec<usize>,
    dist: Vec<u32>,
    queue: VecDeque<usize>,
    max_dist: u32,
    path: Vec<usize>,
    path_pos: Vec<u32>,
    shown: f32,
    phase: Phase,
    hold: f32,
    size: (usize, usize),
    block: usize,
    rng: Rng,
    acc: f32,
    hue: f32,
}

impl Maze {
    pub fn new() -> Self {
        Self {
            mw: 0,
            mh: 0,
            right: Vec::new(),
            down: Vec::new(),
            visited: Vec::new(),
            in_stack: Vec::new(),
            stack: Vec::new(),
            dist: Vec::new(),
            queue: VecDeque::new(),
            max_dist: 0,
            path: Vec::new(),
            path_pos: Vec::new(),
            shown: 0.0,
            phase: Phase::Carve,
            hold: 0.0,
            size: (0, 0),
            block: 1,
            rng: Rng::new(0x3A2E),
            acc: 0.0,
            hue: 0.0,
        }
    }

    fn reset(&mut self) {
        let n = self.mw * self.mh;
        self.right = vec![false; n];
        self.down = vec![false; n];
        self.visited = vec![false; n];
        self.in_stack = vec![false; n];
        self.dist = vec![NONE; n];
        self.path_pos = vec![NONE; n];
        self.stack.clear();
        self.queue.clear();
        self.path.clear();
        self.max_dist = 0;
        self.shown = 0.0;
        self.hold = 0.0;
        self.acc = 0.0;
        self.phase = Phase::Carve;
        self.hue = self.rng.f32();
        if n > 0 {
            self.visited[0] = true;
            self.in_stack[0] = true;
            self.stack.push(0);
        }
    }

    /// Open neighbors of cell `i`.
    fn exits(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let (x, y) = (i % self.mw, i / self.mw);
        [
            (x + 1 < self.mw && self.right[i]).then(|| i + 1),
            (x > 0 && self.right[i.wrapping_sub(1)]).then(|| i - 1),
            (y + 1 < self.mh && self.down[i]).then(|| i + self.mw),
            (y > 0 && self.down[i.wrapping_sub(self.mw)]).then(|| i - self.mw),
        ]
        .into_iter()
        .flatten()
    }

    fn carve_step(&mut self) {
        let Some(&i) = self.stack.last() else {
            self.phase = Phase::Flood;
            self.dist[0] = 0;
            self.queue.push_back(0);
            return;
        };
        let (x, y) = (i % self.mw, i / self.mw);
        let mut options = [0usize; 4];
        let mut n = 0;
        let mut offer = |ok: bool, j: usize| {
            if ok && !self.visited[j] {
                options[n] = j;
                n += 1;
            }
        };
        offer(x + 1 < self.mw, i + 1);
        offer(x > 0, i.wrapping_sub(1));
        offer(y + 1 < self.mh, i + self.mw);
        offer(y > 0, i.wrapping_sub(self.mw));
        if n == 0 {
            self.stack.pop();
            self.in_stack[i] = false;
            return;
        }
        let j = options[self.rng.next_u32() as usize % n];
        match j {
            _ if j == i + 1 => self.right[i] = true,
            _ if j + 1 == i => self.right[j] = true,
            _ if j == i + self.mw => self.down[i] = true,
            _ => self.down[j] = true,
        }
        self.visited[j] = true;
        self.in_stack[j] = true;
        self.stack.push(j);
    }

    fn flood_step(&mut self) {
        let Some(i) = self.queue.pop_front() else {
            self.build_path();
            self.phase = Phase::Trace;
            return;
        };
        let d = self.dist[i] + 1;
        let next: Vec<usize> = self.exits(i).filter(|&j| self.dist[j] == NONE).collect();
        for j in next {
            self.dist[j] = d;
            self.max_dist = self.max_dist.max(d);
            self.queue.push_back(j);
        }
    }

    /// Walks back from the exit along decreasing distances.
    fn build_path(&mut self) {
        let mut cur = self.mw * self.mh - 1;
        if self.dist[cur] == NONE {
            return;
        }
        self.path.push(cur);
        while self.dist[cur] > 0 {
            let d = self.dist[cur];
            let Some(prev) = self.exits(cur).find(|&j| self.dist[j] == d - 1) else {
                break;
            };
            cur = prev;
            self.path.push(cur);
        }
        self.path.reverse();
        for (k, &i) in self.path.iter().enumerate() {
            self.path_pos[i] = k as u32;
        }
    }

    fn advance(&mut self) {
        match self.phase {
            Phase::Carve => self.carve_step(),
            Phase::Flood => self.flood_step(),
            Phase::Trace => {
                self.shown += 1.0;
                if self.shown >= self.path.len() as f32 {
                    self.phase = Phase::Hold;
                }
            }
            Phase::Hold => {}
        }
    }
}

impl Effect for Maze {
    fn name(&self) -> &'static str {
        "Maze"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.block = (c.h.min(c.w / 2) / 40).clamp(1, 4);
            self.mw = (c.w / self.block).saturating_sub(1) / 2;
            self.mh = (c.h / self.block).saturating_sub(1) / 2;
            self.reset();
        }
        c.clear();
        if self.mw < 2 || self.mh < 2 {
            return;
        }

        let rate = match self.phase {
            Phase::Carve | Phase::Flood => (self.mw * self.mh) as f32 / 6.0 * (0.4 + 1.6 * f.level + f.pulse),
            _ => self.path.len() as f32 / 2.5 * (0.5 + f.level),
        };
        self.acc = (self.acc + dt * rate).min(2000.0);
        while self.acc >= 1.0 && self.phase != Phase::Hold {
            self.acc -= 1.0;
            self.advance();
        }
        if self.phase == Phase::Hold {
            self.hold += dt;
            if self.hold > HOLD {
                self.reset();
            }
        }

        let (mw, mh) = (self.mw, self.mh);
        let wall = hsv(self.hue + 0.6, 0.5, 0.1 + 0.3 * f.pulse);
        let path_col = mix(hsv(self.hue + 0.15, 0.5, 1.0), [255, 255, 255], f.pulse);
        let shown = self.shown as u32;
        // Base color per cell, and the path color on top of it.
        let colors: Vec<(Rgb, bool)> = (0..mw * mh)
            .map(|i| {
                let on_path = self.path_pos[i] != NONE && self.path_pos[i] <= shown;
                let base = if self.phase == Phase::Carve {
                    if self.stack.last() == Some(&i) {
                        [255, 255, 255]
                    } else if self.in_stack[i] {
                        hsv(self.hue + i as f32 / (mw * mh) as f32 * 0.1, 0.8, 0.85)
                    } else if self.visited[i] {
                        hsv(self.hue + 0.5, 0.35, 0.3)
                    } else {
                        [6, 6, 12]
                    }
                } else if self.dist[i] != NONE {
                    let d = self.dist[i] as f32 / self.max_dist.max(1) as f32;
                    let v = if self.phase == Phase::Flood { 0.65 } else { 0.4 };
                    hsv(self.hue + d * 0.8, 0.75, v)
                } else {
                    hsv(self.hue + 0.5, 0.35, 0.3)
                };
                (if on_path { path_col } else { base }, on_path)
            })
            .collect();

        let b = self.block;
        let (ox, oy) = ((c.w - (2 * mw + 1) * b) / 2, (c.h - (2 * mh + 1) * b) / 2);
        for by in 0..2 * mh + 1 {
            for bx in 0..2 * mw + 1 {
                let col = match (bx % 2, by % 2) {
                    (1, 1) => colors[(by / 2) * mw + bx / 2].0,
                    (0, 1) if bx > 0 && bx < 2 * mw => {
                        let i = (by / 2) * mw + bx / 2 - 1;
                        connect(self.right[i], colors[i], colors[i + 1], wall)
                    }
                    (1, 0) if by > 0 && by < 2 * mh => {
                        let i = (by / 2 - 1) * mw + bx / 2;
                        connect(self.down[i], colors[i], colors[i + mw], wall)
                    }
                    _ => wall,
                };
                for py in 0..b {
                    for px in 0..b {
                        c.set(ox + bx * b + px, oy + by * b + py, col);
                    }
                }
            }
        }

        // Entrance and exit markers, plus a spark running along the path.
        let marker = |c: &mut Canvas, i: usize, col: Rgb| {
            let (bx, by) = (2 * (i % mw) + 1, 2 * (i / mw) + 1);
            for py in 0..b {
                for px in 0..b {
                    c.set(ox + bx * b + px, oy + by * b + py, col);
                }
            }
        };
        let blink = 0.6 + 0.4 * (t * 6.0).sin();
        marker(c, 0, scale([60, 255, 120], blink));
        marker(c, mw * mh - 1, scale([255, 70, 90], blink));
        if self.phase == Phase::Hold && !self.path.is_empty() {
            let k = ((t * 20.0) as usize) % self.path.len();
            marker(c, self.path[k], [255, 255, 255]);
        }
    }
}

/// Color of the block between two cells: a passage blends them, otherwise wall.
fn connect(open: bool, a: (Rgb, bool), b: (Rgb, bool), wall: Rgb) -> Rgb {
    if !open {
        wall
    } else if a.1 && b.1 {
        a.0
    } else if a.1 || b.1 {
        // Don't let the path color leak into side branches.
        if a.1 { b.0 } else { a.0 }
    } else {
        mix(a.0, b.0, 0.5)
    }
}
