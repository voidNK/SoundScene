use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, mix, rgb, scale};

const EMPTY: u8 = 0;
const TREE: u8 = 1;
const FIRE: u8 = 2;
const ASH: u8 = 3;
/// Steps a cell burns before it turns to ash.
const BURN_STEPS: u8 = 3;
const GROW: f32 = 0.02;
/// Seconds without any fire before lightning strikes on its own.
const CALM_LIMIT: f32 = 2.5;

/// Forest-fire automaton: trees regrow on bare ground, lightning ignites
/// them and fire fronts sweep through the forest, leaving glowing ash. Beats
/// call down lightning (more with more bass), loudness speeds up the
/// simulation and the flames flare with the bass.
pub struct ForestFire {
    cells: Vec<u8>,
    next: Vec<u8>,
    /// Tree age, burn time for fire, remaining glow for ash.
    heat: Vec<u8>,
    next_heat: Vec<u8>,
    colors: Vec<Rgb>,
    gw: usize,
    gh: usize,
    scale: usize,
    size: (usize, usize),
    rng: Rng,
    acc: f32,
    calm: f32,
}

impl ForestFire {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            next: Vec::new(),
            heat: Vec::new(),
            next_heat: Vec::new(),
            colors: Vec::new(),
            gw: 0,
            gh: 0,
            scale: 1,
            size: (0, 0),
            rng: Rng::new(0xF0F),
            acc: 0.0,
            calm: 0.0,
        }
    }

    fn plant(&mut self) {
        for (v, h) in self.cells.iter_mut().zip(&mut self.heat) {
            *v = if self.rng.f32() < 0.55 { TREE } else { EMPTY };
            *h = (self.rng.next_u32() % 60) as u8;
        }
    }

    /// Ignites up to `n` random trees.
    fn strike(&mut self, n: usize) {
        let len = self.cells.len();
        for _ in 0..n {
            for _ in 0..60 {
                let i = self.rng.next_u32() as usize % len;
                if self.cells[i] == TREE {
                    self.cells[i] = FIRE;
                    self.heat[i] = 0;
                    break;
                }
            }
        }
    }

    /// Returns the number of burning cells after the step.
    fn step(&mut self) -> usize {
        let (w, h) = (self.gw, self.gh);
        let mut burning = 0;
        for y in 0..h {
            let (up, down) = (((y + h - 1) % h) * w, ((y + 1) % h) * w);
            let row = y * w;
            for x in 0..w {
                let i = row + x;
                let heat = self.heat[i];
                let (s, hv) = match self.cells[i] {
                    EMPTY => {
                        if self.rng.f32() < GROW {
                            (TREE, 0)
                        } else {
                            (EMPTY, 0)
                        }
                    }
                    TREE => {
                        let (l, r) = ((x + w - 1) % w, (x + 1) % w);
                        let cells = &self.cells;
                        let fire = |j: usize| (cells[j] == FIRE) as u8;
                        let side = fire(up + x) + fire(down + x) + fire(row + l) + fire(row + r);
                        let diag = fire(up + l) + fire(up + r) + fire(down + l) + fire(down + r);
                        let p = self.rng.f32();
                        if (side > 0 && p < 0.92) || (diag > 0 && p < 0.55) {
                            (FIRE, 0)
                        } else {
                            (TREE, heat.saturating_add(1))
                        }
                    }
                    FIRE => {
                        if heat + 1 >= BURN_STEPS {
                            (ASH, 255)
                        } else {
                            burning += 1;
                            (FIRE, heat + 1)
                        }
                    }
                    _ => {
                        let left = heat.saturating_sub(20);
                        if left == 0 { (EMPTY, 0) } else { (ASH, left) }
                    }
                };
                self.next[i] = s;
                self.next_heat[i] = hv;
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        std::mem::swap(&mut self.heat, &mut self.next_heat);
        burning
    }
}

impl Effect for ForestFire {
    fn name(&self) -> &'static str {
        "Forest Fire"
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
            self.heat = vec![0; n];
            self.next_heat = vec![0; n];
            self.colors = vec![[0; 3]; n];
            self.plant();
        }
        if self.gw < 3 || self.gh < 3 {
            c.clear();
            return;
        }

        if f.beat {
            self.strike(1 + (f.bass * 3.0) as usize);
        }
        self.calm += dt;
        if self.calm > CALM_LIMIT {
            self.strike(1);
            self.calm = 0.0;
        }
        self.acc = (self.acc + dt * (12.0 + f.level * 20.0 + f.bass * 10.0)).min(3.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            if self.step() > 0 {
                self.calm = 0.0;
            }
        }

        let soil = rgb(0.07, 0.045, 0.025);
        let young = rgb(0.06, 0.22, 0.05);
        let old = rgb(0.12, 0.5, 0.1);
        let flare = 0.75 + 0.25 * f.bass + 0.2 * f.pulse;
        let flames: [Rgb; 3] = [
            scale(rgb(1.0, 0.95, 0.6), flare),
            scale(rgb(1.0, 0.55, 0.1), flare),
            scale(rgb(0.8, 0.18, 0.03), flare),
        ];
        let ash = rgb(0.32, 0.3, 0.28);
        let ember = rgb(0.7, 0.12, 0.02);
        // Shade by how many of the four neighbors are trees too, so dense
        // forest reads as canopy and sparse regrowth as undergrowth.
        let (w, h) = (self.gw, self.gh);
        for y in 0..h {
            let (up, down) = (((y + h - 1) % h) * w, ((y + 1) % h) * w);
            let row = y * w;
            for x in 0..w {
                let i = row + x;
                let (l, r) = ((x + w - 1) % w, (x + 1) % w);
                let cells = &self.cells;
                let tree = |j: usize| (cells[j] == TREE) as u8;
                let dense = (tree(up + x) + tree(down + x) + tree(row + l) + tree(row + r)) as f32 / 4.0;
                let heat = self.heat[i];
                self.colors[i] = match self.cells[i] {
                    TREE => scale(mix(young, old, heat as f32 / 40.0), 0.6 + 0.4 * dense),
                    FIRE => flames[(heat as usize).min(2)],
                    ASH => {
                        let a = heat as f32 / 255.0;
                        let base = mix(soil, ash, a);
                        if a > 0.85 {
                            mix(base, ember, (a - 0.85) * 6.0)
                        } else {
                            base
                        }
                    }
                    _ => mix(soil, young, dense * 0.5),
                };
            }
        }
        let s = self.scale;
        for y in 0..c.h {
            let row = (y / s) * w;
            for x in 0..c.w {
                c.set(x, y, self.colors[row + x / s]);
            }
        }
    }
}
