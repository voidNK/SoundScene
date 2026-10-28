use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, mix};

/// Turn sequences for multi-color ants: on a cell in state `k` the ant turns
/// by `rule[k]` and advances the cell to the next state.
const RULES: [&[u8]; 6] = [b"RL", b"LLRR", b"RLR", b"LRRRRRLLR", b"RRLLLRLLLRRR", b"LLRRRLRLRLLR"];
const MAX_ANTS: usize = 4;
const MAX_STEPS_PER_FRAME: f32 = 4000.0;
/// Seconds before the board restarts with the next rule.
const LIFETIME: f32 = 30.0;

struct Ant {
    x: usize,
    y: usize,
    dir: u8,
}

/// Langton's ants and their multi-color cousins (turmites) wandering a
/// wrapping board. Loudness sets the step rate, beats add more ants,
/// and freshly flipped cells glow. The board restarts with a new rule when it
/// fills up or after a while.
pub struct Langton {
    cells: Vec<u8>,
    /// Frame number of the last visit, for the glow.
    seen: Vec<u32>,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    ants: Vec<Ant>,
    rule: usize,
    rng: Rng,
    acc: f32,
    age: f32,
    frame: u32,
    hue: f32,
}

impl Langton {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            seen: Vec::new(),
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            ants: Vec::new(),
            rule: 0,
            rng: Rng::new(0xA17),
            acc: 0.0,
            age: 0.0,
            frame: 0,
            hue: 0.55,
        }
    }

    fn restart(&mut self) {
        self.cells = vec![0; self.gw * self.gh];
        self.seen = vec![0; self.gw * self.gh];
        self.ants.clear();
        self.age = 0.0;
        self.acc = 0.0;
        self.hue = self.rng.f32();
        if self.gw > 0 && self.gh > 0 {
            self.ants.push(Ant {
                x: self.gw / 2,
                y: self.gh / 2,
                dir: 0,
            });
        }
    }

    fn add_ant(&mut self) {
        if self.gw == 0 || self.gh == 0 || self.ants.len() >= MAX_ANTS {
            return;
        }
        let (x, y) = (
            self.rng.next_u32() as usize % self.gw,
            self.rng.next_u32() as usize % self.gh,
        );
        let dir = (self.rng.next_u32() % 4) as u8;
        self.ants.push(Ant { x, y, dir });
    }

    fn step(&mut self) {
        let rule = RULES[self.rule];
        let n = rule.len() as u8;
        let (gw, gh) = (self.gw, self.gh);
        for a in &mut self.ants {
            let i = a.y * gw + a.x;
            let st = self.cells[i] % n;
            a.dir = if rule[st as usize] == b'R' {
                (a.dir + 1) % 4
            } else {
                (a.dir + 3) % 4
            };
            self.cells[i] = (st + 1) % n;
            self.seen[i] = self.frame;
            match a.dir {
                0 => a.y = (a.y + gh - 1) % gh,
                1 => a.x = (a.x + 1) % gw,
                2 => a.y = (a.y + 1) % gh,
                _ => a.x = (a.x + gw - 1) % gw,
            }
        }
    }
}

impl Effect for Langton {
    fn name(&self) -> &'static str {
        "Langton's Ant"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.scale = if c.w >= 80 && c.h >= 40 { 2 } else { 1 };
            self.gw = c.w.div_ceil(self.scale);
            self.gh = c.h.div_ceil(self.scale);
            self.restart();
        }
        c.clear();
        if self.gw < 2 || self.gh < 2 {
            return;
        }
        self.frame = self.frame.wrapping_add(1).max(1);

        if f.beat {
            self.hue += 0.04;
            if self.rng.f32() < 0.3 {
                self.add_ant();
            }
        }
        let rate = 120.0 + f.level * 1400.0 + f.pulse * 400.0;
        self.acc = (self.acc + dt * rate).min(MAX_STEPS_PER_FRAME);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.step();
        }

        self.age += dt;
        let filled = self.cells.iter().filter(|&&s| s != 0).count();
        if self.age > LIFETIME || filled * 10 > self.cells.len() * 6 {
            self.rule = (self.rule + 1) % RULES.len();
            self.restart();
        }

        let n = RULES[self.rule].len();
        let palette: Vec<Rgb> = (0..n)
            .map(|k| {
                if k == 0 {
                    hsv(self.hue, 0.6, 0.05)
                } else {
                    hsv(self.hue + k as f32 / n as f32 * 0.7, 0.75, 0.55 + 0.25 * f.mid)
                }
            })
            .collect();
        let glow = hsv(self.hue + 0.5, 0.25, 1.0);

        let s = self.scale;
        for y in 0..c.h {
            let row = (y / s) * self.gw;
            for x in 0..c.w {
                let i = row + x / s;
                let state = self.cells[i] as usize % n;
                let since = self.frame.wrapping_sub(self.seen[i]);
                let col = if self.seen[i] != 0 && since < 24 {
                    mix(palette[state], glow, (24 - since) as f32 / 24.0 * 0.6)
                } else {
                    palette[state]
                };
                c.set(x, y, col);
            }
        }

        let ant_col = mix(hsv(self.hue + 0.5, 0.4, 1.0), [255, 255, 255], f.pulse);
        for a in &self.ants {
            for dy in 0..s {
                for dx in 0..s {
                    c.plot((a.x * s + dx) as i32, (a.y * s + dy) as i32, ant_col);
                }
            }
        }
    }
}
