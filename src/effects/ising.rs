use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

/// Pixels per lattice site along each axis.
const CELL: usize = 2;
const MAX_SWEEPS: f32 = 3.0;

/// 2D Ising model with Metropolis updates: up and down spins form magnetic
/// domains that coarsen when cool. Loudness heats the lattice above the
/// critical temperature so the domains melt into noise, beats flip a
/// random patch, and the two colors blend softly at the domain walls.
pub struct Ising {
    spins: Vec<i8>,
    shown: Vec<f32>,
    colors: Vec<[f32; 3]>,
    gw: usize,
    gh: usize,
    rng: Rng,
    acc: f32,
    temp: f32,
    time: f32,
    hue: f32,
}

impl Ising {
    pub fn new() -> Self {
        Self {
            spins: Vec::new(),
            shown: Vec::new(),
            colors: Vec::new(),
            gw: 0,
            gh: 0,
            rng: Rng::new(0x1514),
            acc: 0.0,
            temp: 2.0,
            time: 0.0,
            hue: 0.0,
        }
    }

    /// One Metropolis sweep: as many random single-spin flip attempts as sites.
    fn sweep(&mut self, temp: f32) {
        let (gw, gh) = (self.gw, self.gh);
        // Energy change of a flip is 2·s·(neighbour sum) ∈ {-8, -4, 0, 4, 8}.
        let p4 = (-4.0 / temp).exp();
        let p8 = (-8.0 / temp).exp();
        for _ in 0..gw * gh {
            let r = self.rng.next_u32();
            let x = (r & 0xFFFF) as usize % gw;
            let y = (r >> 16) as usize % gh;
            let i = y * gw + x;
            let n = self.spins[y * gw + (x + 1) % gw] as i32
                + self.spins[y * gw + (x + gw - 1) % gw] as i32
                + self.spins[(y + 1) % gh * gw + x] as i32
                + self.spins[(y + gh - 1) % gh * gw + x] as i32;
            let de = 2 * self.spins[i] as i32 * n;
            let accept = match de {
                d if d <= 0 => true,
                4 => self.rng.f32() < p4,
                _ => self.rng.f32() < p8,
            };
            if accept {
                self.spins[i] = -self.spins[i];
            }
        }
    }
}

impl Effect for Ising {
    fn name(&self) -> &'static str {
        "Ising"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        if (gw, gh) != (self.gw, self.gh) {
            (self.gw, self.gh) = (gw, gh);
            self.spins = (0..gw * gh)
                .map(|_| if self.rng.f32() < 0.5 { 1 } else { -1 })
                .collect();
            self.shown = self.spins.iter().map(|&s| s as f32).collect();
            self.colors = vec![[0.0; 3]; gw * gh];
        }
        self.time += dt;
        self.hue += dt * 0.012;

        // Below the critical temperature (≈2.27) domains grow; loud parts melt them.
        let target = 1.6 + 0.3 * (self.time * 0.13).sin() + 1.5 * f.level * f.level + 0.8 * f.pulse;
        let rate = if target > self.temp { 8.0 } else { 1.5 };
        self.temp += (target - self.temp) * (1.0 - (-rate * dt).exp());

        if f.beat {
            self.hue += 0.04;
            // Flip a disc so new domains nucleate.
            let r = (gw.min(gh) as f32 * (0.08 + 0.12 * f.bass)).max(1.0);
            let (cx, cy) = (self.rng.f32() * gw as f32, self.rng.f32() * gh as f32);
            let ri = r.ceil() as i32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    if (dx * dx + dy * dy) as f32 <= r * r {
                        let x = (cx as i32 + dx).rem_euclid(gw as i32) as usize;
                        let y = (cy as i32 + dy).rem_euclid(gh as i32) as usize;
                        let i = y * gw + x;
                        self.spins[i] = -self.spins[i];
                    }
                }
            }
        }

        self.acc = (self.acc + dt * (25.0 + 30.0 * f.level)).min(MAX_SWEEPS);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.sweep(self.temp);
        }

        // Displayed magnetization eases towards the spins for soft walls.
        let ease = 1.0 - (-14.0 * dt).exp();
        let up = hsv(self.hue, 0.75, 0.95);
        let down = hsv(self.hue + 0.5, 0.8, 0.45);
        let up = up.map(|v| v as f32 / 255.0);
        let down = down.map(|v| v as f32 / 255.0);
        let glow = 0.8 + 0.4 * f.pulse;
        for (i, (s, col)) in self.shown.iter_mut().zip(self.colors.iter_mut()).enumerate() {
            *s += (self.spins[i] as f32 - *s) * ease;
            let k = (*s * 0.5 + 0.5).clamp(0.0, 1.0);
            // Domain walls (k near 0.5) glow slightly.
            let wall = 1.0 - (2.0 * k - 1.0).abs();
            for ch in 0..3 {
                col[ch] = (down[ch] + (up[ch] - down[ch]) * k) * glow + wall * 0.25;
            }
        }
        upscale(c, &self.colors, gw, gh);
    }
}
