use super::Effect;
use super::ink::{hash, noise};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, rgb};

/// Hue and saturation of the glass colors: ruby, cobalt, emerald, amber,
/// violet, teal, gold, rose.
const GLASS: [(f32, f32); 8] = [
    (0.99, 0.9),
    (0.62, 0.85),
    (0.36, 0.8),
    (0.09, 0.9),
    (0.77, 0.7),
    (0.5, 0.8),
    (0.14, 0.75),
    (0.93, 0.55),
];
const MAX_FLARES: usize = 6;

/// A stained-glass window: irregular panes of colored glass held by dark
/// lead cames, with light shining through from a slowly wandering sun.
/// Each pane glows with its own band, beats make single panes flare.
pub struct StainedGlass {
    /// Pane seeds per grid cell, in pixels.
    seeds: Vec<(f32, f32)>,
    /// Lattice cell id and intensity.
    flares: Vec<(i32, i32, f32)>,
    rng: Rng,
    time: f32,
}

impl StainedGlass {
    pub fn new() -> Self {
        Self {
            seeds: Vec::new(),
            flares: Vec::new(),
            rng: Rng::new(0x57A1),
            time: 0.0,
        }
    }
}

impl Effect for StainedGlass {
    fn name(&self) -> &'static str {
        "Stained Glass"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.time += dt;
        let (wf, hf) = (c.w as f32, c.h as f32);
        let g = (wf.min(hf) / 4.5).max(4.0);
        // One extra ring of lattice cells so border panes have neighbors.
        let gw = (wf / g).ceil() as i32 + 2;
        let gh = (hf / g).ceil() as i32 + 2;

        for fl in &mut self.flares {
            fl.2 *= (-dt * 2.5).exp();
        }
        self.flares.retain(|fl| fl.2 > 0.03);
        if f.beat {
            if self.flares.len() >= MAX_FLARES {
                self.flares.remove(0);
            }
            let i = (self.rng.f32() * (gw - 2) as f32) as i32;
            let j = (self.rng.f32() * (gh - 2) as f32) as i32;
            self.flares.push((i, j, 1.0));
        }

        // Seeds jittered within their cells, swaying very slightly.
        self.seeds.clear();
        for j in -1..gh - 1 {
            for i in -1..gw - 1 {
                let sway = 0.06 * (self.time * 0.4 + hash(i, j + 99) * 6.3).sin();
                self.seeds.push((
                    (i as f32 + 0.15 + 0.7 * hash(i, j) + sway) * g,
                    (j as f32 + 0.15 + 0.7 * hash(i + 31, j - 17) - sway) * g,
                ));
            }
        }
        let gwu = gw as usize;

        // A soft sun wanders behind the window.
        let sun = (
            wf * (0.5 + 0.35 * (self.time * 0.11).sin()),
            hf * (0.4 + 0.3 * (self.time * 0.07).cos()),
        );
        let sun_r2 = (wf.max(hf) * 0.6).powi(2);
        let light = 0.85 + 0.3 * f.level + 0.3 * f.pulse;
        let lead_w = (g * 0.07).max(0.7);

        for y in 0..c.h {
            let py = y as f32 + 0.5;
            let gy = (py / g) as i32;
            for x in 0..c.w {
                let px = x as f32 + 0.5;
                let gx = (px / g) as i32;
                // Nearest and second-nearest seed in the 3×3 neighborhood.
                let (mut d1, mut d2) = (f32::MAX, f32::MAX);
                let (mut s1, mut s2) = ((0.0, 0.0), (0.0, 0.0));
                let mut id = (0, 0);
                for j in gy - 1..=gy + 1 {
                    for i in gx - 1..=gx + 1 {
                        let k = (j + 1) as usize * gwu + (i + 1) as usize;
                        let Some(&s) = self.seeds.get(k) else { continue };
                        let d = (s.0 - px).powi(2) + (s.1 - py).powi(2);
                        if d < d1 {
                            (d2, s2) = (d1, s1);
                            (d1, s1, id) = (d, s, (i, j));
                        } else if d < d2 {
                            (d2, s2) = (d, s);
                        }
                    }
                }
                // Exact distance to the bisector between the two seeds.
                let sep = ((s1.0 - s2.0).powi(2) + (s1.1 - s2.1).powi(2)).sqrt().max(1e-3);
                let edge = if d2 == f32::MAX { g } else { (d2 - d1) / (2.0 * sep) };
                if edge < lead_w {
                    let shade = 0.06 + 0.06 * (1.0 - edge / lead_w);
                    c.set(x, y, rgb(shade, shade, shade * 1.1));
                    continue;
                }

                let hsh = hash(id.0, id.1 + 7);
                let (hue, sat) = GLASS[(hsh * GLASS.len() as f32) as usize % GLASS.len()];
                let bin = (hash(id.0 + 5, id.1) * 48.0) as usize;
                let band = f.spectrum.get(bin).copied().unwrap_or(0.0);
                let flare: f32 = self.flares.iter().filter(|fl| (fl.0, fl.1) == id).map(|fl| fl.2).sum();
                let sun_d2 = (px - sun.0).powi(2) + (py - sun.1).powi(2);
                let lit = (0.5 + 0.6 * (-sun_d2 / sun_r2).exp()) * light;
                // Bevel darkening near the lead, mottled glass, brighter centers.
                let bevel = ((edge - lead_w) / (lead_w * 2.5)).clamp(0.0, 1.0);
                let mottle = 0.8 + 0.3 * noise(px * 0.35 + hsh * 50.0, py * 0.35);
                let center = 1.0 - 0.25 * (d1.sqrt() / g).min(1.0);
                let v = (lit * (0.5 + 0.7 * band) * (0.55 + 0.45 * bevel) * mottle * center + flare * 0.6).min(1.6);
                let base = hsv(hue, sat * (1.0 - 0.3 * flare.min(1.0)), v.min(1.0));
                let over = (v - 1.0).max(0.0);
                c.set(
                    x,
                    y,
                    rgb(
                        base[0] as f32 / 255.0 + over,
                        base[1] as f32 / 255.0 + over,
                        base[2] as f32 / 255.0 + over,
                    ),
                );
            }
        }
    }
}
