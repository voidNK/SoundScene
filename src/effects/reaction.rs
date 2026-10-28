use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, rgb};

/// Pixels per simulation cell along each axis.
const CELL: usize = 2;
const MAX_STEPS: usize = 24;
/// Gray-Scott (feed, kill) pairs: coral, spots, maze, mitosis.
const PRESETS: [(f32, f32); 4] = [(0.0545, 0.062), (0.035, 0.065), (0.029, 0.057), (0.0367, 0.0649)];

/// Gray-Scott reaction-diffusion growing coral, spot and maze patterns.
/// Bass speeds up the chemistry and nudges the feed rate, beats seed new
/// spots, and the feed/kill parameters drift slowly through several regimes.
pub struct ReactionDiffusion {
    u: Vec<f32>,
    v: Vec<f32>,
    nu: Vec<f32>,
    nv: Vec<f32>,
    colors: Vec<[f32; 3]>,
    gw: usize,
    gh: usize,
    rng: Rng,
    acc: f32,
    phase: f32,
    hue: f32,
}

impl ReactionDiffusion {
    pub fn new() -> Self {
        Self {
            u: Vec::new(),
            v: Vec::new(),
            nu: Vec::new(),
            nv: Vec::new(),
            colors: Vec::new(),
            gw: 0,
            gh: 0,
            rng: Rng::new(0x6A5C),
            acc: 0.0,
            phase: 0.0,
            hue: 0.0,
        }
    }

    /// Drops a square of chemical V around a random cell.
    fn seed_spot(&mut self, radius: usize) {
        let (gw, gh) = (self.gw, self.gh);
        let cx = self.rng.next_u32() as usize % gw;
        let cy = self.rng.next_u32() as usize % gh;
        for dy in 0..=2 * radius {
            for dx in 0..=2 * radius {
                let x = (cx + dx + gw - radius % gw) % gw;
                let y = (cy + dy + gh - radius % gh) % gh;
                let i = y * gw + x;
                self.u[i] = 0.5;
                self.v[i] = 0.25 + 0.1 * self.rng.f32();
            }
        }
    }

    fn reset(&mut self) {
        self.u.fill(1.0);
        self.v.fill(0.0);
        let spots = (self.gw * self.gh / 120).clamp(1, 60);
        for _ in 0..spots {
            self.seed_spot(1);
        }
    }

    fn step(&mut self, feed: f32, kill: f32) {
        let (gw, gh) = (self.gw, self.gh);
        for y in 0..gh {
            let up = (y + gh - 1) % gh * gw;
            let down = (y + 1) % gh * gw;
            let row = y * gw;
            for x in 0..gw {
                let l = (x + gw - 1) % gw;
                let r = (x + 1) % gw;
                let i = row + x;
                // 3×3 Laplacian: 0.2 for edges, 0.05 for corners.
                let lap = |a: &[f32]| {
                    (a[row + l] + a[row + r] + a[up + x] + a[down + x]) * 0.2
                        + (a[up + l] + a[up + r] + a[down + l] + a[down + r]) * 0.05
                        - a[i]
                };
                let (u, v) = (self.u[i], self.v[i]);
                let uvv = u * v * v;
                self.nu[i] = (u + lap(&self.u) - uvv + feed * (1.0 - u)).clamp(0.0, 1.0);
                self.nv[i] = (v + 0.5 * lap(&self.v) + uvv - (kill + feed) * v).clamp(0.0, 1.0);
            }
        }
        std::mem::swap(&mut self.u, &mut self.nu);
        std::mem::swap(&mut self.v, &mut self.nv);
    }
}

impl Effect for ReactionDiffusion {
    fn name(&self) -> &'static str {
        "Reaction Diffusion"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        if (gw, gh) != (self.gw, self.gh) {
            (self.gw, self.gh) = (gw, gh);
            let n = gw * gh;
            self.u = vec![1.0; n];
            self.v = vec![0.0; n];
            self.nu = vec![0.0; n];
            self.nv = vec![0.0; n];
            self.colors = vec![[0.0; 3]; n];
            self.reset();
        }

        self.phase += dt * 0.025;
        self.hue += dt * 0.01;
        if f.beat {
            self.hue += 0.04;
            for _ in 0..1 + (f.bass * 3.0) as usize {
                let r = 1 + self.rng.next_u32() as usize % 2;
                self.seed_spot(r);
            }
        }

        // Smoothly walk around the preset ring.
        let p = self.phase.rem_euclid(PRESETS.len() as f32);
        let i0 = (p as usize).min(PRESETS.len() - 1);
        let i1 = (i0 + 1) % PRESETS.len();
        let k = p - i0 as f32;
        let k = k * k * (3.0 - 2.0 * k);
        let feed = PRESETS[i0].0 + (PRESETS[i1].0 - PRESETS[i0].0) * k + 0.003 * f.bass;
        let kill = PRESETS[i0].1 + (PRESETS[i1].1 - PRESETS[i0].1) * k;

        self.acc = (self.acc + dt * (300.0 + 450.0 * f.bass)).min(MAX_STEPS as f32);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.step(feed, kill);
        }
        if self.v.iter().sum::<f32>() < 0.5 {
            self.reset();
        }

        // Colormap: dark background, hue-tinted body, bright rims lit from the top left.
        let bright = 0.8 + 0.3 * f.pulse + 0.2 * f.level;
        for y in 0..gh {
            let up = (y + gh - 1) % gh * gw;
            let down = (y + 1) % gh * gw;
            for x in 0..gw {
                let i = y * gw + x;
                let v = (self.v[i] * 3.2).min(1.0);
                let l = (x + gw - 1) % gw;
                let r = (x + 1) % gw;
                let light = (self.v[y * gw + l] - self.v[y * gw + r] + self.v[up + x] - self.v[down + x]) * 4.0;
                let hue = self.hue + 0.15 * v;
                let base = hsv(hue, 0.85 - 0.4 * v * v, v.powf(0.7) * bright);
                let bg = 0.04 + 0.04 * f.level;
                self.colors[i] = [
                    base[0] as f32 / 255.0 + light.max(0.0) * 0.7 + bg * 0.3,
                    base[1] as f32 / 255.0 + light.max(0.0) * 0.6 + bg * 0.4,
                    base[2] as f32 / 255.0 + light.max(0.0) * 0.5 + bg,
                ];
            }
        }
        upscale(c, &self.colors, gw, gh);
    }
}

/// Maps output index `i` of `n` onto `m` source cells: (left, right, weight).
fn sample_pos(i: usize, n: usize, m: usize) -> (usize, usize, f32) {
    let p = ((i as f32 + 0.5) * m as f32 / n.max(1) as f32 - 0.5).clamp(0.0, m.saturating_sub(1) as f32);
    let i0 = (p as usize).min(m.saturating_sub(1));
    (i0, (i0 + 1).min(m.saturating_sub(1)), p - i0 as f32)
}

/// Bilinear upscale of a `gw`×`gh` float RGB buffer (0..1) onto the whole canvas.
pub(super) fn upscale(c: &mut Canvas, src: &[[f32; 3]], gw: usize, gh: usize) {
    if gw == 0 || gh == 0 || src.len() < gw * gh {
        return;
    }
    let cols: Vec<(usize, usize, f32)> = (0..c.w).map(|x| sample_pos(x, c.w, gw)).collect();
    for y in 0..c.h {
        let (y0, y1, fy) = sample_pos(y, c.h, gh);
        let (r0, r1) = (&src[y0 * gw..(y0 + 1) * gw], &src[y1 * gw..(y1 + 1) * gw]);
        for (x, &(x0, x1, fx)) in cols.iter().enumerate() {
            let mut out = [0.0; 3];
            for (ch, o) in out.iter_mut().enumerate() {
                let top = r0[x0][ch] + (r0[x1][ch] - r0[x0][ch]) * fx;
                let bot = r1[x0][ch] + (r1[x1][ch] - r1[x0][ch]) * fx;
                *o = top + (bot - top) * fy;
            }
            c.set(x, y, rgb(out[0], out[1], out[2]));
        }
    }
}
