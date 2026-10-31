use super::Effect;
use super::ink::noise;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::Canvas;

/// Pixels per sample along each axis on large canvases; small tiles run at full resolution.
const CELL: usize = 2;
/// Paint colors floated on the size bath: indigo, cream, rust, ochre, teal, cream.
const PAINTS: [[f32; 3]; 6] = [
    [0.10, 0.14, 0.42],
    [0.93, 0.88, 0.74],
    [0.70, 0.22, 0.12],
    [0.86, 0.62, 0.20],
    [0.10, 0.45, 0.48],
    [0.95, 0.92, 0.82],
];

fn fbm(x: f32, y: f32) -> f32 {
    noise(x, y) * 0.57 + noise(x * 2.03 + 5.3, y * 2.03 - 3.1) * 0.29 + noise(x * 4.1 - 2.2, y * 4.1 + 8.7) * 0.14
}

/// Paper marbling: bands of floated paint, combed into waves, warped by
/// noise and dragged around by two slow swirls. The level speeds up the
/// flow, bass and beats twist the swirls harder.
pub struct Marble {
    colors: Vec<[f32; 3]>,
    time: f32,
    twist: f32,
    shift: f32,
}

impl Marble {
    pub fn new() -> Self {
        Self {
            colors: Vec::new(),
            time: 0.0,
            twist: 0.0,
            shift: 0.0,
        }
    }
}

impl Effect for Marble {
    fn name(&self) -> &'static str {
        "Marble"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt * (0.15 + 0.5 * f.level);
        if f.beat {
            self.twist += 0.8;
        }
        self.twist *= (-dt * 1.5).exp();
        self.shift += dt * (0.02 + 0.08 * f.mid);

        let cell = if c.w * c.h < 120 * 60 { 1 } else { CELL };
        let (gw, gh) = (c.w.div_ceil(cell), c.h.div_ceil(cell));
        self.colors.resize(gw * gh, [0.0; 3]);
        let s = (gw.min(gh) as f32).max(1.0);
        let t = self.time;
        let swirl = 1.6 + 1.8 * f.bass + self.twist;
        // Two counter-rotating vortices wandering around the sheet (sheet units).
        let (aw, ah) = (gw as f32 / s, gh as f32 / s);
        let vort = [
            (
                aw * (0.5 + 0.3 * (t * 0.5).sin()),
                ah * (0.5 + 0.3 * (t * 0.37).cos()),
                swirl,
            ),
            (
                aw * (0.5 + 0.3 * (t * 0.29 + 2.0).cos()),
                ah * (0.5 + 0.3 * (t * 0.41 + 1.0).sin()),
                -swirl * 0.8,
            ),
        ];
        let rad2 = 0.12;
        // Fewer, wider bands on small tiles so they don't alias.
        let detail = (s / 60.0).clamp(0.45, 1.0);
        let bright = 0.9 + 0.15 * f.pulse;

        for y in 0..gh {
            for x in 0..gw {
                let (mut px, mut py) = (x as f32 / s, y as f32 / s);
                for &(vx, vy, k) in &vort {
                    let (dx, dy) = (px - vx, py - vy);
                    let a = k * (-(dx * dx + dy * dy) / rad2).exp();
                    let (sa, ca) = a.sin_cos();
                    px = vx + dx * ca - dy * sa;
                    py = vy + dx * sa + dy * ca;
                }
                // Comb drawn through the bands, then a noisy drag.
                px += 0.06 * (py * 9.0 * detail + t * 1.3).sin();
                let wx = fbm(px * 2.0 + t * 0.3, py * 2.0);
                let wy = fbm(px * 2.0 - 4.0, py * 2.0 - t * 0.2);
                let band = (px + 0.9 * wx + 0.3 * wy) * 2.1 * detail + self.shift;

                let u = band.rem_euclid(1.0) * PAINTS.len() as f32;
                let i = (u as usize).min(PAINTS.len() - 1);
                let j = (i + 1) % PAINTS.len();
                let k = u - i as f32;
                // Mostly flat paint with short blends and a thin dark vein at each border.
                let blend = ((k - 0.8) / 0.2).clamp(0.0, 1.0);
                let vein = 1.0 - 0.35 * (-((k - 0.9) / 0.05).powi(2)).exp();
                let grain = 0.96 + 0.04 * noise(px * 40.0, py * 40.0);
                let (a, b) = (PAINTS[i], PAINTS[j]);
                let mut out = [0.0; 3];
                for (ch, o) in out.iter_mut().enumerate() {
                    *o = (a[ch] + (b[ch] - a[ch]) * blend) * vein * grain * bright;
                }
                self.colors[y * gw + x] = out;
            }
        }
        upscale(c, &self.colors, gw, gh);
    }
}
