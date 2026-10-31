use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, rgb};

const MAX_DRIPS: usize = 90;

struct Drip {
    x: f32,
    y: f32,
    vy: f32,
    r: f32,
    col: [f32; 3],
    /// Phase of the stop-and-go wobble.
    wob: f32,
}

/// Paint running down from a wet band along the top edge: drips crawl down
/// in stop-and-go runs with bulging heads and merge where they touch.
/// The level sets the drip rate, beats pour a burst of new runs.
pub struct Drips {
    paint: Vec<[f32; 3]>,
    thick: Vec<f32>,
    w: usize,
    h: usize,
    drips: Vec<Drip>,
    rng: Rng,
    acc: f32,
    time: f32,
    hue: f32,
}

impl Drips {
    pub fn new() -> Self {
        Self {
            paint: Vec::new(),
            thick: Vec::new(),
            w: 0,
            h: 0,
            drips: Vec::new(),
            rng: Rng::new(0xD819),
            acc: 0.0,
            time: 0.0,
            hue: 0.0,
        }
    }

    /// Depth of the wet top band at column `x`, in pixels.
    fn band(&self, x: f32) -> f32 {
        let (w, h) = (self.w as f32, self.h as f32);
        let u = x / w.max(1.0);
        h * (0.07 + 0.025 * (u * 17.0 + self.time * 0.4).sin() + 0.02 * (u * 41.0 - self.time * 0.7).sin())
    }

    fn band_color(&self, x: f32) -> [f32; 3] {
        let u = x / (self.w as f32).max(1.0);
        let h = self.hue + 0.5 * (u * 2.3 + self.time * 0.05).sin() + u * 0.4;
        hsv(h, 0.85, 0.95).map(|v| v as f32 / 255.0)
    }

    fn spawn(&mut self) {
        if self.drips.len() >= MAX_DRIPS {
            return;
        }
        let x = self.rng.range(0.0, self.w as f32);
        let r = (self.w.min(self.h * 2) as f32 * self.rng.range(0.005, 0.02)).max(0.9);
        let col = self.band_color(x);
        let y = self.band(x);
        self.drips.push(Drip {
            x,
            y,
            vy: 0.0,
            r,
            col,
            wob: self.rng.range(0.0, 6.3),
        });
    }

    /// Paints a disc of wet paint into the persistent layer.
    fn stamp(&mut self, x: f32, y: f32, r: f32, col: [f32; 3]) {
        let (w, h) = (self.w as i32, self.h as i32);
        let reach = r.ceil() as i32 + 1;
        for py in (y as i32 - reach).max(0)..=(y as i32 + reach).min(h - 1) {
            for px in (x as i32 - reach).max(0)..=(x as i32 + reach).min(w - 1) {
                let d = ((px as f32 + 0.5 - x).powi(2) + (py as f32 + 0.5 - y).powi(2)).sqrt();
                let a = (r + 0.5 - d).clamp(0.0, 1.0);
                if a <= 0.0 {
                    continue;
                }
                let i = py as usize * self.w + px as usize;
                let keep = if self.thick[i] < 0.1 { 1.0 - a } else { 1.0 - 0.5 * a };
                for (p, c) in self.paint[i].iter_mut().zip(col) {
                    *p = *p * keep + c * (1.0 - keep);
                }
                self.thick[i] = self.thick[i].max(a);
            }
        }
    }
}

impl Effect for Drips {
    fn name(&self) -> &'static str {
        "Drips"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        if (c.w, c.h) != (self.w, self.h) {
            (self.w, self.h) = (c.w, c.h);
            self.paint = vec![[0.0; 3]; c.w * c.h];
            self.thick = vec![0.0; c.w * c.h];
            self.drips.clear();
        }
        let (w, h) = (c.w, c.h);
        self.time += dt;
        self.hue += dt * 0.015 + if f.beat { 0.09 } else { 0.0 };

        self.acc += dt * (0.5 + 3.0 * f.level) * (w as f32 / 120.0).clamp(0.3, 2.0);
        if f.beat {
            self.acc += 0.5 + 1.5 * f.bass;
        }
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            self.spawn();
        }

        // Old runs dry and fade so the wall never fills up.
        let dry = (-dt * 0.12).exp();
        for t in &mut self.thick {
            *t *= dry;
        }

        // The wet band along the top edge.
        for x in 0..w {
            let depth = (self.band(x as f32) as usize).min(h);
            let col = self.band_color(x as f32);
            for y in 0..depth {
                let i = y * w + x;
                self.paint[i] = col;
                self.thick[i] = 1.0;
            }
        }

        // Gravity with a stop-and-go wobble, slower for thinner runs.
        let g = h as f32 * (0.07 + 0.2 * f.bass);
        let mut drips = std::mem::take(&mut self.drips);
        for d in &mut drips {
            d.wob += dt * 2.3;
            let target = g * d.r.sqrt() * (0.55 + 0.45 * d.wob.sin()).max(0.05);
            d.vy += (target - d.vy) * (1.0 - (-dt * 3.0).exp());
            let y0 = d.y;
            d.y += d.vy * dt;
            d.r -= dt * (0.04 * d.r + 0.03);
            let steps = (d.y - y0).ceil().clamp(1.0, 200.0) as usize;
            for k in 0..=steps {
                let y = y0 + (d.y - y0) * k as f32 / steps as f32;
                self.stamp(d.x, y, d.r, d.col);
            }
        }
        drips.retain(|d| d.r > 0.45 && d.y < h as f32 + d.r * 2.0);
        self.drips = drips;

        // Wall, paint with a highlight on its left flank and shadow on the right.
        let bright = 0.85 + 0.25 * f.pulse;
        for y in 0..h {
            let wall = 0.10 + 0.04 * (y as f32 / h as f32);
            for x in 0..w {
                let i = y * w + x;
                let a = self.thick[i].min(1.0);
                let l = if x > 0 { self.thick[i - 1] } else { a };
                let r = if x + 1 < w { self.thick[i + 1] } else { a };
                let hl = (a - l).max(0.0) * 0.45 - (a - r).max(0.0) * 0.25;
                let p = self.paint[i];
                c.set(
                    x,
                    y,
                    rgb(
                        wall * (1.0 - a) + (p[0] * bright + hl) * a,
                        wall * 0.95 * (1.0 - a) + (p[1] * bright + hl) * a,
                        wall * 0.9 * (1.0 - a) + (p[2] * bright + hl) * a,
                    ),
                );
            }
        }

        // Bulging, glossy heads (drawn only, not left behind).
        for d in &self.drips {
            let rb = d.r * 1.5 + 0.4;
            let reach = rb.ceil() as i32 + 1;
            let (cx, cy) = (d.x, d.y + d.r * 0.3);
            for py in (cy as i32 - reach).max(0)..=(cy as i32 + reach).min(h as i32 - 1) {
                for px in (cx as i32 - reach).max(0)..=(cx as i32 + reach).min(w as i32 - 1) {
                    let (dx, dy) = (px as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                    let dist = (dx * dx + dy * dy).sqrt();
                    let a = (rb + 0.5 - dist).clamp(0.0, 1.0);
                    if a <= 0.0 {
                        continue;
                    }
                    // Specular spot towards the upper left.
                    let spec = (1.0 - ((dx + rb * 0.4).powi(2) + (dy + rb * 0.4).powi(2)).sqrt() / rb.max(0.5))
                        .max(0.0)
                        .powi(2);
                    let col = [
                        d.col[0] * bright + spec,
                        d.col[1] * bright + spec,
                        d.col[2] * bright + spec,
                    ];
                    let old = c.get(px as usize, py as usize);
                    c.set(
                        px as usize,
                        py as usize,
                        rgb(
                            old[0] as f32 / 255.0 * (1.0 - a) + col[0] * a,
                            old[1] as f32 / 255.0 * (1.0 - a) + col[1] * a,
                            old[2] as f32 / 255.0 * (1.0 - a) + col[2] * a,
                        ),
                    );
                }
            }
        }
    }
}
