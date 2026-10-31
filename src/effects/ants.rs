use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, rgb};
use std::f32::consts::{PI, TAU};

/// Pixels per pheromone cell along each axis.
const CELL: usize = 2;
const MAX_ANTS: usize = 320;
const FOODS: usize = 3;
const STEPS_PER_SEC: f32 = 60.0;
const MAX_STEPS: usize = 5;
const FOOD_AMOUNT: u32 = 160;

struct Ant {
    x: f32,
    y: f32,
    a: f32,
    carrying: bool,
    /// Scent strength laid down; fades the longer the ant is away from its last stop.
    scent: f32,
}

struct Food {
    x: f32,
    y: f32,
    left: u32,
}

/// Ant colony foraging: ants leave the nest, wander until they find food,
/// and carry it home while laying pheromone trails that others follow, so
/// glowing highways form between the nest and the food. Bass speeds the
/// ants up, beats drop a fresh food source.
pub struct Ants {
    home: Vec<f32>,
    food: Vec<f32>,
    colors: Vec<[f32; 3]>,
    gw: usize,
    gh: usize,
    ants: Vec<Ant>,
    foods: Vec<Food>,
    rng: Rng,
    acc: f32,
    hue: f32,
    flash: f32,
}

impl Ants {
    pub fn new() -> Self {
        Self {
            home: Vec::new(),
            food: Vec::new(),
            colors: Vec::new(),
            gw: 0,
            gh: 0,
            ants: Vec::new(),
            foods: Vec::new(),
            rng: Rng::new(0xA275),
            acc: 0.0,
            hue: 0.0,
            flash: 0.0,
        }
    }

    fn nest(&self) -> (f32, f32) {
        (self.gw as f32 * 0.5, self.gh as f32 * 0.5)
    }

    fn radius(&self) -> f32 {
        (self.gw.min(self.gh) as f32 * 0.06).max(1.0)
    }

    /// A food source somewhere away from the nest.
    fn new_food(&mut self) -> Food {
        let (nx, ny) = self.nest();
        let a = self.rng.range(0.0, TAU);
        let d = self.rng.range(0.55, 0.9);
        Food {
            x: (nx + a.cos() * nx * d).min(self.gw as f32 - 1.0).max(0.0),
            y: (ny + a.sin() * ny * d).min(self.gh as f32 - 1.0).max(0.0),
            left: FOOD_AMOUNT,
        }
    }

    fn sample(map: &[f32], gw: usize, gh: usize, x: f32, y: f32) -> f32 {
        if x < 0.0 || y < 0.0 || x >= gw as f32 || y >= gh as f32 {
            return -1.0;
        }
        map[y as usize * gw + x as usize]
    }

    fn step(&mut self, speed: f32) {
        let (gw, gh) = (self.gw, self.gh);
        let (nx, ny) = self.nest();
        let reach = self.radius();
        let sensor = (gw.min(gh) as f32 * 0.04).clamp(1.5, 5.0);
        for i in 0..self.ants.len() {
            let ant = &self.ants[i];
            let (x, y, carrying) = (ant.x, ant.y, ant.carrying);
            let mut a = ant.a;
            // Steer towards the target directly once it is close.
            let target = if carrying {
                Some((nx, ny))
            } else {
                self.foods
                    .iter()
                    .filter(|f| f.left > 0)
                    .map(|f| (f.x, f.y))
                    .find(|&(fx, fy)| (fx - x).powi(2) + (fy - y).powi(2) < (reach * 3.0).powi(2))
            };
            if let Some((tx, ty)) =
                target.filter(|&(tx, ty)| (tx - x).powi(2) + (ty - y).powi(2) < (reach * 3.0).powi(2))
            {
                a = (ty - y).atan2(tx - x);
            } else {
                let map = if carrying { &self.home } else { &self.food };
                let probe =
                    |da: f32| Self::sample(map, gw, gh, x + (a + da).cos() * sensor, y + (a + da).sin() * sensor);
                let (l, m, r) = (probe(0.5), probe(0.0), probe(-0.5));
                if l > m && l > r {
                    a += 0.25;
                } else if r > m && r > l {
                    a -= 0.25;
                }
            }
            a += self.rng.range(-0.25, 0.25);
            let (mut x, mut y) = (x + a.cos() * speed, y + a.sin() * speed);
            if x < 0.0 || x >= gw as f32 {
                a = PI - a;
                x = x.clamp(0.0, gw as f32 - 0.01);
            }
            if y < 0.0 || y >= gh as f32 {
                a = -a;
                y = y.clamp(0.0, gh as f32 - 0.01);
            }
            let ant = &mut self.ants[i];
            (ant.x, ant.y, ant.a) = (x, y, a);
            ant.scent *= 0.993;

            // Searching ants mark the way home, carriers mark the way to food.
            let idx = (y as usize).min(gh - 1) * gw + (x as usize).min(gw - 1);
            let map = if ant.carrying { &mut self.food } else { &mut self.home };
            map[idx] = map[idx].max(ant.scent);

            if ant.carrying {
                if (x - nx).powi(2) + (y - ny).powi(2) < reach * reach {
                    ant.carrying = false;
                    ant.scent = 1.0;
                    ant.a += PI;
                }
            } else if let Some(fd) = self
                .foods
                .iter_mut()
                .find(|f| f.left > 0 && (f.x - x).powi(2) + (f.y - y).powi(2) < reach * reach)
            {
                fd.left -= 1;
                ant.carrying = true;
                ant.scent = 1.0;
                ant.a += PI;
            }
        }
        for m in self.home.iter_mut().chain(self.food.iter_mut()) {
            *m *= 0.988;
        }
        for k in 0..self.foods.len() {
            if self.foods[k].left == 0 {
                self.foods[k] = self.new_food();
            }
        }
    }
}

impl Effect for Ants {
    fn name(&self) -> &'static str {
        "Ants"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (gw, gh) = (c.w.div_ceil(CELL), c.h.div_ceil(CELL));
        if (gw, gh) != (self.gw, self.gh) {
            (self.gw, self.gh) = (gw, gh);
            self.home = vec![0.0; gw * gh];
            self.food = vec![0.0; gw * gh];
            self.colors = vec![[0.0; 3]; gw * gh];
            self.foods.clear();
            for _ in 0..FOODS {
                let fd = self.new_food();
                self.foods.push(fd);
            }
            self.ants.clear();
            let (nx, ny) = self.nest();
            for _ in 0..(gw * gh / 25).clamp(4, MAX_ANTS) {
                self.ants.push(Ant {
                    x: nx,
                    y: ny,
                    a: self.rng.range(0.0, TAU),
                    carrying: false,
                    scent: 1.0,
                });
            }
        }
        self.hue += dt * 0.01;
        self.flash = (self.flash - dt * 2.0).max(0.0);
        if f.beat {
            self.flash = 1.0;
            // Replace the most depleted food source with a fresh one.
            if let Some(k) = (0..self.foods.len()).min_by_key(|&k| self.foods[k].left) {
                self.foods[k] = self.new_food();
            }
        }

        let speed = 0.45 + 0.5 * f.bass + 0.3 * f.pulse;
        self.acc = (self.acc + dt * STEPS_PER_SEC).min(MAX_STEPS as f32);
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.step(speed);
        }

        // Home scent blue-violet, food scent amber-green, on dark soil.
        let glow = 0.8 + 0.4 * f.level;
        let hc = hsv(self.hue + 0.62, 0.8, 1.0).map(|v| v as f32 / 255.0);
        let fc = hsv(self.hue + 0.1, 0.85, 1.0).map(|v| v as f32 / 255.0);
        for ((o, &h), &fo) in self.colors.iter_mut().zip(&self.home).zip(&self.food) {
            let h = h * 0.3 * glow;
            let fo = fo.sqrt() * 0.6 * glow;
            for k in 0..3 {
                o[k] = 0.03 + hc[k] * h + fc[k] * fo;
            }
        }
        let (nx, ny) = self.nest();
        let reach = self.radius();
        for y in 0..gh {
            for x in 0..gw {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let i = y * gw + x;
                let dn = ((px - nx).powi(2) + (py - ny).powi(2)).sqrt() / reach;
                if dn < 1.3 {
                    let k = (1.3 - dn).min(1.0) * (0.6 + 0.4 * self.flash);
                    self.colors[i][0] += 0.55 * k;
                    self.colors[i][1] += 0.35 * k;
                    self.colors[i][2] += 0.25 * k;
                }
                for fd in &self.foods {
                    let size = reach * (0.4 + 0.6 * fd.left as f32 / FOOD_AMOUNT as f32);
                    let d = ((px - fd.x).powi(2) + (py - fd.y).powi(2)).sqrt() / size;
                    if d < 1.0 {
                        let k = 1.0 - d * d;
                        self.colors[i][0] += 0.2 * k;
                        self.colors[i][1] += 0.9 * k;
                        self.colors[i][2] += 0.3 * k;
                    }
                }
            }
        }
        upscale(c, &self.colors, gw, gh);

        let worker = rgb(0.85, 0.8, 0.75);
        let loaded = rgb(1.0, 0.9, 0.3);
        for ant in &self.ants {
            let (x, y) = ((ant.x * CELL as f32) as i32, (ant.y * CELL as f32) as i32);
            c.plot(x, y, if ant.carrying { loaded } else { worker });
        }
    }
}
