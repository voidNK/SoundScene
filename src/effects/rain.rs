use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, add, mix, rgb, scale};

const MAX_DROPS: usize = 1500;
const MAX_SPLASHES: usize = 600;
const MAX_RIPPLES: usize = 300;
/// Street level as a fraction of the height; below it is the wet ground.
const GROUND: f32 = 0.76;
const RIPPLE_LIFE: f32 = 0.7;

/// Positions are fractions of the canvas, speeds are in canvas heights per second.
struct Drop {
    x: f32,
    y: f32,
    /// 0 = far away (dim, slow, lands near the street line), 1 = close.
    z: f32,
}

struct Splash {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    floor: f32,
    life: f32,
}

struct Ripple {
    x: f32,
    y: f32,
    age: f32,
    size: f32,
}

struct Building {
    x0: f32,
    x1: f32,
    top: f32,
    seed: u32,
}

/// Rain at night over a city skyline: slanted streaks that splash and leave
/// ripples on the wet street, which mirrors the sky. Rain density follows
/// the level; strong beats strike lightning that lights up the whole scene.
pub struct Rain {
    drops: Vec<Drop>,
    splashes: Vec<Splash>,
    ripples: Vec<Ripple>,
    buildings: Vec<Building>,
    /// Lightning segments (x0, y0, x1, y1, brightness), fractions of the canvas.
    bolt: Vec<(f32, f32, f32, f32, f32)>,
    bolt_life: f32,
    flash: f32,
    restrike: f32,
    cooldown: f32,
    emit: f32,
    time: f32,
    rng: Rng,
}

impl Rain {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x4A1E);
        let mut buildings = Vec::new();
        let mut x = -0.02;
        while x < 1.0 {
            let wdt = rng.range(0.03, 0.09);
            buildings.push(Building {
                x0: x,
                x1: x + wdt,
                top: GROUND - rng.range(0.06, 0.32),
                seed: rng.next_u32(),
            });
            x += wdt + rng.range(0.0, 0.015);
        }
        Self {
            drops: Vec::new(),
            splashes: Vec::new(),
            ripples: Vec::new(),
            buildings,
            bolt: Vec::new(),
            bolt_life: 0.0,
            flash: 0.0,
            restrike: 0.0,
            cooldown: 0.0,
            emit: 0.0,
            time: 0.0,
            rng,
        }
    }

    fn strike(&mut self) {
        self.bolt.clear();
        let rng = &mut self.rng;
        let (mut x, mut y) = (rng.range(0.15, 0.85), 0.0);
        let end = GROUND - 0.25;
        while y < end {
            let (nx, ny) = (x + rng.range(-0.05, 0.05), y + rng.range(0.03, 0.07));
            self.bolt.push((x, y, nx, ny, 1.0));
            if rng.f32() < 0.3 {
                let dir = if rng.f32() < 0.5 { -1.0 } else { 1.0 };
                let (mut bx, mut by) = (nx, ny);
                for _ in 0..3 + rng.next_u32() % 3 {
                    let (ex, ey) = (bx + dir * rng.range(0.01, 0.05), by + rng.range(0.02, 0.05));
                    self.bolt.push((bx, by, ex, ey, 0.45));
                    (bx, by) = (ex, ey);
                }
            }
            (x, y) = (nx, ny);
        }
        self.bolt_life = 0.25;
        self.flash = 1.0;
        self.restrike = 0.12;
        self.cooldown = 1.8;
    }
}

fn hash(n: u32) -> u32 {
    let n = (n ^ 61) ^ (n >> 16);
    let n = n.wrapping_mul(9);
    let n = n ^ (n >> 4);
    let n = n.wrapping_mul(0x27d4_eb2d);
    n ^ (n >> 15)
}

impl Effect for Rain {
    fn name(&self) -> &'static str {
        "Rain"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let (w, h) = (c.w as f32, c.h as f32);
        // Horizontal speeds are in heights per second, converted to x fractions.
        let aspect = if c.w > 0 { h / w } else { 1.0 };
        let wind = 0.12 + 0.08 * (self.time * 0.15).sin();

        // Lightning.
        self.cooldown -= dt;
        if f.beat && f.bass > 0.6 && self.cooldown <= 0.0 && self.rng.f32() < 0.6 {
            self.strike();
        }
        if self.restrike > 0.0 {
            self.restrike -= dt;
            if self.restrike <= 0.0 {
                self.flash = self.flash.max(0.75);
            }
        }
        self.flash *= (-dt * 7.0).exp();
        self.bolt_life -= dt;

        // Rain.
        self.emit += dt * (50.0 + 900.0 * f.level) * (w / 160.0).clamp(0.15, 2.0);
        while self.emit >= 1.0 {
            self.emit -= 1.0;
            if self.drops.len() < MAX_DROPS {
                let z = self.rng.f32();
                self.drops.push(Drop {
                    x: self.rng.range(-0.15, 1.0),
                    y: -self.rng.range(0.0, 0.2),
                    z: z * z,
                });
            }
        }
        let (rng, splashes, ripples) = (&mut self.rng, &mut self.splashes, &mut self.ripples);
        self.drops.retain_mut(|d| {
            let speed = 1.3 + 1.6 * d.z;
            d.y += speed * dt;
            d.x += wind * speed * dt * aspect;
            let land = GROUND + d.z * (1.0 - GROUND) * 0.97;
            if d.y < land {
                return d.x < 1.1;
            }
            if d.z > 0.08 && ripples.len() < MAX_RIPPLES {
                ripples.push(Ripple {
                    x: d.x,
                    y: land,
                    age: 0.0,
                    size: 0.015 + 0.05 * d.z,
                });
            }
            if d.z > 0.3 {
                for _ in 0..2 + rng.next_u32() % 3 {
                    if splashes.len() < MAX_SPLASHES {
                        splashes.push(Splash {
                            x: d.x,
                            y: land,
                            vx: rng.range(-0.25, 0.25) * d.z,
                            vy: -rng.range(0.2, 0.5) * d.z,
                            floor: land,
                            life: 0.35,
                        });
                    }
                }
            }
            false
        });
        for s in &mut self.splashes {
            s.vy += 3.0 * dt;
            s.x += s.vx * dt * aspect;
            s.y += s.vy * dt;
            s.life -= dt;
        }
        self.splashes.retain(|s| s.life > 0.0 && !(s.vy > 0.0 && s.y > s.floor));
        for r in &mut self.ripples {
            r.age += dt;
        }
        self.ripples.retain(|r| r.age < RIPPLE_LIFE);

        if c.w == 0 || c.h == 0 {
            return;
        }
        let ground = ((h * GROUND) as usize).min(c.h);
        let flash = self.flash;

        // Sky.
        let flash_col = rgb(0.55, 0.55, 0.75);
        for y in 0..ground {
            let g = y as f32 / ground.max(1) as f32;
            let col = mix(rgb(0.02, 0.025, 0.06), rgb(0.08, 0.08, 0.13), g);
            let col = mix(col, flash_col, flash * (0.5 + 0.4 * (1.0 - g)));
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }

        // Bolt behind the skyline.
        if self.bolt_life > 0.0 {
            let k = (self.bolt_life / 0.25).clamp(0.0, 1.0);
            for &(x0, y0, x1, y1, b) in &self.bolt {
                let core = scale(rgb(0.9, 0.9, 1.0), b * k);
                let glow = scale(rgb(0.3, 0.25, 0.6), b * k);
                let (ax, ay, bx, by) = (x0 * w, y0 * h, x1 * w, y1 * h);
                c.line_add(ax, ay, bx, by, core);
                c.line_add(ax - 1.0, ay, bx - 1.0, by, glow);
                c.line_add(ax + 1.0, ay, bx + 1.0, by, glow);
            }
        }

        // Skyline with a few lit windows.
        let body = add(rgb(0.012, 0.012, 0.02), scale(rgb(0.1, 0.1, 0.14), flash));
        for b in &self.buildings {
            let x0 = (b.x0 * w).max(0.0) as usize;
            let x1 = ((b.x1 * w).max(0.0) as usize).min(c.w);
            let y0 = ((b.top * h).max(0.0) as usize).min(ground);
            for y in y0..ground {
                for x in x0..x1 {
                    let (lx, ly) = (x - x0, y - y0);
                    let lit = lx % 3 == 1
                        && ly % 3 == 1
                        && x + 1 < x1
                        && hash(b.seed ^ (lx as u32 * 131 + ly as u32 * 7919)).is_multiple_of(5);
                    let col = if lit {
                        let warm = (hash(b.seed + lx as u32) % 3) as f32 * 0.1;
                        rgb(0.55 + warm, 0.42 + warm * 0.5, 0.15)
                    } else {
                        body
                    };
                    c.set(x, y, col);
                }
            }
        }

        // Wet street: a dim, wobbling mirror image of everything above.
        for y in ground..c.h {
            let d = y - ground;
            let src_y = ground.saturating_sub(d + 1).min(ground.saturating_sub(1));
            let depth = d as f32 / (c.h - ground).max(1) as f32;
            let wobble = ((y as f32 * 1.7 + self.time * 6.0).sin() * (1.0 + 2.0 * depth)) as i32;
            for x in 0..c.w {
                let sx = (x as i32 + wobble).clamp(0, c.w as i32 - 1) as usize;
                let src = if ground > 0 { c.get(sx, src_y) } else { [0; 3] };
                let col = add(scale(src, 0.35 - 0.15 * depth), rgb(0.015, 0.017, 0.025));
                c.set(x, y, col);
            }
        }

        for r in &self.ripples {
            let k = 1.0 - r.age / RIPPLE_LIFE;
            let rad = r.size * h * (r.age / RIPPLE_LIFE).sqrt();
            let col = scale(add(rgb(0.25, 0.28, 0.35), scale(flash_col, flash)), k * 0.6);
            let n = (rad * 6.0).ceil().clamp(4.0, 64.0) as usize;
            let (cx, cy) = (r.x * w, r.y * h);
            for i in 0..n {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                c.plot_add((cx + rad * a.cos()) as i32, (cy + rad * 0.3 * a.sin()) as i32, col);
            }
        }

        for d in &self.drops {
            let len = 0.02 + 0.07 * d.z;
            let b = 0.18 + 0.55 * d.z + 0.3 * flash;
            let col = rgb(0.55 * b, 0.6 * b, 0.75 * b);
            let (hx, hy) = (d.x * w, d.y * h);
            let (tx, ty) = (hx - wind * len * h, hy - len * h);
            c.line_add(tx, ty, hx, hy, col);
        }

        for s in &self.splashes {
            let b = (s.life / 0.35).clamp(0.0, 1.0) * 0.7;
            c.plot_add((s.x * w) as i32, (s.y * h) as i32, rgb(0.6 * b, 0.65 * b, 0.8 * b));
        }
    }
}
