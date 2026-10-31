use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, rgb};

const DISCS: usize = 38;

struct Disc {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    /// Radius as a fraction of the shorter canvas side.
    r: f32,
    hue: f32,
    /// 0 = far (small, dim), 1 = near (large, swells most).
    depth: f32,
    band: usize,
}

/// Soft out-of-focus light discs drifting and overlapping additively, like a
/// defocused night street. Bass swells the discs, every disc follows its own
/// band and beats brighten them.
pub struct Bokeh {
    discs: Vec<Disc>,
    acc: Vec<[f32; 3]>,
    hue: f32,
}

impl Bokeh {
    pub fn new() -> Self {
        let mut rng = Rng::new(0xB0CE);
        let discs = (0..DISCS)
            .map(|i| {
                let depth = rng.f32();
                Disc {
                    x: rng.f32(),
                    y: rng.f32(),
                    vx: rng.range(-0.02, 0.02),
                    vy: rng.range(-0.035, -0.005),
                    r: 0.04 + 0.12 * depth,
                    hue: [0.08, 0.12, 0.55, 0.62, 0.9, 0.0][i % 6] + rng.range(-0.04, 0.04),
                    depth,
                    band: (i * 7) % 48,
                }
            })
            .collect();
        Self {
            discs,
            acc: Vec::new(),
            hue: 0.0,
        }
    }
}

impl Effect for Bokeh {
    fn name(&self) -> &'static str {
        "Bokeh"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let (w, h) = (c.w, c.h);
        if self.acc.len() != w * h {
            self.acc = vec![[0.0; 3]; w * h];
        }
        self.hue += dt * 0.01 + if f.beat { 0.03 } else { 0.0 };

        // Deep blue-violet night gradient.
        for (i, a) in self.acc.iter_mut().enumerate() {
            let v = (i / w) as f32 / h as f32;
            *a = [0.02 + 0.03 * v, 0.015 + 0.01 * v, 0.06 - 0.02 * v];
        }

        let (wf, hf) = (w as f32, h as f32);
        let s = wf.min(hf);
        let drift = 1.0 + 1.5 * f.level;
        // Far discs first so the near ones sit on top in the blend.
        self.discs
            .sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap_or(std::cmp::Ordering::Equal));
        for (n, d) in self.discs.iter_mut().enumerate() {
            d.x += d.vx * dt * drift * (0.5 + d.depth);
            d.y += d.vy * dt * drift * (0.5 + d.depth);
            let m = d.r * s / wf.max(1.0) + 0.05;
            if d.x < -m {
                d.x += 1.0 + 2.0 * m;
            } else if d.x > 1.0 + m {
                d.x -= 1.0 + 2.0 * m;
            }
            let my = d.r * s / hf.max(1.0) + 0.05;
            if d.y < -my {
                d.y += 1.0 + 2.0 * my;
            } else if d.y > 1.0 + my {
                d.y -= 1.0 + 2.0 * my;
            }

            let band = f.spectrum.get(d.band).copied().unwrap_or(0.0);
            let breathe = 1.0 + 0.06 * (t * 0.7 + n as f32).sin();
            let r = (d.r * s * breathe * (1.0 + (0.5 * f.bass + 0.35 * f.pulse) * (0.3 + d.depth))).max(0.8);
            let alpha = (0.2 + 0.25 * (1.0 - d.depth)) * (0.5 + 0.9 * band + 0.4 * f.pulse);
            let col = hsv(d.hue + self.hue, 0.7 + 0.25 * d.depth, 1.0).map(|v| v as f32 / 255.0 * alpha);

            let (cx, cy) = (d.x * wf, d.y * hf);
            let x0 = (cx - r - 1.0).max(0.0) as usize;
            let y0 = (cy - r - 1.0).max(0.0) as usize;
            let x1 = ((cx + r + 1.0).max(0.0) as usize).min(w);
            let y1 = ((cy + r + 1.0).max(0.0) as usize).min(h);
            let soft = 0.6 + 0.12 * r * (1.0 - d.depth);
            for y in y0..y1 {
                let dy = y as f32 + 0.5 - cy;
                for x in x0..x1 {
                    let dx = x as f32 + 0.5 - cx;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let edge = ((r - dist) / soft).clamp(0.0, 1.0);
                    if edge <= 0.0 {
                        continue;
                    }
                    // Flat disc with a brighter rim, as from a real lens.
                    let u = dist / r;
                    let rim = 0.75 + 0.6 * ((u - 0.7) / 0.3).clamp(0.0, 1.0).powi(2);
                    let k = edge * rim;
                    let a = &mut self.acc[y * w + x];
                    for ch in 0..3 {
                        a[ch] += col[ch] * k;
                    }
                }
            }
        }

        for (p, a) in c.px.iter_mut().zip(&self.acc) {
            *p = rgb(
                1.0 - (-a[0] * 1.4).exp(),
                1.0 - (-a[1] * 1.4).exp(),
                1.0 - (-a[2] * 1.4).exp(),
            );
        }
    }
}
