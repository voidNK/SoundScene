use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv};

const SITES: usize = 14;

struct Site {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

/// Drifting Voronoi cells, each lit by its own slice of the spectrum.
/// Bass speeds up the drift, beats kick the sites around.
pub struct Voronoi {
    sites: Vec<Site>,
    rng: Rng,
    hue: f32,
}

impl Voronoi {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x7E55);
        let sites = (0..SITES)
            .map(|_| Site {
                x: rng.f32(),
                y: rng.f32(),
                vx: rng.range(-0.1, 0.1),
                vy: rng.range(-0.1, 0.1),
            })
            .collect();
        Self { sites, rng, hue: 0.0 }
    }
}

impl Effect for Voronoi {
    fn name(&self) -> &'static str {
        "Voronoi"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        let speed = 0.3 + f.bass * 2.0 + f.pulse;
        if f.beat {
            self.hue += 0.1;
        }
        for s in &mut self.sites {
            if f.beat {
                s.vx = (s.vx + self.rng.range(-0.05, 0.05)).clamp(-0.3, 0.3);
                s.vy = (s.vy + self.rng.range(-0.05, 0.05)).clamp(-0.3, 0.3);
            }
            s.x += s.vx * dt * speed;
            s.y += s.vy * dt * speed;
            if s.x < 0.0 || s.x > 1.0 {
                s.vx = -s.vx;
                s.x = s.x.clamp(0.0, 1.0);
            }
            if s.y < 0.0 || s.y > 1.0 {
                s.vy = -s.vy;
                s.y = s.y.clamp(0.0, 1.0);
            }
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let size = w.min(h).max(1.0);
        let points: Vec<(f32, f32)> = self.sites.iter().map(|s| (s.x * w, s.y * h)).collect();
        let bins = f.spectrum.len();

        for y in 0..c.h {
            let fy = y as f32;
            for x in 0..c.w {
                let fx = x as f32;
                let (mut d1, mut d2, mut nearest) = (f32::MAX, f32::MAX, 0);
                for (i, &(px, py)) in points.iter().enumerate() {
                    let d = (fx - px).powi(2) + (fy - py).powi(2);
                    if d < d1 {
                        (d2, d1, nearest) = (d1, d, i);
                    } else if d < d2 {
                        d2 = d;
                    }
                }
                let (d1, d2) = (d1.sqrt(), d2.sqrt());
                let hue = self.hue + nearest as f32 * 0.07;
                let col = if d2 - d1 < 1.2 {
                    hsv(hue, 0.2, 0.9)
                } else {
                    let v = f.spectrum[(nearest * 5) % bins];
                    hsv(hue, 0.65, (0.2 + 0.7 * v) * (1.0 - (d1 / (size * 0.6)).min(0.8)))
                };
                c.set(x, y, col);
            }
        }
    }
}
