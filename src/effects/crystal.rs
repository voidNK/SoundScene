use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, mix, rgb, scale};

/// Axial neighbor offsets on the hex grid.
const NEIGHBORS: [(i32, i32); 6] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, -1), (-1, 1)];
const SQRT3: f32 = 1.732_050_8;
/// Seconds a finished flake stays on screen before it melts and regrows.
const HOLD: f32 = 2.5;
/// Seconds before regrowing even if the flake never reached the edge.
const LIFETIME: f32 = 45.0;

/// Reiter's snowflake model on a hex grid: water vapor diffuses, cells next
/// to the ice soak it up and freeze once they reach 1, growing six-fold
/// dendrites and plates. Loudness speeds up the growth and is mixed into
/// the vapor parameters of each new flake, beats make the ice sparkle, and
/// a finished flake melts away and regrows with new parameters.
pub struct Crystal {
    s: Vec<f32>,
    next: Vec<f32>,
    receptive: Vec<bool>,
    /// Hex radius of the grid; the grid is (2r+1)² axial cells.
    r: i32,
    size: (usize, usize),
    beta: f32,
    gamma: f32,
    rng: Rng,
    acc: f32,
    age: f32,
    done: f32,
    /// Hex distance of the farthest frozen cell.
    reach: i32,
    angle: f32,
}

impl Crystal {
    pub fn new() -> Self {
        Self {
            s: Vec::new(),
            next: Vec::new(),
            receptive: Vec::new(),
            r: 0,
            size: (0, 0),
            beta: 0.4,
            gamma: 0.002,
            rng: Rng::new(0x1CE),
            acc: 0.0,
            age: 0.0,
            done: 0.0,
            reach: 0,
            angle: 0.0,
        }
    }

    fn n(&self) -> i32 {
        2 * self.r + 1
    }

    fn dist(&self, q: i32, r: i32) -> i32 {
        let (q, r) = (q - self.r, r - self.r);
        q.abs().max(r.abs()).max((q + r).abs())
    }

    fn regrow(&mut self, level: f32) {
        self.beta = 0.35 + 0.3 * self.rng.f32() + 0.1 * level;
        self.gamma = 0.0005 + 0.004 * self.rng.f32() * (0.5 + level);
        self.s.fill(self.beta);
        let n = self.n();
        let c = (self.r * n + self.r) as usize;
        if c < self.s.len() {
            self.s[c] = 1.0;
        }
        self.age = 0.0;
        self.done = 0.0;
        self.reach = 0;
        self.angle = self.rng.f32() * std::f32::consts::TAU;
    }

    fn step(&mut self) {
        let n = self.n();
        let nu = n as usize;
        // Receptive cells are frozen or touch ice. They keep their water and
        // gain `gamma`; everything else diffuses freely.
        self.receptive.fill(false);
        for r in 1..n - 1 {
            for q in 1..n - 1 {
                let i = r as usize * nu + q as usize;
                if self.s[i] >= 1.0 {
                    self.receptive[i] = true;
                    for (dq, dr) in NEIGHBORS {
                        self.receptive[((r + dr) * n + q + dq) as usize] = true;
                    }
                }
            }
        }
        for r in 1..n - 1 {
            for q in 1..n - 1 {
                let i = r as usize * nu + q as usize;
                if self.dist(q, r) >= self.r - 1 {
                    self.next[i] = self.beta;
                    continue;
                }
                let mut sum = 0.0;
                for (dq, dr) in NEIGHBORS {
                    let j = ((r + dr) * n + q + dq) as usize;
                    if !self.receptive[j] {
                        sum += self.s[j];
                    }
                }
                let v = self.s[i];
                let (u, keep) = if self.receptive[i] {
                    (0.0, v + self.gamma)
                } else {
                    (v, 0.0)
                };
                self.next[i] = u + 0.5 * (sum / 6.0 - u) + keep;
                if keep >= 1.0 {
                    self.reach = self.reach.max(self.dist(q, r));
                }
            }
        }
        std::mem::swap(&mut self.s, &mut self.next);
    }
}

impl Effect for Crystal {
    fn name(&self) -> &'static str {
        "Crystal"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            let half = c.w.min(c.h) as f32 / 2.0;
            self.r = (half / 1.4).clamp(8.0, 44.0) as i32;
            let n = self.n() as usize;
            self.s = vec![0.0; n * n];
            self.next = vec![0.0; n * n];
            self.receptive = vec![false; n * n];
            self.regrow(f.level);
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        self.age += dt;
        self.angle += dt * 0.04;
        if self.reach >= self.r - 3 || self.age > LIFETIME {
            self.done += dt;
            if self.done > HOLD {
                self.regrow(f.level);
            }
        } else {
            self.acc = (self.acc + dt * (40.0 + 160.0 * f.level + 60.0 * f.bass)).min(30.0);
            while self.acc >= 1.0 {
                self.acc -= 1.0;
                self.step();
            }
        }

        // Hex size so the whole hexagon fits the canvas.
        let rf = self.r as f32 + 0.5;
        let sz = (c.h as f32 / 2.0 / (1.5 * rf)).min(c.w as f32 / 2.0 / (SQRT3 * rf));
        if sz <= 0.0 {
            return;
        }
        let (sin, cos) = self.angle.sin_cos();
        let melt = (1.0 - self.done / HOLD).clamp(0.0, 1.0);
        let sparkle = 0.2 * f.pulse;
        let deep = rgb(0.01, 0.02, 0.06);
        let vapor = rgb(0.1, 0.25, 0.5);
        let ice = rgb(0.3, 0.6, 0.95);
        let white = rgb(0.92, 0.97, 1.0);
        let n = self.n();
        let (cx, cy) = (c.w as f32 / 2.0, c.h as f32 / 2.0);
        for y in 0..c.h {
            for x in 0..c.w {
                let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let (rx, ry) = (px * cos - py * sin, px * sin + py * cos);
                // Pixel -> fractional axial coordinates, then cube rounding.
                let fq = (SQRT3 / 3.0 * rx - ry / 3.0) / sz;
                let fr = (2.0 / 3.0 * ry) / sz;
                let fs = -fq - fr;
                let (mut q, mut r, s) = (fq.round(), fr.round(), fs.round());
                let (dq, dr, ds) = ((q - fq).abs(), (r - fr).abs(), (s - fs).abs());
                if dq > dr && dq > ds {
                    q = -r - s;
                } else if dr > ds {
                    r = -q - s;
                }
                let (q, r) = (q as i32 + self.r, r as i32 + self.r);
                let col = if q < 0 || r < 0 || q >= n || r >= n || self.dist(q, r) >= self.r {
                    deep
                } else {
                    let v = self.s[(r * n + q) as usize];
                    if v >= 1.0 {
                        let k = ((v - 1.0) * 3.0).clamp(0.0, 1.0);
                        scale(mix(white, ice, k), (0.9 + sparkle) * melt + 0.1)
                    } else {
                        mix(deep, vapor, v * 0.6)
                    }
                };
                c.set(x, y, col);
            }
        }
    }
}
