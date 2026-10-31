use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

/// Pixels per sample along each axis on large canvases; small tiles run at full resolution.
const CELL: usize = 2;
const MAX_DROPS: usize = 8;
/// Seconds a drop keeps pushing the water outwards.
const DROP_LIFE: f32 = 4.0;

/// Integer lattice hash in 0..1.
pub(super) fn hash(x: i32, y: i32) -> f32 {
    let mut n = (x as u32).wrapping_mul(0x27D4_EB2D) ^ (y as u32).wrapping_mul(0x1656_67B1);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 13;
    (n & 0xFFFF) as f32 / 65535.0
}

/// Smooth 2D value noise in 0..1.
pub(super) fn noise(x: f32, y: f32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (tx, ty) = (x - xf, y - yf);
    let (tx, ty) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let (i, j) = (xf as i32, yf as i32);
    let a = hash(i, j);
    let b = hash(i.wrapping_add(1), j);
    let c = hash(i, j.wrapping_add(1));
    let d = hash(i.wrapping_add(1), j.wrapping_add(1));
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * ty
}

struct Drop {
    x: f32,
    y: f32,
    age: f32,
    push: f32,
}

/// Ink drops blooming in water: colored clouds spread and swirl in a slowly
/// churning current. Beats drop new ink, bass stirs the water harder.
pub struct Ink {
    dye: Vec<[f32; 3]>,
    next: Vec<[f32; 3]>,
    gw: usize,
    gh: usize,
    drops: Vec<Drop>,
    rng: Rng,
    time: f32,
    hue: f32,
    timer: f32,
}

impl Ink {
    pub fn new() -> Self {
        Self {
            dye: Vec::new(),
            next: Vec::new(),
            gw: 0,
            gh: 0,
            drops: Vec::new(),
            rng: Rng::new(0x1A4B),
            time: 0.0,
            hue: 0.0,
            timer: 0.0,
        }
    }

    /// Splats a soft blob of fresh ink and registers it as a bloom source.
    fn drop_ink(&mut self, bass: f32) {
        let (gw, gh) = (self.gw as f32, self.gh as f32);
        let s = gw.min(gh);
        let x = self.rng.range(0.15, 0.85) * gw;
        let y = self.rng.range(0.15, 0.85) * gh;
        self.hue += 0.38;
        let col = hsv(self.hue + self.rng.range(0.0, 0.1), 0.9, 1.0).map(|v| v as f32 / 255.0);
        let r = (s * (0.035 + 0.035 * bass)).max(0.8);
        let reach = (r * 3.0).ceil() as i32;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (cx, cy) = (x as i32 + dx, y as i32 + dy);
                if cx < 0 || cy < 0 || cx >= self.gw as i32 || cy >= self.gh as i32 {
                    continue;
                }
                let d2 = ((cx as f32 - x).powi(2) + (cy as f32 - y).powi(2)) / (r * r);
                let a = (-d2).exp();
                let p = &mut self.dye[cy as usize * self.gw + cx as usize];
                for ch in 0..3 {
                    p[ch] += (col[ch] - p[ch]) * a;
                }
            }
        }
        if self.drops.len() >= MAX_DROPS {
            self.drops.remove(0);
        }
        self.drops.push(Drop {
            x,
            y,
            age: 0.0,
            push: 0.6 + bass,
        });
    }
}

/// Stream function of the current: two octaves of drifting noise.
fn psi(x: f32, y: f32, t: f32) -> f32 {
    noise(x + t * 0.3, y - t * 0.2)
        + 0.5 * noise(x * 2.1 - t * 0.5, y * 2.1 + 7.0)
        + 0.25 * noise(x * 4.3 + 3.0, y * 4.3 + t * 0.7)
}

impl Effect for Ink {
    fn name(&self) -> &'static str {
        "Ink"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let cell = if c.w * c.h < 120 * 60 { 1 } else { CELL };
        let (gw, gh) = (c.w.div_ceil(cell), c.h.div_ceil(cell));
        if (gw, gh) != (self.gw, self.gh) {
            (self.gw, self.gh) = (gw, gh);
            self.dye = vec![[0.0; 3]; gw * gh];
            self.next = vec![[0.0; 3]; gw * gh];
            self.drops.clear();
            self.timer = 0.0;
        }

        self.time += dt * (0.25 + 0.5 * f.level);
        self.hue += dt * 0.02;
        self.timer -= dt;
        if (f.beat && self.timer < 1.6) || self.timer <= 0.0 {
            self.drop_ink(f.bass);
            self.timer = self.rng.range(2.0, 2.8);
        }
        for d in &mut self.drops {
            d.age += dt;
        }
        self.drops.retain(|d| d.age < DROP_LIFE);

        // Semi-Lagrangian advection along the curl of the stream function,
        // plus a radial push from each fresh drop.
        let s = gw.min(gh) as f32;
        let fscale = 3.0 / s.max(1.0);
        let amp = s * (0.1 + 0.3 * f.bass);
        let t = self.time;
        let e = 0.25;
        let decay = (-dt * 0.16).exp();
        let (maxx, maxy) = ((gw - 1) as f32, (gh - 1) as f32);
        for y in 0..gh {
            for x in 0..gw {
                let (nx, ny) = (x as f32 * fscale, y as f32 * fscale);
                let mut vx = (psi(nx, ny + e, t) - psi(nx, ny - e, t)) / (2.0 * e) * amp;
                let mut vy = -(psi(nx + e, ny, t) - psi(nx - e, ny, t)) / (2.0 * e) * amp;
                for d in &self.drops {
                    let (dx, dy) = (x as f32 - d.x, y as f32 - d.y);
                    let reach = s * (0.08 + 0.3 * (1.0 - (-d.age * 1.5).exp()));
                    let k = d.push * s * 0.5 * (-d.age * 1.2).exp() / reach;
                    let fall = (-(dx * dx + dy * dy) / (reach * reach)).exp();
                    vx += dx * k * fall;
                    vy += dy * k * fall;
                }
                let sx = (x as f32 - vx * dt).clamp(0.0, maxx);
                let sy = (y as f32 - vy * dt).clamp(0.0, maxy);
                let (x0, y0) = (sx as usize, sy as usize);
                let (x1, y1) = ((x0 + 1).min(gw - 1), (y0 + 1).min(gh - 1));
                let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                let (a, b) = (self.dye[y0 * gw + x0], self.dye[y0 * gw + x1]);
                let (cc, dd) = (self.dye[y1 * gw + x0], self.dye[y1 * gw + x1]);
                let mut out = [0.0; 3];
                for (ch, o) in out.iter_mut().enumerate() {
                    let top = a[ch] + (b[ch] - a[ch]) * fx;
                    let bot = cc[ch] + (dd[ch] - cc[ch]) * fx;
                    *o = (top + (bot - top) * fy) * decay;
                }
                self.next[y * gw + x] = out;
            }
        }
        std::mem::swap(&mut self.dye, &mut self.next);

        // Dark water, ink glowing a little brighter on beats.
        let g = 0.85 + 0.3 * f.pulse;
        let bg = [0.012, 0.03, 0.05];
        for (o, d) in self.next.iter_mut().zip(&self.dye) {
            // Normalize to the hue and lift thin wisps so faint ink stays visible.
            let dens = d[0].max(d[1]).max(d[2]);
            let lift = (1.0 - (-dens * 3.0).exp()) / dens.max(1e-4) * g;
            for ch in 0..3 {
                o[ch] = bg[ch] * (1.0 - dens).max(0.0) + d[ch] * lift;
            }
        }
        upscale(c, &self.next, gw, gh);
    }
}
