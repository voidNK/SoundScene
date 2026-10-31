use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, add, hsv, scale};
use std::f32::consts::{PI, TAU};

const LAYERS: usize = 10;
const MORPH_TIME: f32 = 0.8;

/// Additive antialiased line `width` pixels wide with butt ends, so the
/// segments of a polyline join without bright seams. Lines thinner than a
/// pixel are drawn one pixel wide and dimmed instead.
pub(super) fn stroke(c: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, col: Rgb) {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let len = (dx * dx + dy * dy).sqrt();
    if c.w == 0 || c.h == 0 || !len.is_finite() || len < 1e-4 || width.is_nan() || width <= 0.0 {
        return;
    }
    let col = if width < 1.0 { scale(col, width) } else { col };
    let hw = (width * 0.5).max(0.5);
    let (ux, uy) = (dx / len, dy / len);
    // Walk along the major axis and cover a short span on the minor axis.
    let steep = dy.abs() > dx.abs();
    let (a0, a1, b0, b1, amax, bmax) = if steep {
        (y0, y1, x0, x1, c.h, c.w)
    } else {
        (x0, x1, y0, y1, c.w, c.h)
    };
    let slope = (b1 - b0) / (a1 - a0);
    let span = (hw + 1.0) * len / (a1 - a0).abs();
    let lo = (a0.min(a1) - hw - 1.0).max(0.0) as usize;
    let hi = ((a0.max(a1) + hw + 1.0).min(amax as f32 - 1.0)).max(-1.0);
    if hi < 0.0 || span > 1.0e4 {
        return;
    }
    for a in lo..=hi as usize {
        let ac = a as f32 + 0.5;
        let bc = b0 + (ac - a0) * slope;
        let blo = (bc - span).max(0.0) as usize;
        let bhi = (bc + span).min(bmax as f32 - 1.0);
        if bhi < 0.0 {
            continue;
        }
        for b in blo..=bhi as usize {
            let (x, y) = if steep { (b, a) } else { (a, b) };
            let (qx, qy) = (x as f32 + 0.5 - x0, y as f32 + 0.5 - y0);
            let along = qx * ux + qy * uy;
            let perp = (qx * uy - qy * ux).abs();
            let cov = (hw + 0.5 - perp).clamp(0.0, 1.0) * (along + 0.5).min(len - along + 0.5).clamp(0.0, 1.0);
            if cov > 0.0 {
                let i = y * c.w + x;
                c.px[i] = add(c.px[i], scale(col, cov));
            }
        }
    }
}

/// Distance from the center to the outline of a regular `n`-gon with
/// circumradius 1, in direction `a` (a vertex sits at angle 0).
fn polygon_radius(n: f32, a: f32) -> f32 {
    let sector = TAU / n;
    let local = a.rem_euclid(sector) - sector * 0.5;
    (PI / n).cos() / local.cos()
}

struct Layer {
    from: u32,
    to: u32,
    morph: f32,
    angle: f32,
    speed: f32,
}

/// Nested concentric regular polygons, each layer spinning at its own speed
/// and direction and morphing into a new side count now and then (and on
/// beats). Bass speeds up the spin, the pulse pumps the layers outwards in a
/// wave and the spectrum lights them from bass (outside) to treble (inside).
pub struct Polygons {
    layers: Vec<Layer>,
    hue: f32,
    idle: f32,
    next: usize,
    rng: Rng,
}

impl Polygons {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x9017);
        let layers = (0..LAYERS)
            .map(|i| {
                let n = 3 + rng.next_u32() % 6;
                let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
                Layer {
                    from: n,
                    to: n,
                    morph: 1.0,
                    angle: i as f32 * 0.3,
                    speed: dir * rng.range(0.15, 0.6),
                }
            })
            .collect();
        Self {
            layers,
            hue: 0.0,
            idle: 0.0,
            next: 0,
            rng,
        }
    }

    fn retarget(&mut self) {
        let i = self.next % LAYERS;
        // Step through the layers in a scattered order.
        self.next += 3;
        self.idle = 0.0;
        let l = &mut self.layers[i];
        let mut n = 3 + self.rng.next_u32() % 6;
        if n == l.to {
            n = 3 + (n - 2) % 6;
        }
        l.from = if l.morph < 0.5 { l.from } else { l.to };
        l.to = n;
        l.morph = 0.0;
    }
}

impl Effect for Polygons {
    fn name(&self) -> &'static str {
        "Polygons"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fill([3, 2, 8]);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.idle += dt;
        if (f.beat && self.idle > 0.4) || self.idle > 1.5 {
            self.retarget();
        }
        self.hue += dt * (0.02 + 0.08 * f.treble) + if f.beat { 0.04 } else { 0.0 };
        for l in &mut self.layers {
            l.morph = (l.morph + dt / MORPH_TIME).min(1.0);
            l.angle += dt * l.speed * (1.0 + 2.0 * f.bass);
        }

        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let outer = c.w.min(c.h) as f32 * 0.5 * 0.96;
        let width = (outer / 40.0).max(0.8);
        let bins = f.spectrum.len();
        for (i, l) in self.layers.iter().enumerate() {
            let depth = i as f32 / LAYERS as f32;
            // The pulse travels inwards as a wave.
            let wave = (f.pulse * (1.0 - depth * 0.6)).max(0.0) * (t * 6.0 - depth * 4.0).sin().abs();
            let band = if bins > 0 {
                let b = (depth * (bins - 1) as f32) as usize;
                f.spectrum[b.min(bins - 1)]
            } else {
                0.0
            };
            let r = outer * 0.84f32.powi(i as i32) * (1.0 + 0.1 * wave + 0.05 * band);
            let m = l.morph * l.morph * (3.0 - 2.0 * l.morph);
            let points = if m >= 1.0 { l.to as usize } else { 120 };
            let point = |k: usize| {
                let a = TAU * k as f32 / points as f32;
                let rad = if m >= 1.0 {
                    1.0
                } else {
                    let ra = polygon_radius(l.from as f32, a);
                    ra + (polygon_radius(l.to as f32, a) - ra) * m
                };
                let a = a + l.angle;
                (cx + a.cos() * r * rad, cy + a.sin() * r * rad)
            };
            let value = 0.35 + 0.5 * band + 0.25 * f.pulse;
            let col = hsv(self.hue + depth * 0.55, 0.8, value.min(1.0));
            let glow = scale(col, 0.22);
            let lw = width * (1.0 - depth * 0.5);
            let mut prev = point(0);
            for k in 1..=points {
                let p = point(k % points);
                stroke(c, prev.0, prev.1, p.0, p.1, lw * 3.0, glow);
                stroke(c, prev.0, prev.1, p.0, p.1, lw, col);
                prev = p;
            }
        }
    }
}
