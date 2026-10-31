use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, scale};

/// The bright eight-color palette of the early home computers.
const PAL: [Rgb; 8] = [
    [0, 0, 0],
    [0, 0, 215],
    [215, 0, 0],
    [215, 0, 215],
    [0, 215, 0],
    [0, 215, 215],
    [215, 215, 0],
    [215, 215, 215],
];
const BLUE: usize = 1;
const RED: usize = 2;
const MAGENTA: usize = 3;
const GREEN: usize = 4;
const CYAN: usize = 5;
const YELLOW: usize = 6;
const WHITE: usize = 7;

/// Ink and paper per picture region: sky, sun, hills, ground.
const SCHEMES: [[(usize, usize); 4]; 3] = [
    [(CYAN, BLUE), (YELLOW, RED), (WHITE, MAGENTA), (GREEN, 0)],
    [(MAGENTA, BLUE), (YELLOW, MAGENTA), (CYAN, BLUE), (CYAN, 0)],
    [(WHITE, 0), (WHITE, BLUE), (BLUE, 0), (RED, 0)],
];
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
/// Length of the pilot tone before each picture, in progress units.
const PILOT: f32 = 0.35;
const HORIZON: f32 = 0.64;

/// A procedural landscape: dithered bitmap plus one ink/paper pair per cell.
struct Picture {
    sun_x: f32,
    ph: [f32; 2],
    scheme: usize,
}

impl Picture {
    fn new(seed: u32) -> Self {
        let mut r = Rng::new(seed.wrapping_mul(0x9E37_79B9) | 1);
        Self {
            sun_x: r.range(0.25, 0.75),
            ph: [r.range(0.0, 6.3), r.range(0.0, 6.3)],
            scheme: seed as usize % SCHEMES.len(),
        }
    }

    /// Region index (into the scheme) and brightness at normalized (u, v).
    fn sample(&self, u: f32, v: f32, aspect: f32) -> (usize, f32) {
        if v > HORIZON {
            // Perspective grid on the ground.
            let z = 1.0 / (v - HORIZON + 0.03);
            let gx = ((u - 0.5) * z * 0.6).rem_euclid(1.0);
            let gz = (z * 0.25).rem_euclid(1.0);
            let line = gx < 0.06 * z.sqrt() || gz < 0.12;
            return (3, if line { 1.0 } else { 0.08 });
        }
        let ridge = HORIZON - 0.1 - 0.07 * (u * 7.0 + self.ph[0]).sin() - 0.04 * (u * 19.0 + self.ph[1]).sin();
        if v > ridge {
            let slope = (u * 7.0 + self.ph[0]).cos() * 0.49 + (u * 19.0 + self.ph[1]).cos() * 0.76;
            return (2, if slope > 0.0 { 0.7 } else { 0.25 });
        }
        let (dx, dy) = ((u - self.sun_x) * aspect, v - 0.36);
        if dx * dx + dy * dy < 0.2 * 0.2 {
            // Horizontal cuts through the lower half of the sun.
            let cut = dy > 0.0 && (dy * 32.0).rem_euclid(1.0) < dy * 4.0;
            return (1, if cut { 0.0 } else { 1.0 });
        }
        (0, (v / HORIZON).powi(2) * 0.65)
    }
}

/// An 8-bit tape-loading screen: raster stripes race through the border
/// while the picture fills in, first as interleaved bitmap lines, then cell
/// by cell in color. Level speeds up the loading and packs the stripes
/// tighter, bass widens them, and every beat flashes the border in new colors.
pub struct Loader {
    rng: Rng,
    /// -PILOT..0 pilot tone, 0..1 bitmap lines, 1..2 color cells, then hold.
    progress: f32,
    hold: f32,
    seed: u32,
    picture: Picture,
    /// Cached bitmap bits and per-cell attributes for the current size.
    key: (usize, usize, usize, u32),
    bits: Vec<bool>,
    attrs: Vec<(usize, usize)>,
    stripes: Vec<(usize, usize)>,
    restripe: f32,
    flash: f32,
    flash_pair: (usize, usize),
}

impl Loader {
    pub fn new() -> Self {
        Self {
            rng: Rng::new(0x10AD),
            progress: -PILOT,
            hold: 0.0,
            seed: 0,
            picture: Picture::new(0),
            key: (0, 0, 0, u32::MAX),
            bits: Vec::new(),
            attrs: Vec::new(),
            stripes: Vec::new(),
            restripe: 0.0,
            flash: 0.0,
            flash_pair: (RED, CYAN),
        }
    }

    fn rebuild(&mut self, cw: usize, ch: usize, cs: usize) {
        let key = (cw, ch, cs, self.seed);
        if self.key == key {
            return;
        }
        self.key = key;
        let aspect = cw as f32 / ch.max(1) as f32;
        let p = &self.picture;
        self.bits = (0..cw * ch)
            .map(|i| {
                let (x, y) = (i % cw, i / cw);
                let (_, lum) = p.sample(x as f32 / cw as f32, y as f32 / ch as f32, aspect);
                lum * 16.0 > BAYER[y % 4][x % 4] as f32 + 0.5
            })
            .collect();
        let (gw, gh) = (cw.div_ceil(cs), ch.div_ceil(cs));
        self.attrs = (0..gw * gh)
            .map(|i| {
                let u = ((i % gw) as f32 + 0.5) * cs as f32 / cw as f32;
                let v = ((i / gw) as f32 + 0.5) * cs as f32 / ch as f32;
                SCHEMES[p.scheme][p.sample(u, v, aspect).0]
            })
            .collect();
    }

    /// Fresh random stripe heights for the data phase.
    fn restripe(&mut self, h: usize, level: f32, bass: f32) {
        self.stripes.clear();
        let unit = (h as f32 / 60.0).max(1.0);
        let spread = 1.0 + 3.0 * (1.0 - level) + 4.0 * bass;
        let mut y = 0;
        let mut k = 0;
        while y < h {
            let len = ((1.0 + self.rng.f32() * spread) * unit) as usize;
            self.stripes.push((len.max(1), k & 1));
            y += len.max(1);
            k += 1;
        }
    }
}

impl Effect for Loader {
    fn name(&self) -> &'static str {
        "Tape Loader"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if self.progress < 2.0 {
            let rate = 0.1 + 0.35 * f.level + 0.15 * f.bass;
            self.progress = (self.progress + dt * rate).min(2.0);
        } else {
            self.hold += dt;
            if self.hold > 2.5 {
                self.hold = 0.0;
                self.progress = -PILOT;
                self.seed = self.seed.wrapping_add(1);
                self.picture = Picture::new(self.seed);
            }
        }
        self.flash = (self.flash - dt * 5.0).max(0.0);
        if f.beat {
            self.flash = 1.0;
            let a = 1 + self.rng.next_u32() as usize % 7;
            let b = 1 + (a + 1 + self.rng.next_u32() as usize % 5) % 7;
            self.flash_pair = (a, b);
        }
        self.restripe -= dt;
        let total: usize = self.stripes.iter().map(|s| s.0).sum();
        if self.restripe <= 0.0 || total < c.h {
            self.restripe(c.h, f.level, f.bass);
            self.restripe = 1.0 / 25.0;
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        // Border color of every pixel row.
        let pilot = self.progress < 0.0;
        let holding = self.progress >= 2.0;
        let pair = if self.flash > 0.4 {
            self.flash_pair
        } else if pilot {
            (RED, CYAN)
        } else {
            (BLUE, YELLOW)
        };
        let mut rows: Vec<Rgb> = Vec::with_capacity(c.h);
        if pilot || (holding && self.flash <= 0.4) {
            let band = (c.h as f32 / 9.0).max(2.0);
            let calm = SCHEMES[self.picture.scheme][0].1;
            for y in 0..c.h {
                let k = ((y as f32 + t * band * 6.0) / band) as usize & 1;
                rows.push(if holding {
                    scale(PAL[calm.max(BLUE)], 0.55)
                } else {
                    PAL[if k == 0 { pair.0 } else { pair.1 }]
                });
            }
        } else {
            for &(len, k) in &self.stripes {
                let col = PAL[if k == 0 { pair.0 } else { pair.1 }];
                rows.extend(std::iter::repeat_n(col, len));
            }
        }

        let bx = ((c.w as f32 * 0.1).round() as usize).max(1).min(c.w / 2);
        let by = ((c.h as f32 * 0.12).round() as usize).max(1).min(c.h / 2);
        let (cw, ch) = (c.w - 2 * bx, c.h - 2 * by);
        if cw == 0 || ch == 0 {
            for (y, &col) in rows.iter().enumerate().take(c.h) {
                c.px[y * c.w..(y + 1) * c.w].fill(col);
            }
            return;
        }
        let cs = ((cw as f32 / 24.0).min(ch as f32 / 16.0).round() as usize).max(2);
        self.rebuild(cw, ch, cs);
        let gw = cw.div_ceil(cs);

        // Lines load in the interleaved order of the real thing: by screen
        // third, then pixel row inside the cell, then cell row.
        let cell_rows = ch.div_ceil(cs);
        let per_third = cell_rows.div_ceil(3).max(1);
        let lines = (3 * cs * per_third) as f32;
        let loaded = self.progress.clamp(0.0, 1.0) * lines;
        let cells = (self.progress - 1.0).clamp(0.0, 1.0) * self.attrs.len() as f32;

        for (y, &border) in rows.iter().enumerate().take(c.h) {
            if y < by || y >= by + ch {
                c.px[y * c.w..(y + 1) * c.w].fill(border);
                continue;
            }
            for x in (0..bx).chain(bx + cw..c.w) {
                c.set(x, y, border);
            }
            let ly = y - by;
            let r = ly / cs;
            let order = ((r / per_third) * cs + ly % cs) * per_third + r % per_third;
            // The line being loaded right now fills from left to right.
            let fill = ((loaded - order as f32).clamp(0.0, 1.0) * cw as f32) as usize;
            for lx in 0..cw {
                let col = if lx >= fill {
                    PAL[0]
                } else {
                    let cell = r * gw + lx / cs;
                    let (ink, paper) = if (cell as f32) < cells {
                        self.attrs[cell]
                    } else {
                        (WHITE, 0)
                    };
                    PAL[if self.bits[ly * cw + lx] { ink } else { paper }]
                };
                c.set(bx + lx, y, col);
            }
        }
    }
}
