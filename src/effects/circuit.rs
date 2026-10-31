use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, mix, scale};

const MAX_PULSES: usize = 240;
/// Grid steps for the eight trace directions, clockwise from east.
const DIRS: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
const MASK: Rgb = [10, 48, 30];
const COPPER: Rgb = [36, 118, 64];
const GOLD: Rgb = [196, 164, 80];

struct Trace {
    /// Dense pixel path, one entry per pixel step.
    path: Vec<(i32, i32)>,
}

struct Pulse {
    trace: usize,
    pos: f32,
    speed: f32,
    col: Rgb,
}

/// A procedurally routed circuit board: chips, 45° traces and gold pads,
/// with signal pulses racing along the traces. Level sets the pulse rate and
/// speed, every beat fires a burst, and the color of each pulse tells which
/// band was loudest when it left.
pub struct Circuit {
    w: usize,
    h: usize,
    board: Vec<Rgb>,
    traces: Vec<Trace>,
    pulses: Vec<Pulse>,
    /// Arrival flashes at trace ends: (x, y, color, age).
    flashes: Vec<(i32, i32, Rgb, f32)>,
    rng: Rng,
    spawn: f32,
}

impl Circuit {
    pub fn new() -> Self {
        Self {
            w: 0,
            h: 0,
            board: Vec::new(),
            traces: Vec::new(),
            pulses: Vec::new(),
            flashes: Vec::new(),
            rng: Rng::new(0xC1C0),
            spawn: 0.0,
        }
    }

    /// Routes a new board for the current size and paints it into `board`.
    fn build(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.traces.clear();
        self.pulses.clear();
        self.flashes.clear();
        let mut rng = Rng::new(0xB0A2D ^ (w as u32 * 7919 + h as u32));
        self.board = (0..w * h)
            .map(|i| {
                let n = ((i as f32 * 12.9898).sin() * 43_758.547).fract().abs();
                scale(MASK, 0.85 + 0.25 * n)
            })
            .collect();
        let cs = ((w.min(h) as f32 / 20.0).round() as usize).clamp(3, 8);
        let (gw, gh) = (w / cs, h / cs);
        if gw < 3 || gh < 3 {
            return;
        }
        let (ox, oy) = (((w - gw * cs) / 2 + cs / 2) as i32, ((h - gh * cs) / 2 + cs / 2) as i32);
        let center = |gx: i32, gy: i32| (ox + gx * cs as i32, oy + gy * cs as i32);
        let mut occ = vec![false; gw * gh];
        let (gwi, ghi) = (gw as i32, gh as i32);
        let free =
            |occ: &[bool], x: i32, y: i32| x >= 0 && y >= 0 && x < gwi && y < ghi && !occ[(y * gwi + x) as usize];

        // Chips first, each with pins along all four edges.
        let mut chips = Vec::new();
        let mut starts = Vec::new();
        let tries = (gw * gh / 90).clamp(1, 6);
        for _ in 0..tries * 8 {
            if chips.len() >= tries {
                break;
            }
            let cw = (3 + rng.next_u32() as usize % 5).min(gw.saturating_sub(4)) as i32;
            let ch = (2 + rng.next_u32() as usize % 3).min(gh.saturating_sub(4)) as i32;
            if cw < 1 || ch < 1 {
                break;
            }
            let x0 = 2 + (rng.f32() * (gwi - cw - 3).max(1) as f32) as i32;
            let y0 = 2 + (rng.f32() * (ghi - ch - 3).max(1) as f32) as i32;
            let clear = (y0 - 2..y0 + ch + 2).all(|y| (x0 - 2..x0 + cw + 2).all(|x| free(&occ, x, y)));
            if !clear {
                continue;
            }
            for y in y0..y0 + ch {
                for x in x0..x0 + cw {
                    occ[(y * gwi + x) as usize] = true;
                }
            }
            for x in x0..x0 + cw {
                starts.push((x, y0 - 1, 6));
                starts.push((x, y0 + ch, 2));
            }
            for y in y0..y0 + ch {
                starts.push((x0 - 1, y, 4));
                starts.push((x0 + cw, y, 0));
            }
            chips.push((x0, y0, cw, ch));
        }
        // Then some loose traces between two vias.
        for _ in 0..gw * gh / 12 {
            let x = (rng.f32() * gwi as f32) as i32;
            let y = (rng.f32() * ghi as f32) as i32;
            starts.push((x, y, (rng.next_u32() % 4 * 2) as usize));
        }

        let mut routes: Vec<(Vec<(i32, i32)>, bool)> = Vec::new();
        for (k, &(sx, sy, d0)) in starts.iter().enumerate() {
            let from_chip = k < starts.len() - gw * gh / 12;
            if !free(&occ, sx, sy) || (from_chip && rng.f32() < 0.25) {
                continue;
            }
            let mut cells = vec![(sx, sy)];
            occ[(sy * gwi + sx) as usize] = true;
            let (mut x, mut y, mut d) = (sx, sy, d0);
            let len = 3 + (rng.f32() * (gw + gh) as f32 * 0.5) as usize;
            // The first step leaves the chip straight; later ones may bend.
            for step in 0..len {
                if step > 1 && rng.f32() < 0.2 {
                    d = if rng.f32() < 0.5 { (d + 1) % 8 } else { (d + 7) % 8 };
                }
                let mut moved = false;
                for turn in [0, 1, 7] {
                    let nd = (d + turn) % 8;
                    let (nx, ny) = (x + DIRS[nd].0, y + DIRS[nd].1);
                    // Diagonals may not squeeze between two used cells.
                    let squeeze = nd % 2 == 1 && !free(&occ, nx, y) && !free(&occ, x, ny);
                    if free(&occ, nx, ny) && !squeeze {
                        (x, y, d) = (nx, ny, nd);
                        occ[(y * gwi + x) as usize] = true;
                        cells.push((x, y));
                        moved = true;
                        break;
                    }
                }
                if !moved {
                    break;
                }
            }
            if cells.len() >= 2 {
                routes.push((cells, from_chip));
            }
        }

        // Paint traces, pads and chips.
        let thick = cs >= 6;
        for (cells, from_chip) in &routes {
            let mut path = Vec::new();
            for pair in cells.windows(2) {
                let (a, b) = (center(pair[0].0, pair[0].1), center(pair[1].0, pair[1].1));
                let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
                for i in 0..n {
                    path.push((a.0 + (b.0 - a.0) * i / n, a.1 + (b.1 - a.1) * i / n));
                }
            }
            let last = cells[cells.len() - 1];
            path.push(center(last.0, last.1));
            for &(x, y) in &path {
                self.paint(x, y, COPPER);
                if thick {
                    self.paint(x + 1, y, COPPER);
                    self.paint(x, y + 1, COPPER);
                }
            }
            let r = (cs as f32 * 0.42).max(1.0);
            let (ex, ey) = path[path.len() - 1];
            self.pad(ex, ey, r);
            if !from_chip {
                self.pad(path[0].0, path[0].1, r);
            }
            self.traces.push(Trace { path });
        }
        let half = cs as i32 / 2;
        for &(x0, y0, cw, ch) in &chips {
            let (ax, ay) = center(x0, y0);
            let (bx, by) = center(x0 + cw - 1, y0 + ch - 1);
            // Pins reach out to the neighboring grid cells.
            for x in x0..x0 + cw {
                let px = center(x, y0).0;
                for y in ay - cs as i32..=by + cs as i32 {
                    self.paint(px, y, [150, 150, 160]);
                }
            }
            for y in y0..y0 + ch {
                let py = center(x0, y).1;
                for x in ax - cs as i32..=bx + cs as i32 {
                    self.paint(x, py, [150, 150, 160]);
                }
            }
            for y in ay - half..=by + half {
                for x in ax - half..=bx + half {
                    let edge = y == ay - half;
                    self.paint(x, y, if edge { [44, 44, 52] } else { [20, 20, 25] });
                }
            }
            self.paint(ax - half + 1, ay - half + 2, [80, 80, 90]);
        }
    }

    fn paint(&mut self, x: i32, y: i32, col: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.board[y as usize * self.w + x as usize] = col;
        }
    }

    fn pad(&mut self, cx: i32, cy: i32, r: f32) {
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d <= r {
                    let hole = r >= 2.0 && d < r * 0.4;
                    self.paint(cx + dx, cy + dy, if hole { [8, 20, 14] } else { GOLD });
                }
            }
        }
    }
}

impl Effect for Circuit {
    fn name(&self) -> &'static str {
        "Circuit"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w != self.w || c.h != self.h || self.board.len() != c.w * c.h {
            self.build(c.w, c.h);
        }
        if c.w == 0 || c.h == 0 {
            return;
        }
        c.px.copy_from_slice(&self.board);
        if self.traces.is_empty() {
            return;
        }

        let size = (c.w.max(c.h) as f32 / 240.0).max(0.3);
        let tint = if f.bass >= f.mid && f.bass >= f.treble {
            0.08
        } else if f.mid >= f.treble {
            0.5
        } else {
            0.85
        };
        self.spawn += dt * (3.0 + 24.0 * f.level) * size.max(0.5);
        let mut burst = if f.beat {
            6 + (16.0 * f.bass * size.max(0.4)) as usize
        } else {
            0
        };
        while self.spawn >= 1.0 {
            self.spawn -= 1.0;
            burst += 1;
        }
        for _ in 0..burst.min(32) {
            if self.pulses.len() >= MAX_PULSES {
                break;
            }
            let trace = self.rng.next_u32() as usize % self.traces.len();
            let hue = tint + self.rng.range(-0.05, 0.05);
            self.pulses.push(Pulse {
                trace,
                pos: 0.0,
                speed: self.rng.range(0.7, 1.3) * (45.0 + 90.0 * f.level) * size,
                col: hsv(hue, 0.45, 1.0),
            });
        }

        let tail = (24.0 * size).max(5.0);
        let boost = 1.0 + 0.6 * f.pulse;
        for p in &mut self.pulses {
            p.pos += p.speed * dt;
            let path = &self.traces[p.trace].path;
            let head = p.pos as i32;
            for k in 0..tail as i32 {
                let i = head - k;
                if i < 0 || i as usize >= path.len() {
                    continue;
                }
                let fall = 1.0 - k as f32 / tail;
                let (x, y) = path[i as usize];
                let v = fall * fall * boost;
                c.plot_add(x, y, scale(p.col, v));
                let halo = if k == 0 { 0.8 * boost } else { 0.3 * v };
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    c.plot_add(x + dx, y + dy, scale(p.col, halo));
                }
            }
            if head as usize >= path.len() && p.speed > 0.0 {
                let (x, y) = path[path.len() - 1];
                self.flashes.push((x, y, p.col, 0.0));
                p.speed = -1.0;
            }
        }
        self.pulses.retain(|p| p.speed > 0.0);

        let r = (size * 3.0).max(1.5);
        for fl in &mut self.flashes {
            fl.3 += dt;
            let k = (1.0 - fl.3 * 4.0).max(0.0);
            let ri = r.ceil() as i32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let d = ((dx * dx + dy * dy) as f32).sqrt() / r;
                    if d < 1.0 {
                        c.plot_add(
                            fl.0 + dx,
                            fl.1 + dy,
                            scale(mix(fl.2, [255, 255, 255], 0.4), k * (1.0 - d)),
                        );
                    }
                }
            }
        }
        self.flashes.retain(|fl| fl.3 < 0.25);
        if self.flashes.len() > MAX_PULSES {
            let n = self.flashes.len() - MAX_PULSES;
            self.flashes.drain(..n);
        }
    }
}
