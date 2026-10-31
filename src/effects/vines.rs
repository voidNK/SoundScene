use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, mix, rgb, scale};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Growth step length in units of the canvas height.
const STEP: f32 = 0.01;
const MAX_SEGS: usize = 5000;
const MAX_TIPS: usize = 40;
const MAX_BLOSSOMS: usize = 160;
/// Seconds the finished wall stays up before it fades out and regrows.
const HOLD: f32 = 4.0;
const FADE: f32 = 2.0;

/// Positions in units of the canvas height.
struct Seg {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    width: f32,
    tendril: bool,
}

struct Tip {
    x: f32,
    y: f32,
    angle: f32,
    curl: f32,
    width: f32,
    /// Remaining length.
    life: f32,
    tendril: bool,
    depth: u8,
    since_leaf: f32,
    side: f32,
    acc: f32,
}

struct Leaf {
    x: f32,
    y: f32,
    angle: f32,
    size: f32,
    born: f32,
    shade: f32,
}

struct Blossom {
    x: f32,
    y: f32,
    size: f32,
    hue: f32,
    born: f32,
    spin: f32,
}

/// Vines climbing across an old stone wall: stems wander and branch, throw
/// out curling tendrils and sprout leaves. Beats make blossoms open along the
/// vines, the level and bass speed up the growth, and once the wall is
/// covered it fades and starts over.
pub struct Vines {
    segs: Vec<Seg>,
    tips: Vec<Tip>,
    leaves: Vec<Leaf>,
    blossoms: Vec<Blossom>,
    /// Time since the growth finished, `None` while growing.
    done: Option<f32>,
    time: f32,
    rng: Rng,
}

impl Vines {
    pub fn new() -> Self {
        // Starts as "finished" so the first frame plants roots for the real canvas.
        Self {
            segs: Vec::new(),
            tips: Vec::new(),
            leaves: Vec::new(),
            blossoms: Vec::new(),
            done: Some(HOLD + FADE),
            time: 0.0,
            rng: Rng::new(0x71_4E5),
        }
    }

    fn reset(&mut self, aspect: f32) {
        self.segs.clear();
        self.tips.clear();
        self.leaves.clear();
        self.blossoms.clear();
        self.done = None;
        let roots = ((aspect * 2.5) as usize).clamp(2, 6);
        for i in 0..roots {
            let (x, y, angle) = match i % 4 {
                1 => (0.0, self.rng.range(0.4, 0.9), 0.0),
                3 => (aspect, self.rng.range(0.4, 0.9), PI),
                _ => (self.rng.range(0.1, 0.9) * aspect, 1.0, -FRAC_PI_2),
            };
            self.tips.push(Tip {
                x,
                y,
                angle: angle + self.rng.range(-0.3, 0.3),
                curl: 0.0,
                width: 0.012,
                life: self.rng.range(3.0, 5.0),
                tendril: false,
                depth: 0,
                since_leaf: 0.0,
                side: 1.0,
                acc: 0.0,
            });
        }
    }

    /// Advances one tip by one step; returns new tips it spawned.
    fn step(&mut self, i: usize, aspect: f32, out: &mut Vec<Tip>) {
        let rng = &mut self.rng;
        let t = &mut self.tips[i];
        if t.tendril {
            // Tendrils wind ever tighter into a spiral.
            t.curl *= 1.12;
        } else {
            t.curl = (t.curl + rng.range(-1.0, 1.0) * 0.8).clamp(-3.0, 3.0);
            // Steer back towards the middle near the edges.
            let (cx, cy) = (aspect * 0.5 - t.x, 0.5 - t.y);
            let m = 0.12;
            if t.x < m || t.x > aspect - m || t.y < m || t.y > 1.0 - m {
                let want = cy.atan2(cx);
                let diff = (want - t.angle + PI).rem_euclid(TAU) - PI;
                t.curl += diff * 2.0;
            }
        }
        t.angle += t.curl * STEP;
        let (x0, y0) = (t.x, t.y);
        t.x += t.angle.cos() * STEP;
        t.y += t.angle.sin() * STEP;
        t.life -= STEP;
        if t.tendril {
            t.width *= 0.97;
        } else {
            t.width = (t.width * 0.9985).max(0.003);
        }
        self.segs.push(Seg {
            x0,
            y0,
            x1: t.x,
            y1: t.y,
            width: t.width,
            tendril: t.tendril,
        });
        if t.tendril {
            return;
        }
        t.since_leaf += STEP;
        if t.since_leaf > 0.045 {
            t.since_leaf = 0.0;
            t.side = -t.side;
            self.leaves.push(Leaf {
                x: t.x,
                y: t.y,
                angle: t.angle + t.side * rng.range(0.7, 1.3),
                size: rng.range(0.025, 0.04) * (0.6 + 30.0 * t.width).min(1.3),
                born: self.time,
                shade: rng.f32(),
            });
        }
        let r = rng.f32();
        if r < 0.025 && t.depth < 3 {
            let s = if rng.f32() < 0.5 { 1.0 } else { -1.0 };
            out.push(Tip {
                x: t.x,
                y: t.y,
                angle: t.angle + s * rng.range(0.5, 1.1),
                curl: 0.0,
                width: t.width * 0.75,
                life: rng.range(0.4, 1.2),
                tendril: false,
                depth: t.depth + 1,
                since_leaf: 0.0,
                side: s,
                acc: 0.0,
            });
        } else if r < 0.06 {
            let s = if rng.f32() < 0.5 { 1.0 } else { -1.0 };
            out.push(Tip {
                x: t.x,
                y: t.y,
                angle: t.angle + s * 0.8,
                curl: s * rng.range(5.0, 8.0),
                width: 0.004,
                life: rng.range(0.08, 0.16),
                tendril: true,
                depth: 9,
                since_leaf: 0.0,
                side: s,
                acc: 0.0,
            });
        }
    }
}

/// Bounds-checked overwrite blended with `alpha`.
fn put(c: &mut Canvas, x: i32, y: i32, col: Rgb, alpha: f32) {
    if x >= 0 && y >= 0 && (x as usize) < c.w && (y as usize) < c.h {
        let p = c.get(x as usize, y as usize);
        c.set(x as usize, y as usize, mix(p, col, alpha));
    }
}

impl Effect for Vines {
    fn name(&self) -> &'static str {
        "Vines"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let now = self.time;
        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if c.h > 0 { (w / h).clamp(0.2, 10.0) } else { 1.0 };

        if let Some(d) = self.done.as_mut() {
            *d += dt;
            if *d >= HOLD + FADE {
                self.reset(aspect);
            }
        } else {
            let dist = dt * (0.08 + 0.25 * f.level + 0.2 * f.bass);
            let mut i = 0;
            while i < self.tips.len() {
                self.tips[i].acc += dist;
                let mut born = Vec::new();
                while self.tips[i].acc >= STEP && self.tips[i].life > 0.0 && self.segs.len() < MAX_SEGS {
                    self.tips[i].acc -= STEP;
                    self.step(i, aspect, &mut born);
                }
                let room = MAX_TIPS.saturating_sub(self.tips.len());
                self.tips.extend(born.into_iter().take(room));
                i += 1;
            }
            self.tips
                .retain(|t| t.life > 0.0 && t.x > -0.2 && t.x < aspect + 0.2 && t.y > -0.2 && t.y < 1.2);
            if self.tips.is_empty() || self.segs.len() >= MAX_SEGS {
                self.done = Some(0.0);
            }
        }
        if f.beat && !self.leaves.is_empty() {
            for _ in 0..1 + (f.bass * 3.0) as usize {
                if self.blossoms.len() >= MAX_BLOSSOMS {
                    break;
                }
                let k = (self.rng.f32() * self.leaves.len() as f32) as usize % self.leaves.len();
                let l = &self.leaves[k];
                let (x, y) = (l.x, l.y);
                self.blossoms.push(Blossom {
                    x,
                    y,
                    size: self.rng.range(0.02, 0.035),
                    hue: [0.92, 0.83, 0.97, 0.12, 0.75][self.rng.next_u32() as usize % 5],
                    born: now,
                    spin: self.rng.range(0.0, TAU),
                });
            }
        }

        if c.w == 0 || c.h == 0 {
            return;
        }

        // Stone wall: offset rows of blocks with dark mortar.
        let bh = (h / 9.0).max(2.0);
        let bw = bh * 2.2;
        for y in 0..c.h {
            let row = (y as f32 / bh).floor();
            let vy = y as f32 / bh - row;
            for x in 0..c.w {
                let fx = x as f32 / bw + row * 0.5;
                let col_i = fx.floor();
                let vx = fx - col_i;
                let n = ((row * 12.9898 + col_i * 78.233).sin() * 43_758.547).fract().abs();
                let stone = mix(rgb(0.1, 0.1, 0.11), rgb(0.17, 0.16, 0.15), n);
                let mortar = vy < 0.08 || vx < 0.04;
                let shade = 1.0 - 0.35 * (y as f32 / h);
                c.set(x, y, scale(if mortar { rgb(0.04, 0.04, 0.05) } else { stone }, shade));
            }
        }

        let alpha = match self.done {
            Some(d) if d > HOLD => (1.0 - (d - HOLD) / FADE).clamp(0.0, 1.0),
            _ => 1.0,
        };

        for s in &self.segs {
            let col = if s.tendril {
                rgb(0.45, 0.65, 0.2)
            } else {
                mix(
                    rgb(0.2, 0.5, 0.12),
                    rgb(0.25, 0.2, 0.08),
                    (s.width * 80.0 - 0.3).clamp(0.0, 1.0),
                )
            };
            let r = s.width * h * 0.5;
            let (x0, y0, x1, y1) = (s.x0 * h, s.y0 * h, s.x1 * h, s.y1 * h);
            let steps = ((x1 - x0).abs().max((y1 - y0).abs()) * 1.5).ceil().clamp(1.0, 50.0) as i32;
            let ri = r.round() as i32;
            for k in 0..=steps {
                let t = k as f32 / steps as f32;
                let (x, y) = ((x0 + (x1 - x0) * t).round() as i32, (y0 + (y1 - y0) * t).round() as i32);
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        if dx * dx + dy * dy <= ri * ri {
                            put(c, x + dx, y + dy, col, alpha);
                        }
                    }
                }
            }
        }

        for l in &self.leaves {
            let grow = ((now - l.born) / 0.6).clamp(0.0, 1.0);
            let len = l.size * h * grow;
            if len < 0.5 {
                put(c, (l.x * h) as i32, (l.y * h) as i32, rgb(0.2, 0.55, 0.15), alpha);
                continue;
            }
            let (ca, sa) = (l.angle.cos(), l.angle.sin());
            let base = mix(rgb(0.12, 0.45, 0.1), rgb(0.35, 0.65, 0.15), l.shade);
            let ri = len.ceil() as i32 + 1;
            let (ox, oy) = (l.x * h, l.y * h);
            let (ix, iy) = (ox.round() as i32, oy.round() as i32);
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let (px, py) = ((ix + dx) as f32 - ox, (iy + dy) as f32 - oy);
                    let u = (px * ca + py * sa) / len;
                    let v = (-px * sa + py * ca) / len;
                    if !(0.0..=1.0).contains(&u) {
                        continue;
                    }
                    let half = 0.38 * (PI * u).sin() * (1.0 - 0.3 * u);
                    if v.abs() <= half {
                        let vein = v.abs() < 0.06;
                        let k = 0.8 + 0.4 * (v / half.max(0.01)) * 0.5;
                        let col = if vein {
                            mix(base, rgb(0.6, 0.8, 0.4), 0.5)
                        } else {
                            scale(base, k)
                        };
                        put(c, ix + dx, iy + dy, col, alpha);
                    }
                }
            }
        }

        for b in &self.blossoms {
            let t = ((now - b.born) / 0.8).clamp(0.0, 1.0);
            let open = t * t * (3.0 - 2.0 * t);
            let r = b.size * h * open * (1.0 + 0.2 * f.pulse);
            if r < 0.5 {
                continue;
            }
            let petal = hsv(b.hue, 0.55, 1.0);
            let ri = r.ceil() as i32 + 1;
            let (ox, oy) = (b.x * h, b.y * h);
            let (ix, iy) = (ox.round() as i32, oy.round() as i32);
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    let (px, py) = ((ix + dx) as f32 - ox, (iy + dy) as f32 - oy);
                    let d = (px * px + py * py).sqrt();
                    let a = py.atan2(px) + b.spin;
                    let edge = r * (0.45 + 0.55 * (2.5 * a).cos().abs());
                    if d <= edge {
                        let col = if d < r * 0.25 {
                            rgb(1.0, 0.85, 0.25)
                        } else {
                            mix(petal, rgb(1.0, 1.0, 1.0), (1.0 - d / edge.max(0.1)) * 0.4)
                        };
                        put(c, ix + dx, iy + dy, col, alpha);
                    }
                }
            }
        }
    }
}
