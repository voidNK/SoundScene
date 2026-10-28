use super::Effect;
use super::fire::{FireField, PALETTES, fire_color};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, add};

/// Fire burning in from all four edges, each side in its own palette.
/// The palettes rotate every 8 beats.
pub struct Inferno {
    // bottom, top, left, right
    sides: [FireField; 4],
    rng: Rng,
    palette: usize,
    beats: u32,
}

impl Inferno {
    pub fn new() -> Self {
        Self {
            sides: [FireField::new(), FireField::new(), FireField::new(), FireField::new()],
            rng: Rng::new(0x1AFE),
            palette: 0,
            beats: 0,
        }
    }
}

impl Effect for Inferno {
    fn name(&self) -> &'static str {
        "Inferno"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, _dt: f32, f: &Features) {
        let (w, h) = (c.w, c.h);
        if w < 3 || h < 3 {
            return;
        }
        // Each field burns along its edge; left/right fields are rotated,
        // so their length is the screen height and their depth its width.
        self.sides[0].resize(w, h);
        self.sides[1].resize(w, h);
        self.sides[2].resize(h, w);
        self.sides[3].resize(h, w);

        let fuel = 0.3 + f.bass * 0.9 + f.pulse * 0.5;
        let reach = 0.18 + 0.3 * f.level.max(f.bass);
        for side in &mut self.sides {
            side.step(&mut self.rng, fuel, reach, f.beat);
        }
        if f.beat {
            self.beats += 1;
            if self.beats.is_multiple_of(8) {
                self.palette = (self.palette + 1) % PALETTES;
            }
        }

        let p = self.palette;
        for y in 0..h {
            for x in 0..w {
                let bottom = fire_color(self.sides[0].at(x, y), p);
                let top = fire_color(self.sides[1].at(x, h - 1 - y), p + 1);
                let left = fire_color(self.sides[2].at(y, w - 1 - x), p + 2);
                let right = fire_color(self.sides[3].at(y, x), p + 3);
                c.set(x, y, add(add(bottom, top), add(left, right)));
            }
        }
    }
}
