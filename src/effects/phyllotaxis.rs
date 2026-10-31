use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix};

/// 2π / φ², the angle between successive seeds of a sunflower.
const GOLDEN_ANGLE: f32 = 2.399_963;

/// Antialiased filled disc; `shade` maps the normalized distance from the
/// center (0..1) to a color, blended over what is already there.
pub(super) fn disc(c: &mut Canvas, x: f32, y: f32, r: f32, shade: impl Fn(f32) -> Rgb) {
    if r.is_nan() || r <= 0.0 || !x.is_finite() || !y.is_finite() || c.w == 0 || c.h == 0 {
        return;
    }
    let x0 = (x - r - 1.0).max(0.0) as usize;
    let y0 = (y - r - 1.0).max(0.0) as usize;
    let x1 = (x + r + 1.0).min(c.w as f32 - 1.0);
    let y1 = (y + r + 1.0).min(c.h as f32 - 1.0);
    if x1 < 0.0 || y1 < 0.0 {
        return;
    }
    let inv = 1.0 / r;
    for py in y0..=y1 as usize {
        let dy = py as f32 + 0.5 - y;
        for px in x0..=x1 as usize {
            let dx = px as f32 + 0.5 - x;
            let d = (dx * dx + dy * dy).sqrt();
            let cov = (r + 0.5 - d).min(1.0);
            if cov > 0.0 {
                let i = py * c.w + px;
                let col = shade((d * inv).min(1.0));
                c.px[i] = if cov >= 1.0 { col } else { mix(c.px[i], col, cov) };
            }
        }
    }
}

/// Sunflower phyllotaxis: seeds placed at multiples of the golden angle,
/// streaming outwards from the middle. The angle is slightly detuned over
/// time so spiral arms wind up and unwind. Seeds swell with the spectrum band
/// of their ring, bass speeds up the flow and the pulse makes them all pop.
pub struct Phyllotaxis {
    flow: f32,
    time: f32,
    spin: f32,
    hue: f32,
}

impl Phyllotaxis {
    pub fn new() -> Self {
        Self {
            flow: 0.0,
            time: 0.0,
            spin: 0.0,
            hue: 0.1,
        }
    }
}

impl Effect for Phyllotaxis {
    fn name(&self) -> &'static str {
        "Phyllotaxis"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        c.fill([2, 3, 2]);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        self.flow += dt * (1.5 + 8.0 * f.bass);
        self.spin += dt * (0.05 + 0.1 * f.mid);
        self.hue += dt * 0.015;
        // Keep the seed counter small; whole steps leave the pattern unchanged.
        if self.flow > 1.0e4 {
            self.flow -= 1.0e4;
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let reach = 0.5 * (w * w + h * h).sqrt();
        let spacing = (w.min(h) / 26.0).max(2.6);
        let n = ((reach / spacing).powi(2) as usize).clamp(8, 4000);
        let detune = 0.012 * (self.time * 0.07).sin() + 0.004 * (self.time * 0.23).sin();
        let angle = GOLDEN_ANGLE + detune;
        let frac = self.flow.fract();
        let first = self.flow.floor();
        let bins = f.spectrum.len();
        let (cx, cy) = (w * 0.5, h * 0.5);
        let base = spacing * 0.42;

        for j in 0..n {
            let k = j as f32 + frac;
            let r = spacing * k.sqrt();
            let a = k * angle + self.spin;
            let ring = r / reach;
            let band = if bins > 0 {
                f.spectrum[((ring * bins as f32) as usize).min(bins - 1)]
            } else {
                0.0
            };
            // Seeds grow in from nothing at the center.
            let birth = k.min(1.0);
            let size = base * birth * (0.55 + 0.6 * band + 0.35 * f.pulse);
            // A seed keeps its color while it travels outwards.
            let id = j as f32 - first;
            let hue = self.hue + ring * 0.35 + 0.04 * (id * 0.618).fract();
            let value = (0.45 + 0.4 * band + 0.25 * f.pulse).min(1.0);
            let inner = hsv(hue, 0.4, value);
            let outer = hsv(hue + 0.05, 0.9, value * 0.55);
            disc(c, cx + a.cos() * r, cy + a.sin() * r, size, |d| mix(inner, outer, d));
        }
    }
}
