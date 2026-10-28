use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, rgb};

/// Tour stops: center and half height of the view.
const STOPS: [(f64, f64, f64); 5] = [
    (-0.45, -0.5, 1.15),
    (-1.762, -0.028, 0.045),
    (-1.6, -0.02, 0.25),
    (-1.941, -0.002, 0.012),
    (-1.1, -1.0, 0.3),
];
const TRAVEL: f32 = 9.0;
const HOLD: f32 = 5.0;
const MAX_ITER: u32 = 72;
const PALETTE: usize = 256;

/// Burning Ship fractal on a slow tour between its ships and antennas, in a
/// fire palette that scrolls with the treble. Beats flash the colors and
/// punch the zoom, bass speeds up the journey.
pub struct BurningShip {
    stop: usize,
    clock: f32,
    shift: f32,
    vals: Vec<f32>,
    size: (usize, usize),
    palette: Vec<Rgb>,
}

impl BurningShip {
    pub fn new() -> Self {
        let palette = (0..PALETTE)
            .map(|i| {
                let v = i as f32 / (PALETTE - 1) as f32;
                rgb(v * 2.5, v * 2.5 - 0.9, v * 3.0 - 2.1)
            })
            .collect();
        Self {
            stop: 0,
            clock: 0.0,
            shift: 0.0,
            vals: Vec::new(),
            size: (0, 0),
            palette,
        }
    }

    /// Current view: center and half height.
    fn view(&self) -> (f64, f64, f64) {
        let a = STOPS[self.stop];
        let b = STOPS[(self.stop + 1) % STOPS.len()];
        let u = ((self.clock - HOLD) / TRAVEL).clamp(0.0, 1.0) as f64;
        let u = u * u * (3.0 - 2.0 * u);
        // Zoom exponentially and move the center in proportion to the zoom
        // change, so the flight neither overshoots nor drifts sideways.
        let s = (a.2.ln() + (b.2.ln() - a.2.ln()) * u).exp();
        let k = if (a.2 - b.2).abs() > 1e-9 {
            (a.2 - s) / (a.2 - b.2)
        } else {
            u
        };
        (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k, s)
    }
}

impl Effect for BurningShip {
    fn name(&self) -> &'static str {
        "Burning Ship"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if self.size != (c.w, c.h) {
            self.size = (c.w, c.h);
            self.vals = vec![-1.0; c.w * c.h];
        }
        self.clock += dt * (0.7 + 0.8 * f.bass);
        if self.clock >= HOLD + TRAVEL {
            self.clock = 0.0;
            self.stop = (self.stop + 1) % STOPS.len();
        }
        self.shift += dt * (0.05 + 0.25 * f.treble);

        let (ox, oy, s) = self.view();
        // Gentle drift while holding, and a zoom punch on the beat.
        let drift = (t as f64 * 0.3).sin() * 0.04;
        let s = s * (1.0 + drift) * (1.0 - 0.03 * f.pulse as f64);
        let per = 2.0 * s / c.h.max(1) as f64;
        let (cx, cy) = (c.w as f64 * 0.5, c.h as f64 * 0.5);

        for y in 0..c.h {
            let ci = oy + (y as f64 - cy) * per;
            for x in 0..c.w {
                let cr = ox + (x as f64 - cx) * per;
                self.vals[y * c.w + x] = escape(cr, ci);
            }
        }

        // Deep in the tour every outside pixel takes many iterations; measuring
        // from the lowest count keeps the far background dark.
        let floor = self
            .vals
            .iter()
            .fold(f32::MAX, |m, &v| if v >= 0.0 { m.min(v) } else { m });
        let floor = if floor == f32::MAX { 0.0 } else { floor };
        let flash = 0.85 + 0.45 * f.pulse;
        let inside = rgb(0.12 * f.pulse + 0.03, 0.0, 0.0);
        for (px, &v) in c.px.iter_mut().zip(&self.vals) {
            *px = if v < 0.0 {
                inside
            } else {
                // Triangle wave so the palette runs up and down instead of wrapping.
                // Dark far outside, brightening towards the set.
                let u = (v.sqrt() * 0.5 - self.shift).rem_euclid(1.0);
                let tri = 1.0 - (2.0 * u - 1.0).abs();
                let k = (((v - floor) / 22.0).min(1.0) * (0.5 + 0.5 * tri) * flash).min(1.0);
                self.palette[((k * (PALETTE - 1) as f32) as usize).min(PALETTE - 1)]
            };
        }
    }
}

/// Smooth escape count, or -1 if the point stays bounded.
fn escape(cr: f64, ci: f64) -> f32 {
    let (mut zr, mut zi) = (0.0f64, 0.0f64);
    for n in 0..MAX_ITER {
        let (zr2, zi2) = (zr * zr, zi * zi);
        if zr2 + zi2 > 64.0 {
            let mu = n as f64 + 1.0 - (0.5 * (zr2 + zi2).ln()).ln() / std::f64::consts::LN_2;
            return mu.max(0.0) as f32;
        }
        zi = 2.0 * (zr * zi).abs() + ci;
        zr = zr2 - zi2 + cr;
    }
    -1.0
}
