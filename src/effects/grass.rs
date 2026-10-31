use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, mix, rgb, scale};
use std::f32::consts::TAU;

const LAYERS: usize = 4;
const BLADES: usize = 220;
const SEEDS: usize = 40;

struct Blade {
    /// Fraction of the width.
    x: f32,
    /// Fraction of the layer's height.
    height: f32,
    phase: f32,
    shade: f32,
}

/// A meadow under a summer sky: four depth layers of grass blades sway in
/// wind waves that roll across the field. Bass and beats blow gusts that bend
/// the grass flat and set seeds flying; the level brightens the sky.
pub struct Grass {
    layers: Vec<Vec<Blade>>,
    /// Flying seeds as (x, y, phase), fractions of the canvas.
    seeds: Vec<(f32, f32, f32)>,
    wind: f32,
    gust: f32,
    /// Position of the wind waves, advances with the wind.
    travel: f32,
    time: f32,
}

impl Grass {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x6_7A55);
        let layers = (0..LAYERS)
            .map(|_| {
                (0..BLADES)
                    .map(|_| Blade {
                        x: rng.range(-0.05, 1.05),
                        height: rng.range(0.55, 1.0),
                        phase: rng.range(0.0, TAU),
                        shade: rng.f32(),
                    })
                    .collect()
            })
            .collect();
        let seeds = (0..SEEDS)
            .map(|_| (rng.f32(), rng.range(0.3, 0.9), rng.range(0.0, TAU)))
            .collect();
        Self {
            layers,
            seeds,
            wind: 0.2,
            gust: 0.0,
            travel: 0.0,
            time: 0.0,
        }
    }

    /// Bend of the grass at fraction `u` across the field, roughly -1..1.5.
    fn bend(&self, u: f32, depth: f32, phase: f32) -> f32 {
        let k = 4.0 + 3.0 * depth;
        let wave = (u * k - self.travel).sin() * 0.6 + (u * k * 2.3 - self.travel * 1.7 + 1.0).sin() * 0.25;
        let flutter = (self.time * 3.0 + phase).sin() * 0.08;
        self.wind * (0.6 + 0.6 * wave) + flutter
    }
}

impl Effect for Grass {
    fn name(&self) -> &'static str {
        "Grass"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        if f.beat {
            self.gust += 0.3 + 0.5 * f.bass;
        }
        self.gust = (self.gust * (-dt * 1.2).exp()).min(1.2);
        let target = 0.25 + 0.5 * f.bass + self.gust;
        self.wind += (target - self.wind) * (1.0 - (-dt * 3.0).exp());
        self.travel += dt * (1.0 + 3.0 * self.wind);
        let wind = self.wind;
        for s in &mut self.seeds {
            s.0 += dt * (0.02 + 0.25 * wind);
            s.1 += dt * 0.03 * (self.time * 1.3 + s.2).sin() - dt * 0.02 * wind;
            if s.0 > 1.05 || s.1 < 0.0 || s.1 > 1.0 {
                *s = (-0.05, 0.4 + 0.5 * (s.2 * 7.3 + self.time).sin().abs(), s.2);
            }
        }

        if c.w == 0 || c.h == 0 {
            return;
        }
        let (w, h) = (c.w as f32, c.h as f32);

        // Sky, sun haze and a far line of hills.
        let horizon = h * 0.45;
        let sun = (w * 0.75, h * 0.12);
        let bright = 0.85 + 0.15 * f.level;
        for y in 0..c.h {
            let g = (y as f32 / horizon.max(1.0)).min(1.0);
            let sky = mix(rgb(0.2, 0.45, 0.85), rgb(0.75, 0.88, 0.95), g * g);
            for x in 0..c.w {
                let (dx, dy) = (x as f32 - sun.0, y as f32 - sun.1);
                let d = (dx * dx + dy * dy).sqrt() / h;
                let col = mix(
                    sky,
                    rgb(1.0, 0.98, 0.85),
                    (-d * 8.0).exp() + if d < 0.05 { 1.0 } else { 0.0 },
                );
                c.set(x, y, scale(col, bright));
            }
        }
        for x in 0..c.w {
            let u = x as f32 / h;
            let top = horizon - h * (0.04 + 0.03 * (u * 2.1).sin() + 0.015 * (u * 5.3 + 1.0).sin());
            for y in (top.max(0.0) as usize).min(c.h)..c.h {
                c.set(x, y, rgb(0.35, 0.5, 0.45));
            }
        }

        // Layers from far to near, each a band of ground with blades on it.
        for (li, layer) in self.layers.iter().enumerate() {
            let depth = li as f32 / (LAYERS - 1) as f32;
            let base = h * (0.5 + 0.17 * li as f32);
            let tall = h * (0.08 + 0.2 * depth * depth);
            let haze = 0.55 * (1.0 - depth);
            let ground = mix(rgb(0.15, 0.35, 0.08), rgb(0.45, 0.6, 0.5), haze);
            for y in (base.max(0.0) as usize).min(c.h)..c.h {
                let k = 1.0 - 0.3 * ((y as f32 - base) / h).min(1.0);
                for x in 0..c.w {
                    c.set(x, y, scale(ground, k));
                }
            }
            let thick = if depth > 0.6 && h > 50.0 { 1 } else { 0 };
            let count = ((w * (0.5 + depth)) as usize).clamp(8, BLADES);
            for b in layer.iter().take(count) {
                let bend = self.bend(b.x, depth, b.phase);
                let len = tall * b.height;
                let steps = len.ceil().clamp(1.0, 200.0) as usize;
                let lean = len * bend * 0.9;
                let dark = mix(rgb(0.05, 0.25, 0.03), rgb(0.12, 0.4, 0.06), b.shade);
                let tip = mix(rgb(0.45, 0.75, 0.2), rgb(0.75, 0.85, 0.35), b.shade);
                let sheen = (bend - 0.5).max(0.0) * 0.5;
                let (dark, tip) = (mix(dark, ground, haze), mix(tip, ground, haze * 0.8));
                let bx = b.x * w;
                for s in 0..=steps {
                    let v = s as f32 / steps as f32;
                    let x = bx + lean * v * v;
                    // Bending shortens the blade.
                    let y = base - len * v * (1.0 - 0.3 * (bend * v).abs().min(1.0));
                    let col = mix(mix(dark, tip, v), rgb(0.95, 1.0, 0.7), sheen * v);
                    let (xi, yi) = (x.round() as i32, y.round() as i32);
                    c.plot(xi, yi, col);
                    if thick > 0 && v < 0.6 {
                        c.plot(xi + 1, yi, col);
                    }
                }
            }
        }

        for &(sx, sy, ph) in &self.seeds {
            let (x, y) = ((sx * w) as i32, (sy * h) as i32);
            let k = 0.5 + 0.3 * (ph + self.time * 5.0).sin();
            c.plot_add(x, y, scale(rgb(1.0, 1.0, 0.9), k));
        }
    }
}
