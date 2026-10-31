use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, mix, rgb, scale};
use std::f32::consts::TAU;

const FISH: usize = 110;

/// Position and velocity in units of the canvas height.
struct Swimmer {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    /// 0.6 (far) .. 1.0 (near), scales size and brightness.
    depth: f32,
    phase: f32,
}

/// A school of silvery fish in a blue gradient sea with light rays, flocking
/// like boids around a wandering lead point. Every beat throws the school
/// into a sudden turn, flashing their flanks as they bank; bass makes them
/// swim faster and the level brightens the water.
pub struct Fish {
    fish: Vec<Swimmer>,
    /// Where the school is heading, units of the height.
    goal: (f32, f32),
    goal_time: f32,
    beats: usize,
    /// Flank flash after a turn, decays.
    flash: f32,
    time: f32,
    rng: Rng,
}

impl Fish {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xF154);
        let fish = (0..FISH)
            .map(|_| {
                let a = rng.range(-0.4, 0.4);
                Swimmer {
                    x: rng.range(0.3, 1.0),
                    y: rng.range(0.3, 0.7),
                    vx: a.cos() * 0.2,
                    vy: a.sin() * 0.2,
                    depth: rng.range(0.6, 1.0),
                    phase: rng.range(0.0, TAU),
                }
            })
            .collect();
        Self {
            fish,
            goal: (0.8, 0.5),
            goal_time: 0.0,
            beats: 0,
            flash: 0.0,
            time: 0.0,
            rng,
        }
    }

    /// Picks a goal on the far side of the school so it keeps cruising.
    fn new_goal(&mut self, aspect: f32) {
        let n = self.fish.len().max(1) as f32;
        let cx = self.fish.iter().map(|f| f.x).sum::<f32>() / n;
        let side = if cx < aspect * 0.5 {
            self.rng.range(0.65, 0.9)
        } else {
            self.rng.range(0.1, 0.35)
        };
        self.goal = (side * aspect.max(0.2), self.rng.range(0.2, 0.8));
        self.goal_time = self.rng.range(5.0, 9.0);
    }
}

impl Effect for Fish {
    fn name(&self) -> &'static str {
        "Fish"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        let now = self.time;
        let (w, h) = (c.w as f32, c.h as f32);
        let aspect = if c.h > 0 { (w / h).clamp(0.2, 10.0) } else { 1.0 };

        self.goal_time -= dt;
        let n = self.fish.len().max(1) as f32;
        let cx = self.fish.iter().map(|f| f.x).sum::<f32>() / n;
        let cy = self.fish.iter().map(|f| f.y).sum::<f32>() / n;
        let near = (cx - self.goal.0).powi(2) + (cy - self.goal.1).powi(2) < 0.06;
        if self.goal_time <= 0.0 || near {
            self.new_goal(aspect);
        }
        // Every beat banks the whole school, every eighth picks a new heading.
        self.flash *= (-dt * 3.0).exp();
        let mut turn = 0.0;
        if f.beat {
            self.beats += 1;
            if self.beats.is_multiple_of(8) {
                self.new_goal(aspect);
            }
            self.flash = 1.0;
            turn = if self.beats.is_multiple_of(2) { 0.7 } else { -0.7 } * (0.6 + 0.6 * f.bass);
        }
        let speed = 0.18 + 0.25 * f.bass;
        let n = self.fish.len();
        let mut acc = vec![(0.0f32, 0.0f32); n];
        for (i, a) in acc.iter_mut().enumerate() {
            let me = &self.fish[i];
            let (mut cx, mut cy, mut ax, mut ay, mut sx, mut sy, mut cnt) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
            for (j, o) in self.fish.iter().enumerate() {
                if i == j {
                    continue;
                }
                let (dx, dy) = (o.x - me.x, o.y - me.y);
                let d2 = dx * dx + dy * dy;
                if d2 < 0.02 {
                    cx += dx;
                    cy += dy;
                    ax += o.vx;
                    ay += o.vy;
                    cnt += 1.0;
                    if d2 < 0.003 {
                        sx -= dx / (d2 + 1e-4);
                        sy -= dy / (d2 + 1e-4);
                    }
                }
            }
            let (mut fx, mut fy) = (0.0, 0.0);
            if cnt > 0.0 {
                fx += cx / cnt * 0.8 + (ax / cnt - me.vx) * 2.0;
                fy += cy / cnt * 0.8 + (ay / cnt - me.vy) * 2.0;
            }
            fx += sx * 0.006;
            fy += sy * 0.006;
            // Steer towards the goal at cruising speed.
            let (gx, gy) = (self.goal.0 - me.x, self.goal.1 - me.y);
            let gd = (gx * gx + gy * gy).sqrt().max(0.05);
            fx += (gx / gd * speed - me.vx) * 1.2;
            fy += (gy / gd * speed - me.vy) * 1.2;
            // Stay inside the water.
            let m = 0.15;
            if me.x < m {
                fx += (m - me.x) * 15.0;
            }
            if me.x > aspect - m {
                fx -= (me.x - aspect + m) * 15.0;
            }
            if me.y < m {
                fy += (m - me.y) * 15.0;
            }
            if me.y > 1.0 - m {
                fy -= (me.y - 1.0 + m) * 15.0;
            }
            *a = (fx, fy);
        }
        for (fi, &(ax, ay)) in self.fish.iter_mut().zip(&acc) {
            if turn != 0.0 {
                let (s, co) = (turn + fi.phase * 0.03).sin_cos();
                let (vx, vy) = (fi.vx, fi.vy);
                fi.vx = vx * co - vy * s;
                fi.vy = vy * co + vx * s;
            }
            fi.vx += ax * dt;
            // Fish prefer to swim level.
            fi.vy += (ay - fi.vy * 1.5) * dt;
            let v = (fi.vx * fi.vx + fi.vy * fi.vy).sqrt().max(1e-4);
            let target = speed * (0.85 + 0.3 * fi.depth);
            let k = (v + (target - v) * (1.0 - (-dt * 2.0).exp())) / v;
            fi.vx *= k;
            fi.vy *= k;
            fi.x = (fi.x + fi.vx * dt).clamp(-0.2, aspect + 0.2);
            fi.y = (fi.y + fi.vy * dt).clamp(-0.2, 1.2);
        }

        if c.w == 0 || c.h == 0 {
            return;
        }

        let bright = 0.8 + 0.3 * f.level;
        for y in 0..c.h {
            let g = y as f32 / h;
            let water = mix(rgb(0.1, 0.45, 0.65), rgb(0.0, 0.06, 0.2), g);
            for x in 0..c.w {
                let u = (x as f32 - y as f32 * 0.25) / h;
                let ray = ((u * 4.0 + now * 0.2).sin() * (u * 2.3 - now * 0.13).sin()).max(0.0);
                let col = mix(water, rgb(0.6, 0.9, 1.0), ray * ray * (1.0 - g) * 0.3);
                c.set(x, y, scale(col, bright));
            }
        }

        // Far fish first.
        let mut order: Vec<usize> = (0..self.fish.len()).collect();
        order.sort_by(|&a, &b| self.fish[a].depth.total_cmp(&self.fish[b].depth));
        for i in order {
            let fi = &self.fish[i];
            let v = (fi.vx * fi.vx + fi.vy * fi.vy).sqrt().max(1e-4);
            let (dx, dy) = (fi.vx / v, fi.vy / v);
            let len = (h * 0.065 * fi.depth).max(2.0);
            let (px, py) = (fi.x * h, fi.y * h);
            // Flanks flash when the fish banks out of the horizontal.
            let flash = (dy.abs() * 1.3 + self.flash * (0.5 + 0.5 * (fi.phase * 5.0).sin())).min(1.0);
            let body = mix(rgb(0.35, 0.5, 0.6), rgb(0.95, 1.0, 1.0), flash * 0.7 + 0.1);
            let body = mix(body, rgb(0.05, 0.25, 0.4), 1.0 - fi.depth);
            let wag = (now * 12.0 * (0.5 + v * 3.0) + fi.phase).sin();
            let r = len.ceil() as i32;
            let (ix, iy) = (px.round() as i32, py.round() as i32);
            for oy in -r..=r {
                for ox in -r..=r {
                    let (qx, qy) = ((ix + ox) as f32 - px, (iy + oy) as f32 - py);
                    let u = (qx * dx + qy * dy) / len;
                    let s = (-qx * dy + qy * dx) / len;
                    let inside_body = u > -0.3 && (u / 0.5).powi(2) + (s / 0.22).powi(2) < 1.0;
                    let tail_u = -u - 0.3;
                    let inside_tail = (0.0..0.35).contains(&tail_u) && (s - wag * tail_u * 0.5).abs() < tail_u * 0.7;
                    if inside_body {
                        // Darker back, light belly.
                        let k = 0.8 + 0.4 * s.signum() * dx.signum() * (s.abs() / 0.22);
                        c.plot(ix + ox, iy + oy, scale(body, k.clamp(0.5, 1.2)));
                    } else if inside_tail {
                        c.plot(ix + ox, iy + oy, scale(body, 0.7));
                    }
                }
            }
        }
    }
}
