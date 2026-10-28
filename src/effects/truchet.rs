use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, scale};
use std::f32::consts::FRAC_PI_2;

/// Tile states wrap around a fixed grid, so the pattern survives resizes.
const GRID: usize = 48;
const FLIP_TIME: f32 = 0.3;

#[derive(Clone, Copy)]
struct Tile {
    orient: u8,
    /// Time the current quarter turn started (may be in the future), or None.
    flip: Option<f32>,
}

/// Truchet tiles: quarter-circle arcs that join into endless winding tubes
/// over a two-colored maze. On beats, tiles rotate a quarter turn in random
/// patches, rings or diagonal sweeps, rerouting the paths. Bass scrolls the
/// field and thickens the tubes, a color gradient flows along them.
pub struct Truchet {
    tiles: Vec<Tile>,
    time: f32,
    scroll: f32,
    rng: Rng,
}

/// Distance between two grid indices on the wrapped grid, in tiles.
fn wrap_dist(a: usize, b: usize) -> usize {
    let d = (a + GRID - b % GRID) % GRID;
    d.min(GRID - d)
}

impl Truchet {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x7_2C4E);
        let tiles = (0..GRID * GRID)
            .map(|_| Tile {
                orient: (rng.next_u32() & 1) as u8,
                flip: None,
            })
            .collect();
        Self {
            tiles,
            time: 0.0,
            scroll: 0.0,
            rng,
        }
    }

    fn flip_pattern(&mut self, center: (usize, usize), strength: f32) {
        let pattern = self.rng.next_u32() % 3;
        let now = self.time;
        for j in 0..GRID {
            for i in 0..GRID {
                let tile = &mut self.tiles[j * GRID + i];
                if tile.flip.is_some() {
                    continue;
                }
                let di = wrap_dist(i, center.0) as f32;
                let dj = wrap_dist(j, center.1) as f32;
                let start = match pattern {
                    0 => (self.rng.f32() < 0.2 + 0.3 * strength).then_some(now),
                    1 => ((di * di + dj * dj).sqrt() as u32)
                        .is_multiple_of(2)
                        .then_some(now + (di * di + dj * dj).sqrt() * 0.04),
                    _ => ((i + j) % 3 == 0).then_some(now + (di + dj) * 0.025),
                };
                tile.flip = start;
            }
        }
    }
}

impl Effect for Truchet {
    fn name(&self) -> &'static str {
        "Truchet"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        self.scroll += dt * (0.15 + 0.5 * f.bass);
        let now = self.time;
        for tile in &mut self.tiles {
            if let Some(start) = tile.flip
                && now - start >= FLIP_TIME
            {
                tile.orient ^= 1;
                tile.flip = None;
            }
        }

        let size = (c.h as f32 / 8.0).max(3.0);
        let (ox, oy) = (self.scroll, self.scroll * 0.6);
        let center = (
            ((c.w as f32 * 0.5 / size + ox).max(0.0) as usize) % GRID,
            ((c.h as f32 * 0.5 / size + oy).max(0.0) as usize) % GRID,
        );
        if f.beat {
            self.flip_pattern(center, f.bass);
        }
        if self.rng.f32() < f.mid * dt * 6.0 {
            let k = self.rng.next_u32() as usize % self.tiles.len();
            self.tiles[k].flip.get_or_insert(now);
        }

        if c.w == 0 || c.h == 0 {
            return;
        }
        let width = 0.1 + 0.06 * f.bass + 0.03 * f.pulse;
        let hue = now * 0.03;
        let bins = f.spectrum.len();
        let bright = 0.6 + 0.4 * f.pulse;

        for y in 0..c.h {
            let gy = y as f32 / size + oy;
            let ty = gy.floor();
            let ly = gy - ty - 0.5;
            let j = (ty as i64).rem_euclid(GRID as i64) as usize;
            for x in 0..c.w {
                let gx = x as f32 / size + ox;
                let tx = gx.floor();
                let lx = gx - tx - 0.5;
                let i = (tx as i64).rem_euclid(GRID as i64) as usize;
                let tile = self.tiles[j * GRID + i];

                // Rotate the sample point back by the tile's current turn.
                let p = tile.flip.map_or(0.0, |s| ((now - s) / FLIP_TIME).clamp(0.0, 1.0));
                let p = p * p * (3.0 - 2.0 * p);
                let (u, v) = if p > 0.0 {
                    let (sn, cs) = (p * FRAC_PI_2).sin_cos();
                    (lx * cs + ly * sn, -lx * sn + ly * cs)
                } else {
                    (lx, ly)
                };
                let (ax, ay) = if tile.orient == 0 { (-0.5, -0.5) } else { (0.5, -0.5) };
                let da = ((u - ax).powi(2) + (v - ay).powi(2)).sqrt();
                let db = ((u + ax).powi(2) + (v + ay).powi(2)).sqrt();
                let inside = da < 0.5 || db < 0.5;
                let d = (da - 0.5).abs().min((db - 0.5).abs());

                let parity = (i + j + tile.orient as usize + inside as usize) % 2;
                let fill = hsv(hue + 0.55, 0.6, if parity == 0 { 0.13 } else { 0.05 });
                // Hide the parity swap at the end of a turn behind a neutral tone.
                let fill = mix(fill, hsv(hue + 0.55, 0.6, 0.09), (p * std::f32::consts::PI).sin());

                let col = if d < width {
                    let tube = 1.0 - (d / width).powi(2);
                    let ring = wrap_dist(i, center.0) + wrap_dist(j, center.1);
                    let band = if bins > 0 {
                        f.spectrum[(ring * 2).min(bins - 1)]
                    } else {
                        0.0
                    };
                    let flow = 0.5 + 0.5 * ((gx + gy) * 1.6 - now * 3.0).sin();
                    let h = hue + (gx + gy) * 0.02;
                    let v = tube * bright * (0.55 + 0.25 * flow + 0.3 * band);
                    mix(fill, hsv(h, 0.75 - 0.4 * tube * tube, v.min(1.0)), tube.sqrt())
                } else {
                    scale(fill, 1.0 - 0.4 * (1.0 - (d - width).min(0.2) / 0.2))
                };
                c.set(x, y, col);
            }
        }
    }
}
