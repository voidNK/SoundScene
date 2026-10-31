use super::Effect;
use super::polygons::stroke;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, scale};
use std::f32::consts::SQRT_2;

const MAX_LEVELS: usize = 13;
/// Seconds the finished tree must stand before a beat may regrow it.
const HOLD: f32 = 2.5;

/// Recursive H-tree fractal growing outwards level by level, faster with
/// bass. Each level is lit by its own spectrum band (bass at the trunk,
/// treble at the leaves), the pulse runs down the branches, and a beat after
/// the tree is complete knocks it back to regrow in a new color.
pub struct HTree {
    grow: f32,
    hold: f32,
    hue: f32,
    flash: f32,
}

struct Style<'a> {
    levels: usize,
    grow: f32,
    light: &'a [f32; MAX_LEVELS],
    hue: f32,
    width0: f32,
}

impl HTree {
    pub fn new() -> Self {
        Self {
            grow: 0.0,
            hold: 0.0,
            hue: 0.3,
            flash: 0.0,
        }
    }
}

/// Draws the segment of `level` centered at (x, y) and recurses into both ends.
fn branch(c: &mut Canvas, s: &Style, x: f32, y: f32, len: f32, level: usize, horizontal: bool) {
    let g = (s.grow - level as f32).clamp(0.0, 1.0);
    if level >= s.levels || g <= 0.0 {
        return;
    }
    let half = len * 0.5 * g;
    let (dx, dy) = if horizontal { (half, 0.0) } else { (0.0, half) };
    let depth = level as f32 / s.levels.max(1) as f32;
    let col: Rgb = hsv(s.hue + depth * 0.45, 0.8 - 0.3 * s.light[level], s.light[level]);
    let width = (s.width0 * 0.78f32.powi(level as i32)).max(0.7);
    stroke(c, x - dx, y - dy, x + dx, y + dy, width * 2.5, scale(col, 0.25));
    stroke(c, x - dx, y - dy, x + dx, y + dy, width, col);
    if g >= 1.0 {
        let next = len / SQRT_2;
        branch(c, s, x - dx, y - dy, next, level + 1, !horizontal);
        branch(c, s, x + dx, y + dy, next, level + 1, !horizontal);
    } else {
        // Bright growing tips.
        let tip = hsv(s.hue + depth * 0.45, 0.2, 1.0);
        c.plot_add((x - dx) as i32, (y - dy) as i32, tip);
        c.plot_add((x + dx) as i32, (y + dy) as i32, tip);
    }
}

impl Effect for HTree {
    fn name(&self) -> &'static str {
        "H-Tree"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fill([2, 2, 6]);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (w, h) = (c.w as f32, c.h as f32);
        // Extent of the full tree: 2·len0 wide, √2·len0 high.
        let len0 = (w * 0.94 / 2.0).min(h * 0.94 / SQRT_2);
        let mut levels = 0;
        while levels < MAX_LEVELS && len0 / SQRT_2.powi(levels as i32) >= 2.0 {
            levels += 1;
        }
        let levels = levels.max(1);

        self.grow += dt * (1.3 + 3.0 * f.bass);
        if self.grow >= levels as f32 {
            self.grow = self.grow.min(levels as f32);
            self.hold += dt;
            if f.beat && self.hold > HOLD {
                self.grow = 0.0;
                self.hold = 0.0;
                self.hue += 0.23;
                self.flash = 1.0;
            }
        }
        self.flash *= (-dt * 3.0).exp();

        let bins = f.spectrum.len();
        let mut light = [0.0f32; MAX_LEVELS];
        for (i, l) in light.iter_mut().enumerate().take(levels) {
            let depth = i as f32 / levels as f32;
            let band = if bins > 0 {
                f.spectrum[((depth * bins as f32) as usize).min(bins - 1)]
            } else {
                0.0
            };
            // The pulse travels from the trunk to the leaves.
            let wave = f.pulse * (1.0 - ((t * 3.0).fract() - depth).abs() * 3.0).max(0.0);
            *l = (0.35 + 0.5 * band + 0.4 * wave + 0.3 * self.flash).min(1.0);
        }
        let style = Style {
            levels,
            grow: self.grow,
            light: &light,
            hue: self.hue + 0.05 * (t * 0.2).sin(),
            width0: (len0 / 28.0).max(1.0),
        };
        branch(c, &style, w * 0.5, h * 0.5, len0, 0, true);
    }
}
