use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, rgb};

/// Fern height in IFS units (the attractor spans y = 0..10).
const HEIGHT: f32 = 10.0;
const LIFE: f32 = 40.0;
const GROW: f32 = 7.0;

/// One fern of the group: horizontal position, size, mirror, phase.
const FERNS: [(f32, f32, f32, f32); 3] = [(0.5, 1.0, 1.0, 0.0), (0.2, 0.62, -1.0, 1.7), (0.8, 0.7, 1.0, 3.1)];

/// Barnsley ferns grown from the IFS chaos game. They unroll from the stem
/// upwards, bend in a wind driven by the loudness, shift their green with the
/// spectrum and light up their tips on beats. After a while they wilt and
/// grow again.
pub struct Fern {
    age: f32,
    wind: f32,
    hue: f32,
    rng: Rng,
}

impl Fern {
    pub fn new() -> Self {
        Self {
            age: 0.0,
            wind: 0.0,
            hue: 0.0,
            rng: Rng::new(0xFE2),
        }
    }
}

impl Effect for Fern {
    fn name(&self) -> &'static str {
        "Fern"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.age += dt;
        if self.age > LIFE {
            self.age = 0.0;
        }
        self.wind += (f.level - self.wind) * (1.0 - (-2.0 * dt).exp());
        let bins = f.spectrum.len();
        let mids = if bins > 0 {
            f.spectrum[bins / 4..bins / 2].iter().sum::<f32>() / (bins / 2 - bins / 4).max(1) as f32
        } else {
            0.0
        };
        self.hue += (mids * 0.12 - self.hue) * (1.0 - (-dt).exp());

        let (w, h) = (c.w, c.h);
        for y in 0..h {
            let g = y as f32 / h.max(1) as f32;
            let col = rgb(0.01, 0.02 + 0.04 * g, 0.03 + 0.03 * g);
            for x in 0..w {
                c.set(x, y, col);
            }
        }

        // Grow upwards, wilt (fade) at the end of the life cycle.
        let grow = (self.age / GROW).min(1.0);
        let reach = HEIGHT * grow * (2.0 - grow);
        let life = ((LIFE - self.age) / 2.0).clamp(0.0, 1.0);
        let unit = h as f32 / HEIGHT * 0.95;
        let points = (w * h * 4).clamp(200, 60_000);

        for (k, &(fx, size, mirror, phase)) in FERNS.iter().enumerate() {
            // Wind turns the frond map, which curls the fern towards its tip.
            let gust = (t * 0.9 + phase).sin() * 0.02 + (t * 2.3 + phase).sin() * 0.008;
            let bend = mirror * (gust + 0.05 * self.wind) + if k == 0 { 0.0 } else { -0.01 * mirror };
            let (bs, bc) = bend.sin_cos();
            let (a, b) = (0.85 * bc + 0.04 * bs, 0.04 * bc - 0.85 * bs);
            let (cc, d) = (0.85 * bs - 0.04 * bc, 0.04 * bs + 0.85 * bc);
            let n = (points as f32 * size * size) as usize / 2 + 50;
            let (ox, oy) = (w as f32 * fx, h as f32 - 1.0);
            let s = unit * size;
            let (mut x, mut y) = (0.0f32, 0.0f32);
            let base_hue = 0.27 + self.hue + k as f32 * 0.03;
            for i in 0..n {
                let r = self.rng.f32();
                (x, y) = if r < 0.01 {
                    (0.0, 0.16 * y)
                } else if r < 0.86 {
                    (a * x + b * y, cc * x + d * y + 1.6)
                } else if r < 0.93 {
                    (0.2 * x - 0.26 * y, 0.23 * x + 0.22 * y + 1.6)
                } else {
                    (-0.15 * x + 0.28 * y, 0.26 * x + 0.24 * y + 0.44)
                };
                if i < 8 || y > reach || !x.is_finite() || !y.is_finite() {
                    continue;
                }
                let u = y / HEIGHT;
                let tip = (u * u * f.pulse).min(1.0);
                let col = hsv(
                    base_hue - 0.08 * u,
                    0.85 - 0.5 * tip,
                    (0.35 + 0.45 * u + 0.4 * tip) * life,
                );
                let px = ox + x * s * mirror;
                let py = oy - y * s;
                c.plot(px as i32, py as i32, col);
            }
        }
    }
}
