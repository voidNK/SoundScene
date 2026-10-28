use super::{Effect, digits};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, scale};

/// Paddle half height, as a fraction of the field height.
const HALF: f32 = 0.11;
const MAX_SPARKS: usize = 200;
const WIN: u32 = 9;

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
}

/// Two bots playing a bat-and-ball game, with a glowing ball trail and the
/// score on top. Bass speeds up the ball, every beat flashes the net, and
/// paddle hits throw sparks. Positions are kept in field units (0..1) so the
/// game survives any resize.
pub struct Pong {
    ball: (f32, f32),
    vel: (f32, f32),
    paddles: [f32; 2],
    /// Where each bot plans to meet the ball, relative to its paddle center.
    aim: [f32; 2],
    score: [u32; 2],
    serve: f32,
    flash: [f32; 2],
    sparks: Vec<Spark>,
    rng: Rng,
    hue: f32,
}

impl Pong {
    pub fn new() -> Self {
        let mut p = Self {
            ball: (0.5, 0.5),
            vel: (0.0, 0.0),
            paddles: [0.5; 2],
            aim: [0.0; 2],
            score: [0; 2],
            serve: 0.0,
            flash: [0.0; 2],
            sparks: Vec::new(),
            rng: Rng::new(0x9096),
            hue: 0.0,
        };
        p.serve_to(1);
        p
    }

    fn serve_to(&mut self, side: usize) {
        self.ball = (0.5, self.rng.range(0.3, 0.7));
        let dir = if side == 0 { -1.0 } else { 1.0 };
        self.vel = (dir * 0.45, self.rng.range(-0.45, 0.45));
        self.serve = 0.7;
        self.reaim();
    }

    /// New random aim point; sometimes outside the paddle, so the bot misses.
    fn reaim(&mut self) {
        for a in &mut self.aim {
            *a = self.rng.range(-1.25, 1.25) * HALF;
        }
    }

    /// Where the ball will cross `x`, with wall bounces folded in.
    fn intercept(&self, x: f32) -> f32 {
        let (bx, by) = self.ball;
        let (vx, vy) = self.vel;
        if vx.abs() < 1e-4 {
            return by;
        }
        let y = by + vy * ((x - bx) / vx);
        let m = y.rem_euclid(2.0);
        if m > 1.0 { 2.0 - m } else { m }
    }

    fn sparks_at(&mut self, x: f32, y: f32, dir: f32) {
        for _ in 0..20 {
            if self.sparks.len() >= MAX_SPARKS {
                break;
            }
            let a = self.rng.range(-1.2, 1.2);
            let s = self.rng.range(0.2, 0.9);
            self.sparks.push(Spark {
                x,
                y,
                vx: dir * a.cos() * s,
                vy: a.sin() * s,
                life: 1.0,
            });
        }
    }

    fn update(&mut self, dt: f32, f: &Features, px: f32) {
        for fl in &mut self.flash {
            *fl = (*fl - dt * 2.0).max(0.0);
        }
        for s in &mut self.sparks {
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.vy += dt * 0.8;
            s.life -= dt * 2.0;
        }
        self.sparks.retain(|s| s.life > 0.0);

        let paddle_x = [3.0 * px, 1.0 - 3.0 * px];
        let speed = 1.2 + f.level * 0.6;
        for (side, &x) in paddle_x.iter().enumerate() {
            let coming = (self.vel.0 < 0.0) == (side == 0);
            let goal = if coming {
                self.intercept(x) - self.aim[side]
            } else {
                0.5
            };
            let d = goal - self.paddles[side];
            self.paddles[side] += d.clamp(-speed * dt, speed * dt);
            self.paddles[side] = self.paddles[side].clamp(HALF, 1.0 - HALF);
        }

        if self.serve > 0.0 {
            self.serve -= dt;
            return;
        }
        let boost = 0.7 + f.bass * 1.1 + f.pulse * 0.3;
        let (dx, dy) = (self.vel.0 * boost * dt, self.vel.1 * boost * dt);
        // Sub-steps of at most one pixel so the ball can't tunnel a paddle.
        let steps = ((dx.abs().max(dy.abs()) / px.max(1e-4)).ceil() as usize).clamp(1, 400);
        for _ in 0..steps {
            self.ball.0 += dx / steps as f32;
            self.ball.1 += dy / steps as f32;
            if self.ball.1 < 0.0 || self.ball.1 > 1.0 {
                self.ball.1 = self.ball.1.clamp(0.0, 1.0);
                self.vel.1 = -self.vel.1;
            }
            for side in 0..2 {
                let incoming = if side == 0 { self.vel.0 < 0.0 } else { self.vel.0 > 0.0 };
                let crossed = if side == 0 {
                    self.ball.0 <= paddle_x[0]
                } else {
                    self.ball.0 >= paddle_x[1]
                };
                if incoming && crossed && (self.ball.1 - self.paddles[side]).abs() < HALF + px {
                    let off = (self.ball.1 - self.paddles[side]) / HALF;
                    let vx = (self.vel.0.abs() * 1.06).min(1.6);
                    self.vel = (if side == 0 { vx } else { -vx }, self.vel.1 * 0.5 + off * vx * 0.9);
                    self.reaim();
                    self.hue += 0.05;
                    self.sparks_at(paddle_x[side], self.ball.1, if side == 0 { 1.0 } else { -1.0 });
                }
            }
            if self.ball.0 < 0.0 || self.ball.0 > 1.0 {
                let winner = if self.ball.0 < 0.0 { 1 } else { 0 };
                self.score[winner] += 1;
                self.flash[winner] = 1.0;
                if self.score[winner] > WIN {
                    self.score = [0; 2];
                }
                self.serve_to(1 - winner);
                return;
            }
        }
    }
}

impl Effect for Pong {
    fn name(&self) -> &'static str {
        "Pong"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        let (w, h) = (c.w as f32, c.h as f32);
        let px = 1.0 / w.max(1.0);
        self.update(dt.min(0.1), f, px);
        // The fade leaves a trail behind the ball.
        c.fade(0.72);
        if c.w < 6 || c.h < 4 {
            return;
        }

        let net = mix(hsv(self.hue, 0.3, 0.25), hsv(self.hue + 0.5, 0.6, 1.0), f.pulse);
        let dash = (c.h / 24).max(1);
        let nx = c.w / 2;
        for y in 0..c.h {
            if (y / dash).is_multiple_of(2) {
                c.set(nx, y, net);
            }
        }

        let ds = (c.h / 28).max(1) as i32;
        for side in 0..2 {
            let n = self.score[side];
            let col = mix(
                hsv(0.55 + side as f32 * 0.35, 0.5, 0.7),
                [255, 255, 255],
                self.flash[side],
            );
            let gap = 3 * ds;
            let x = if side == 0 {
                nx as i32 - gap - digits::width(n, ds)
            } else {
                nx as i32 + gap + 1
            };
            digits::draw(c, x, 2 * ds, n, ds, col);
        }

        for s in &self.sparks {
            let col = scale(hsv(self.hue + 0.1, 0.6, 1.0), s.life.clamp(0.0, 1.0));
            c.plot_add((s.x * w) as i32, (s.y * h) as i32, col);
        }

        let thick = (c.w / 120).max(1) as i32;
        for side in 0..2 {
            let col = hsv(0.55 + side as f32 * 0.35, 0.7, 1.0);
            let x = if side == 0 { 2 } else { c.w as i32 - 3 - (thick - 1) };
            let (top, bot) = ((self.paddles[side] - HALF) * h, (self.paddles[side] + HALF) * h);
            for y in top.round() as i32..bot.round() as i32 {
                for k in 0..thick {
                    c.plot(x + k, y, col);
                }
            }
        }

        let bs = (c.h / 45).max(1) as i32;
        let speed = self.vel.0.abs();
        let ball = mix([255, 255, 255], hsv(self.hue, 0.8, 1.0), (speed - 0.45) * 1.5);
        let (bx, by) = ((self.ball.0 * w) as i32, (self.ball.1 * h) as i32);
        for dy in 0..=bs {
            for dx in 0..=bs {
                c.plot(bx + dx - bs / 2, by + dy - bs / 2, ball);
            }
        }
    }
}
