use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, rgb, scale};

const DEPTH: u32 = 9;
const LIFE: f32 = 32.0;
const GROW: f32 = 6.0;
const MAX_PETALS: usize = 400;

struct Petal {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    hue: f32,
}

/// Per-frame drawing parameters shared by the whole recursion.
struct Ctx {
    seed: u64,
    t: f32,
    sway: f32,
    grow: f32,
    unit: f32,
    hue: f32,
    pulse: f32,
    blossoms: Vec<(f32, f32, f32)>,
}

/// Organic fractal tree of tapering line branches, randomly forked from a
/// seed. It grows from the trunk out, its branches sway with the bass and
/// the blossoms on its twigs flash on beats, glow with the spectrum and
/// shed petals. Every half minute a new tree grows.
pub struct FractalTree {
    seed: u64,
    age: f32,
    bass: f32,
    hue: f32,
    rng: Rng,
    petals: Vec<Petal>,
}

impl FractalTree {
    pub fn new() -> Self {
        Self {
            seed: 0x7EE,
            age: 0.0,
            bass: 0.0,
            hue: 0.9,
            rng: Rng::new(0x7EE5),
            petals: Vec::new(),
        }
    }
}

/// splitmix64, gives every branch its own stable random numbers.
fn hash(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit(h: u64, k: u32) -> f32 {
    ((h >> (k * 12)) & 0xFFF) as f32 / 4095.0
}

/// Line with round caps of the given radius.
fn thick_line(c: &mut Canvas, a: (f32, f32), b: (f32, f32), r: f32, col: Rgb) {
    let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as i32).clamp(1, 1000);
    let ri = r.ceil() as i32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        if ri <= 0 || r < 0.75 {
            c.plot(x as i32, y as i32, col);
            continue;
        }
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if (dx * dx + dy * dy) as f32 <= r * r + 0.5 {
                    c.plot(x as i32 + dx, y as i32 + dy, col);
                }
            }
        }
    }
}

fn branch(c: &mut Canvas, ctx: &mut Ctx, from: (f32, f32), angle: f32, len: f32, depth: u32, id: u64) {
    let h = hash(id ^ ctx.seed);
    let shown = (ctx.grow * (DEPTH + 1) as f32 - depth as f32).clamp(0.0, 1.0);
    if shown <= 0.0 || len < 0.6 {
        return;
    }
    // Thin twigs move more than the trunk.
    let wobble = (ctx.t * (1.1 + unit(h, 0)) + unit(h, 1) * 6.0).sin();
    let a = angle + wobble * ctx.sway * (0.01 + depth as f32 * 0.025);
    let l = len * shown;
    let to = (from.0 + a.sin() * l, from.1 - a.cos() * l);
    let u = depth as f32 / DEPTH as f32;
    let bark = rgb(0.25 + 0.2 * u, 0.15 + 0.2 * u, 0.1 + 0.05 * u);
    thick_line(c, from, to, (len * 0.07).min(ctx.unit * 0.04), bark);

    if depth == DEPTH || len < 2.0 {
        if shown >= 1.0 {
            ctx.blossoms.push((to.0, to.1, unit(h, 2)));
        }
        return;
    }
    let forks = if depth < 4 && unit(h, 3) < 0.3 { 3 } else { 2 };
    for k in 0..forks {
        let hk = hash(h.wrapping_add(k as u64));
        let side = if forks == 3 {
            k as f32 - 1.0
        } else {
            k as f32 * 2.0 - 1.0
        };
        let spread = (0.3 + 0.35 * unit(hk, 0)) * side + (unit(hk, 1) - 0.5) * 0.25;
        let shrink = 0.66 + 0.16 * unit(hk, 2);
        branch(c, ctx, to, a + spread, len * shrink, depth + 1, id * 4 + k as u64 + 1);
    }
}

impl Effect for FractalTree {
    fn name(&self) -> &'static str {
        "Fractal Tree"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.age += dt;
        if self.age > LIFE {
            self.age = 0.0;
            self.seed = hash(self.seed);
            self.hue = self.rng.f32();
        }
        self.bass += (f.bass - self.bass) * (1.0 - (-5.0 * dt).exp());

        let (w, h) = (c.w as f32, c.h as f32);
        let ground = h * 0.94;
        for y in 0..c.h {
            let g = y as f32 / h.max(1.0);
            let col = if (y as f32) < ground {
                rgb(0.03 + 0.05 * g, 0.02 + 0.02 * g, 0.07 + 0.04 * g)
            } else {
                rgb(0.05, 0.04, 0.03)
            };
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }

        let grow = (self.age / GROW).min(1.0);
        let mut ctx = Ctx {
            seed: self.seed,
            t,
            sway: 0.4 + 2.2 * self.bass,
            grow: grow * (2.0 - grow),
            unit: h.min(w * 0.6),
            hue: self.hue,
            pulse: f.pulse,
            blossoms: Vec::new(),
        };
        let trunk = ctx.unit * 0.26;
        branch(c, &mut ctx, (w * 0.5, ground), 0.0, trunk, 0, 0);

        let bins = f.spectrum.len();
        let wilt = ((LIFE - self.age) / 2.5).clamp(0.0, 1.0);
        let r = (ctx.unit * 0.008).clamp(0.0, 1.2);
        for (i, &(x, y, k)) in ctx.blossoms.iter().enumerate() {
            let band = if bins > 0 { f.spectrum[i * 7 % bins] } else { 0.0 };
            let v = (0.35 + 0.35 * band + 0.3 * ctx.pulse) * wilt;
            let col = hsv(ctx.hue + k * 0.12, 0.6 - 0.35 * ctx.pulse, v.min(1.0));
            let ri = r.ceil() as i32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    if (dx * dx + dy * dy) as f32 <= r * r + 0.5 {
                        c.plot(x as i32 + dx, y as i32 + dy, col);
                    }
                }
            }
        }

        if f.beat && !ctx.blossoms.is_empty() {
            for _ in 0..30 {
                if self.petals.len() >= MAX_PETALS {
                    break;
                }
                let i = self.rng.next_u32() as usize % ctx.blossoms.len();
                let (x, y, k) = ctx.blossoms[i];
                self.petals.push(Petal {
                    x: x / h.max(1.0),
                    y: y / h.max(1.0),
                    vx: self.rng.range(-0.05, 0.05),
                    vy: self.rng.range(-0.02, 0.02),
                    life: 1.0,
                    hue: ctx.hue + k * 0.12,
                });
            }
        }
        let breeze = 0.04 + 0.1 * f.level;
        for p in &mut self.petals {
            p.vy += 0.12 * dt;
            p.vx += (breeze - p.vx) * dt;
            p.x += p.vx * dt + (t * 3.0 + p.hue * 20.0).sin() * 0.03 * dt;
            p.y += p.vy.min(0.12) * dt;
            p.life -= dt * 0.25;
            let col = scale(hsv(p.hue, 0.5, 1.0), p.life.clamp(0.0, 1.0) * 0.8);
            c.plot_add((p.x * h) as i32, (p.y * h) as i32, col);
        }
        self.petals.retain(|p| p.life > 0.0 && p.y * h < ground);
    }
}
