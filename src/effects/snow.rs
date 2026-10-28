use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, mix, rgb, scale};

const FLAKES: usize = 900;
/// Resolution of the snow pile, independent of the canvas width.
const PILE_COLS: usize = 256;
/// The pile never grows above this fraction of the height.
const PILE_CAP: f32 = 0.16;

/// Positions are fractions of the canvas.
struct Flake {
    x: f32,
    y: f32,
    /// Parallax layer 0 (far) .. 2 (near).
    layer: u8,
    phase: f32,
}

/// Night snowfall over a pine forest with three parallax layers. Bass pushes
/// the wind, beats blow gusts, and the near flakes slowly pile up into a
/// drift along the bottom that settles and melts back a little.
pub struct Snow {
    flakes: Vec<Flake>,
    pile: Vec<f32>,
    /// Pines as (x, height, half width), fractions of the canvas.
    trees: Vec<(f32, f32, f32)>,
    wind: f32,
    gust: f32,
    time: f32,
    rng: Rng,
}

impl Snow {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x5_0700);
        let flakes = (0..FLAKES)
            .map(|i| Flake {
                x: rng.f32(),
                y: rng.f32(),
                layer: (i % 3) as u8,
                phase: rng.range(0.0, std::f32::consts::TAU),
            })
            .collect();
        let trees = (0..14)
            .map(|i| {
                let x = (i as f32 + rng.range(0.0, 0.8)) / 13.0 - 0.05;
                (x, rng.range(0.25, 0.5), rng.range(0.04, 0.07))
            })
            .collect();
        Self {
            flakes,
            pile: vec![0.02; PILE_COLS],
            trees,
            wind: 0.0,
            gust: 0.0,
            time: 0.0,
            rng,
        }
    }
}

impl Effect for Snow {
    fn name(&self) -> &'static str {
        "Snow"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        if f.beat {
            let dir = if self.rng.f32() < 0.7 { 1.0 } else { -1.0 };
            self.gust += dir * (0.4 + 0.6 * f.bass);
        }
        self.gust *= (-dt * 1.5).exp();
        let target = 0.04 + 0.35 * f.bass + 0.1 * (self.time * 0.13).sin() + self.gust;
        self.wind += (target - self.wind) * (1.0 - (-dt * 2.0).exp());

        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if c.w > 0 { h / w } else { 1.0 };
        let (t, wind) = (self.time, self.wind);
        for fl in &mut self.flakes {
            let depth = 0.4 + 0.3 * fl.layer as f32;
            let fall = 0.05 + 0.07 * fl.layer as f32;
            let sway = (t * 1.3 + fl.phase).sin() * 0.04;
            fl.y += fall * dt * (1.0 + 0.3 * f.level);
            fl.x = (fl.x + (wind * depth + sway) * dt * aspect).rem_euclid(1.0);
            let col = ((fl.x * PILE_COLS as f32) as usize).min(PILE_COLS - 1);
            let floor = if fl.layer == 0 { 0.9 } else { 1.0 - self.pile[col] };
            if fl.y >= floor {
                if fl.layer > 0 {
                    self.pile[col] = (self.pile[col] + 0.005 * fl.layer as f32).min(PILE_CAP);
                }
                fl.y = -self.rng.range(0.0, 0.1);
                fl.x = self.rng.f32();
            }
        }

        // Settle: let steep steps slide sideways, and melt a little.
        for i in 1..PILE_COLS {
            let d = self.pile[i] - self.pile[i - 1];
            if d.abs() > 0.004 {
                let m = d * 0.25;
                self.pile[i] -= m;
                self.pile[i - 1] += m;
            }
        }
        for p in &mut self.pile {
            *p = (*p - dt * 0.0008).max(0.02);
        }

        if c.w == 0 || c.h == 0 {
            return;
        }

        for y in 0..c.h {
            let g = y as f32 / h;
            let col = mix(rgb(0.01, 0.015, 0.05), rgb(0.12, 0.14, 0.22), g * g);
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }
        let draw_flakes = |c: &mut Canvas, flakes: &[Flake], layer: u8| {
            for fl in flakes.iter().filter(|fl| fl.layer == layer) {
                let (x, y) = ((fl.x * w) as i32, (fl.y * h) as i32);
                let b = 0.35 + 0.3 * layer as f32 + 0.15 * f.treble;
                let col = rgb(0.85 * b, 0.9 * b, b);
                c.plot_add(x, y, col);
                if layer == 2 {
                    let soft = scale(col, 0.35);
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        c.plot_add(x + dx, y + dy, soft);
                    }
                }
            }
        };
        draw_flakes(c, &self.flakes, 0);

        // Pines: stacked triangles with snowy edges.
        let tree_col = rgb(0.02, 0.04, 0.05);
        let snow_col = rgb(0.35, 0.38, 0.45);
        for &(tx, th, tw) in &self.trees {
            let base = h * 0.92;
            let top = base - th * h;
            let half = tw * w.min(h * 2.0);
            let x0 = ((tx * w - half).max(0.0) as usize).min(c.w);
            let x1 = ((tx * w + half).max(0.0) as usize + 1).min(c.w);
            for x in x0..x1 {
                let dx = (x as f32 - tx * w).abs() / half.max(0.5);
                for y in (top.max(0.0) as usize).min(c.h)..(base as usize).min(c.h) {
                    // Three tiers: width grows within each tier.
                    let v = (y as f32 - top) / (base - top).max(1.0);
                    let tier = (v * 3.0).fract();
                    let width = (0.1 + 0.9 * v) * (0.5 + 0.5 * tier);
                    if dx <= width {
                        let edge = tier < 0.25 && dx > width - 0.25;
                        c.set(x, y, if edge { snow_col } else { tree_col });
                    }
                }
            }
        }

        draw_flakes(c, &self.flakes, 1);

        for x in 0..c.w {
            let p = self.pile[(x * PILE_COLS / c.w).min(PILE_COLS - 1)];
            let top = h * (1.0 - p);
            for y in (top.max(0.0) as usize).min(c.h)..c.h {
                let d = (y as f32 - top) / (p * h).max(1.0);
                c.set(x, y, mix(rgb(0.85, 0.88, 0.95), rgb(0.35, 0.4, 0.55), d));
            }
        }

        draw_flakes(c, &self.flakes, 2);
    }
}
