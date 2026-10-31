use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, scale};

/// (radius, petals, petal depth, spin speed) from the center outwards.
const LAYERS: [(f32, f32, f32, f32); 7] = [
    (0.13, 6.0, 0.45, 0.30),
    (0.25, 8.0, 0.35, -0.22),
    (0.37, 12.0, 0.30, 0.16),
    (0.50, 10.0, 0.35, -0.12),
    (0.63, 16.0, 0.22, 0.09),
    (0.78, 12.0, 0.32, -0.07),
    (0.95, 24.0, 0.14, 0.05),
];

/// Radially symmetric mandala of scalloped petal rings, each spinning the
/// other way from its neighbours. Every ring breathes with its own slice of
/// the spectrum (bass in the middle, treble at the rim), the pulse swells the
/// whole flower and beats shift the palette.
pub struct Mandala {
    spin: [f32; 7],
    breath: [f32; 7],
    hue: f32,
    /// Per-pixel (radius, angle), rebuilt when the canvas size changes.
    polar: Vec<(f32, f32)>,
    size: (usize, usize),
}

impl Mandala {
    pub fn new() -> Self {
        Self {
            spin: [0.0; 7],
            breath: [0.0; 7],
            hue: 0.0,
            polar: Vec::new(),
            size: (0, 0),
        }
    }
}

impl Effect for Mandala {
    fn name(&self) -> &'static str {
        "Mandala"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.hue += dt * 0.02 + if f.beat { 0.07 } else { 0.0 };
        let bins = f.spectrum.len();
        let k = 1.0 - (-dt * 10.0).exp();
        for (i, ((spin, breath), layer)) in self.spin.iter_mut().zip(&mut self.breath).zip(&LAYERS).enumerate() {
            *spin += dt * layer.3 * (1.0 + 1.5 * f.mid);
            let band = if bins > 0 {
                let (lo, hi) = (
                    i * bins / LAYERS.len(),
                    ((i + 1) * bins / LAYERS.len()).max(i * bins / LAYERS.len() + 1),
                );
                f.spectrum[lo.min(bins - 1)..hi.min(bins)].iter().sum::<f32>() / (hi - lo) as f32
            } else {
                0.0
            };
            *breath += (band - *breath) * k;
        }

        let scale_px = c.w.min(c.h) as f32 * 0.5;
        let inv = 1.0 / scale_px.max(1.0);
        if self.size != (c.w, c.h) || self.polar.len() != c.w * c.h {
            self.size = (c.w, c.h);
            let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
            self.polar = (0..c.w * c.h)
                .map(|i| {
                    let dx = ((i % c.w) as f32 + 0.5 - cx) * inv;
                    let dy = ((i / c.w) as f32 + 0.5 - cy) * inv;
                    ((dx * dx + dy * dy).sqrt(), dy.atan2(dx))
                })
                .collect();
        }
        let swell = 1.0 + 0.05 * f.pulse + 0.03 * (t * 0.7).sin();
        // Outline width in pixels, thinner on tiny tiles.
        let line = (scale_px / 12.0).clamp(0.5, 1.4) * inv;

        for (px, &(r, a)) in c.px.iter_mut().zip(&self.polar) {
            let mut inner = 0.0;
            let mut col = None;
            for (i, &(base, petals, depth, _)) in LAYERS.iter().enumerate() {
                let p = (0.5 * petals * (a - self.spin[i])).cos().abs();
                let edge = base * swell * (1.0 + 0.12 * self.breath[i]) * (1.0 - depth * (1.0 - p) * (1.0 - p));
                if r < edge {
                    let span = (edge - inner).max(1e-3);
                    let u = ((r - inner) / span).clamp(0.0, 1.0);
                    let hue = self.hue + i as f32 * 0.11 + 0.05 * u;
                    let glow = 0.35 + 0.65 * self.breath[i];
                    let mut v = (0.25 + 0.55 * u * (0.4 + 0.6 * p)) * (0.6 + 0.4 * glow) + 0.15 * glow * p;
                    // Dark outline along the petal rim and a thin inner contour.
                    let rim = ((edge - r) / line).min(1.0);
                    let contour = ((r - (inner + span * 0.55)).abs() / line).min(1.0);
                    v *= rim * (0.55 + 0.45 * contour);
                    col = Some(hsv(hue, 0.75 - 0.35 * u * p, v.min(1.0)));
                    break;
                }
                inner = edge;
            }
            *px = col.unwrap_or_else(|| {
                let rays = 0.5 + 0.5 * (24.0 * a + t * 0.5).cos();
                scale(hsv(self.hue + 0.5, 0.6, 0.12), rays * (1.0 / (1.0 + r)))
            });
        }
    }
}
