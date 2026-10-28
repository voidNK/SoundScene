use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, rgb};

/// Number of palettes `fire_color` knows.
pub const PALETTES: usize = 6;

/// Maps heat 0..1 to a color. Palette 0 is the classic black → red →
/// yellow → white; the others permute which channel lights up first
/// (magenta, toxic green, violet, ice blue, emerald).
pub fn fire_color(heat: f32, palette: usize) -> Rgb {
    const ORDERS: [[f32; 3]; PALETTES] = [
        [0.0, 1.0, 2.0],
        [0.0, 2.0, 1.0],
        [1.0, 0.0, 2.0],
        [1.0, 2.0, 0.0],
        [2.0, 1.0, 0.0],
        [2.0, 0.0, 1.0],
    ];
    let o = ORDERS[palette % PALETTES];
    let v = heat * 3.0;
    rgb(v - o[0], v - o[1], v - o[2])
}

/// Old-school fire simulation: heat is fed along the last row and averaged
/// towards row 0. `len` runs along the burning edge, `depth` away from it.
pub struct FireField {
    heat: Vec<f32>,
    len: usize,
    depth: usize,
}

impl FireField {
    pub fn new() -> Self {
        Self {
            heat: Vec::new(),
            len: 0,
            depth: 0,
        }
    }

    pub fn resize(&mut self, len: usize, depth: usize) {
        if (self.len, self.depth) != (len, depth) {
            (self.len, self.depth) = (len, depth);
            self.heat = vec![0.0; len * depth];
        }
    }

    /// `fuel` scales the heat fed in, `reach` is roughly how far up the
    /// flames climb as a fraction of `depth`, `sparks` throws hot blobs.
    pub fn step(&mut self, rng: &mut Rng, fuel: f32, reach: f32, sparks: bool) {
        let (w, h) = (self.len, self.depth);
        if w == 0 || h < 3 {
            return;
        }
        for x in 0..w {
            self.heat[(h - 1) * w + x] = (rng.f32() * fuel * 1.6).min(1.0);
        }
        if sparks {
            for _ in 0..w / 6 {
                let x = rng.next_u32() as usize % w.saturating_sub(2).max(1);
                let y = h - 2 - rng.next_u32() as usize % (h / 4).max(1);
                for (dx, dy) in [(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)] {
                    let (px, py) = (x + dx, y + dy);
                    if px < w && py < h {
                        self.heat[py * w + px] = 1.0;
                    }
                }
            }
        }

        // Rows are processed top-down so each reads last frame's rows below.
        let cool = 1.0 / (h as f32 * reach.max(0.05));
        for y in 0..h - 1 {
            let below = (y + 1) * w;
            let below2 = if y + 2 < h { (y + 2) * w } else { below };
            for x in 0..w {
                let xl = x.saturating_sub(1);
                let xr = (x + 1).min(w - 1);
                let sum = self.heat[below + xl] + self.heat[below + x] + self.heat[below + xr] + self.heat[below2 + x];
                let v = sum * 0.25 - cool * (0.5 + rng.f32());
                self.heat[y * w + x] = v.max(0.0);
            }
        }
    }

    /// Heat at position `x` along the edge, row `y` (row `depth - 1` is the edge).
    #[inline]
    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.heat[y * self.len + x]
    }
}

/// Classic bottom-up fire. Bass feeds the flames, beats throw sparks,
/// loudness sets the flame height.
pub struct Fire {
    field: FireField,
    rng: Rng,
}

impl Fire {
    pub fn new() -> Self {
        Self {
            field: FireField::new(),
            rng: Rng::new(0xF1AE),
        }
    }
}

impl Effect for Fire {
    fn name(&self) -> &'static str {
        "Fire"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, _dt: f32, f: &Features) {
        self.field.resize(c.w, c.h);
        if c.w == 0 || c.h < 3 {
            return;
        }
        let fuel = 0.35 + f.bass * 0.9 + f.pulse * 0.5;
        let reach = 0.35 + 0.5 * f.level.max(f.bass);
        self.field.step(&mut self.rng, fuel, reach, f.beat);
        for y in 0..c.h {
            for x in 0..c.w {
                c.set(x, y, fire_color(self.field.at(x, y), 0));
            }
        }
    }
}
