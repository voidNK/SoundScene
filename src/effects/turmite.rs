use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, scale};

/// Turn by 0, 90° right, 180° or 90° left.
const N: u8 = 0;
const R: u8 = 1;
const U: u8 = 2;
const L: u8 = 3;

/// Two-state, two-color turmites: `TABLES[k][state][color]` is
/// (color to write, turn, next state). Each one draws differently: spirals,
/// diagonal highways, lattices and chaotic growth.
type Table = [[(u8, u8, u8); 2]; 2];
const TABLES: [Table; 7] = [
    // Fibonacci spiral.
    [[(1, L, 1), (1, L, 1)], [(1, R, 1), (0, N, 0)]],
    // Chaotic blob that sends out highways.
    [[(1, R, 0), (0, L, 0)], [(0, R, 0), (1, N, 1)]],
    // Pinwheel of striped arms.
    [[(0, R, 1), (1, U, 0)], [(1, L, 1), (0, N, 0)]],
    // Growing diamond lattice.
    [[(1, N, 1), (1, L, 0)], [(1, U, 0), (1, L, 1)]],
    // Noisy core with a cross of highways.
    [[(1, N, 0), (0, R, 1)], [(1, N, 1), (0, N, 0)]],
    // Dense chaotic growth.
    [[(1, L, 0), (0, N, 1)], [(0, R, 0), (1, L, 1)]],
    // Diagonal stripes.
    [[(1, R, 1), (0, N, 1)], [(1, L, 0), (0, U, 0)]],
];
const MAX_MITES: usize = 8;
const DX: [i32; 4] = [0, 1, 0, -1];
const DY: [i32; 4] = [-1, 0, 1, 0];
/// Minimum seconds between two beat-spawned turmites.
const SPAWN_GAP: f32 = 1.5;
/// Seconds before the board is wiped.
const LIFETIME: f32 = 40.0;

struct Mite {
    x: usize,
    y: usize,
    dir: u8,
    state: u8,
    table: usize,
    /// Owner slot written into the cells, indexes `hues`.
    slot: usize,
}

/// Several turmites with different rule tables share one wrapping board,
/// each painting in its own color and disturbing the others' patterns where
/// their paths cross. Loudness sets the step rate, beats release new
/// turmites (replacing the oldest once there are eight) and freshly painted
/// cells glow.
pub struct Turmite {
    /// Bit 0: color, bits 1..: owner slot + 1 (0 = never painted).
    cells: Vec<u8>,
    glow: Vec<f32>,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    mites: Vec<Mite>,
    hues: [f32; MAX_MITES],
    rng: Rng,
    acc: f32,
    age: f32,
    next_table: usize,
    next_slot: usize,
    cooldown: f32,
}

impl Turmite {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            glow: Vec::new(),
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            mites: Vec::new(),
            hues: [0.0; MAX_MITES],
            rng: Rng::new(0x7E3),
            acc: 0.0,
            age: 0.0,
            next_table: 0,
            next_slot: 0,
            cooldown: 0.0,
        }
    }

    fn restart(&mut self) {
        self.cells.fill(0);
        self.glow.fill(0.0);
        self.mites.clear();
        self.age = 0.0;
        for _ in 0..3 {
            self.spawn();
        }
    }

    fn spawn(&mut self) {
        // Slots are reused round-robin, so the oldest turmite makes way.
        let slot = self.next_slot;
        self.next_slot = (slot + 1) % MAX_MITES;
        self.mites.retain(|m| m.slot != slot);
        let table = self.next_table % TABLES.len();
        self.next_table += 1 + (self.rng.next_u32() % 3) as usize;
        // Golden-ratio hue steps keep consecutive turmites apart.
        let prev = self.hues[(slot + MAX_MITES - 1) % MAX_MITES];
        self.hues[slot] = (prev + 0.618 + 0.1 * self.rng.f32()).fract();
        self.mites.push(Mite {
            x: self.rng.next_u32() as usize % self.gw,
            y: self.rng.next_u32() as usize % self.gh,
            dir: (self.rng.next_u32() % 4) as u8,
            state: 0,
            table,
            slot,
        });
    }

    fn step(&mut self) {
        let (gw, gh) = (self.gw as i32, self.gh as i32);
        for m in &mut self.mites {
            let i = m.y * self.gw + m.x;
            let color = (self.cells[i] & 1) as usize;
            let (write, turn, next) = TABLES[m.table][m.state as usize & 1][color];
            self.cells[i] = write | ((m.slot as u8 + 1) << 1);
            self.glow[i] = 1.0;
            m.dir = (m.dir + turn) % 4;
            m.state = next;
            m.x = (m.x as i32 + DX[m.dir as usize]).rem_euclid(gw) as usize;
            m.y = (m.y as i32 + DY[m.dir as usize]).rem_euclid(gh) as usize;
        }
    }
}

impl Effect for Turmite {
    fn name(&self) -> &'static str {
        "Turmites"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.scale = if c.w >= 160 && c.h >= 80 { 2 } else { 1 };
            self.gw = c.w.div_ceil(self.scale);
            self.gh = c.h.div_ceil(self.scale);
            let n = self.gw * self.gh;
            self.cells = vec![0; n];
            self.glow = vec![0.0; n];
            if n > 0 {
                self.restart();
            }
        }
        if self.gw == 0 || self.gh == 0 {
            return;
        }

        self.age += dt;
        if self.age > LIFETIME {
            self.restart();
        }
        self.cooldown -= dt;
        if f.beat && self.cooldown <= 0.0 {
            self.spawn();
            self.cooldown = SPAWN_GAP;
        }
        let decay = (-dt * 3.0).exp();
        for g in &mut self.glow {
            *g *= decay;
        }
        // Step rate grows with the board so big screens fill as fast as tiles.
        let base = (self.gw * self.gh) as f32 / 40.0;
        self.acc = (self.acc + dt * base * (1.0 + 3.0 * f.level + 2.0 * f.bass)).min(4000.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.step();
        }

        let bright = 0.8 + 0.2 * f.pulse;
        let s = self.scale;
        for y in 0..c.h {
            let row = (y / s) * self.gw;
            for x in 0..c.w {
                let i = row + x / s;
                let v = self.cells[i];
                let col = if v == 0 {
                    [0, 0, 0]
                } else {
                    let hue = self.hues[((v >> 1) as usize - 1) % MAX_MITES];
                    let base = if v & 1 == 1 {
                        hsv(hue, 0.75, 0.8 * bright)
                    } else {
                        hsv(hue, 0.7, 0.15)
                    };
                    mix(base, [255, 255, 255], self.glow[i] * 0.7)
                };
                c.set(x, y, col);
            }
        }
        for m in &self.mites {
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (x, y) = (m.x * s + dx, m.y * s + dy);
                if dx < s && dy < s && x < c.w && y < c.h {
                    c.set(x, y, scale([255, 255, 255], bright));
                }
            }
        }
    }
}
