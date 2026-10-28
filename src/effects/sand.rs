use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, scale};

const STEPS_PER_SEC: f32 = 60.0;
const MAX_EMITTERS: usize = 6;

/// Falling sand: emitters wander along the top and pour colored grains (rate
/// by loudness, color by their spectrum band, bursts on beats) that pile up
/// in layers. Once the box is full, holes open in the floor and it drains.
pub struct Sand {
    /// 0 = empty, otherwise 0x01RRGGBB.
    cells: Vec<u32>,
    w: usize,
    h: usize,
    count: usize,
    rng: Rng,
    acc: f32,
    emit: [f32; MAX_EMITTERS],
    draining: Option<f32>,
    hue: f32,
    tick: usize,
}

impl Sand {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            w: 0,
            h: 0,
            count: 0,
            rng: Rng::new(0x5A4D),
            acc: 0.0,
            emit: [0.0; MAX_EMITTERS],
            draining: None,
            hue: 0.0,
            tick: 0,
        }
    }

    fn pack(c: Rgb) -> u32 {
        0x0100_0000 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32
    }

    fn unpack(v: u32) -> Rgb {
        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
    }

    /// Places a grain at (x, y) if free; returns false if blocked.
    fn drop_grain(&mut self, x: usize, y: usize, col: Rgb) -> bool {
        let i = y * self.w + x;
        if self.cells[i] != 0 {
            return false;
        }
        self.cells[i] = Self::pack(col);
        self.count += 1;
        true
    }

    fn step(&mut self, open: Option<(usize, usize)>) {
        let (w, h) = (self.w, self.h);
        self.tick += 1;
        if let Some((period, width)) = open {
            let row = (h - 1) * w;
            for x in 0..w {
                if x % period < width && self.cells[row + x] != 0 {
                    self.cells[row + x] = 0;
                    self.count -= 1;
                }
            }
        }
        for y in (0..h - 1).rev() {
            // Alternate the scan direction so piles don't lean to one side.
            let flip = (y + self.tick).is_multiple_of(2);
            for k in 0..w {
                let x = if flip { w - 1 - k } else { k };
                let i = y * w + x;
                let g = self.cells[i];
                if g == 0 {
                    continue;
                }
                let below = i + w;
                let target = if self.cells[below] == 0 {
                    Some(below)
                } else {
                    let first_left = self.rng.next_u32() & 1 == 0;
                    let left = (x > 0 && self.cells[below - 1] == 0).then(|| below - 1);
                    let right = (x + 1 < w && self.cells[below + 1] == 0).then(|| below + 1);
                    if first_left { left.or(right) } else { right.or(left) }
                };
                if let Some(t) = target {
                    self.cells[t] = g;
                    self.cells[i] = 0;
                }
            }
        }
    }
}

impl Effect for Sand {
    fn name(&self) -> &'static str {
        "Sand"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if (self.w, self.h) != (c.w, c.h) {
            (self.w, self.h) = (c.w, c.h);
            self.cells = vec![0; c.w * c.h];
            self.count = 0;
            self.draining = None;
        }
        let (w, h) = (self.w, self.h);
        if w == 0 || h < 2 {
            c.clear();
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.hue += dt * 0.03;
        if f.beat {
            self.hue += 0.06;
        }

        let n = (w / 40).clamp(1, MAX_EMITTERS);
        let bins = f.spectrum.len();
        if self.draining.is_none() {
            for e in 0..n {
                let slot = w as f32 / n as f32;
                let wobble = (t * (0.4 + 0.13 * e as f32) + e as f32 * 1.7).sin() * slot * 0.4;
                let x = ((e as f32 + 0.5) * slot + wobble).clamp(0.0, (w - 1) as f32) as usize;
                let band = if bins > 0 {
                    let (lo, hi) = (e * bins / n, ((e + 1) * bins / n).max(e * bins / n + 1));
                    f.spectrum[lo..hi.min(bins)].iter().sum::<f32>() / (hi.min(bins) - lo).max(1) as f32
                } else {
                    0.0
                };
                self.emit[e] += dt * (10.0 + 220.0 * f.level) * (0.3 + band);
                if f.beat {
                    self.emit[e] += 12.0 + 20.0 * f.bass;
                }
                let base = self.hue + e as f32 / n as f32 * 0.6;
                // A burst can't all fit in the top rows at once; the rest is simply lost.
                self.emit[e] = self.emit[e].min(40.0);
                while self.emit[e] >= 1.0 {
                    self.emit[e] -= 1.0;
                    let col = hsv(base + self.rng.range(-0.02, 0.02), 0.65, self.rng.range(0.7, 1.0));
                    for _ in 0..4 {
                        let gx = (x + self.rng.next_u32() as usize % 5).saturating_sub(2).min(w - 1);
                        let gy = self.rng.next_u32() as usize % 3 % h;
                        if self.drop_grain(gx, gy, col) {
                            break;
                        }
                    }
                }
            }
        }
        // Full: most of the box, or the pile has reached the emitters.
        let top = 4.min(h - 1) * w;
        let reached = self.cells[top..top + w].iter().filter(|&&g| g != 0).count() > w / 2;
        if reached || self.count > w * h * 3 / 5 {
            self.draining.get_or_insert(0.0);
        }

        self.acc = (self.acc + dt * STEPS_PER_SEC).min(4.0);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let open = self.draining.map(|age| {
                let period = (w / 4).max(2);
                (period, (1.0 + age * 8.0).min(period as f32) as usize)
            });
            self.step(open);
        }
        if let Some(age) = &mut self.draining {
            *age += dt;
            if self.count < w * h / 50 || *age > 20.0 {
                self.draining = None;
            }
        }

        for y in 0..h {
            let bg = scale([10, 8, 24], 1.0 - y as f32 / h as f32 * 0.6 + 0.4 * f.pulse);
            for x in 0..w {
                let g = self.cells[y * w + x];
                c.set(x, y, if g == 0 { bg } else { Self::unpack(g) });
            }
        }
    }
}
