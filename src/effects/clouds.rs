use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, mix, rgb, scale};

/// Horizon as a fraction of the height.
const HORIZON: f32 = 0.8;
const MAX_FLASHES: usize = 4;

fn hash(x: i32, y: i32) -> f32 {
    let mut n = (x as u32).wrapping_mul(0x27D4_EB2D) ^ (y as u32).wrapping_mul(0x1656_67B1);
    n ^= n >> 15;
    n = n.wrapping_mul(0x85EB_CA6B);
    n ^= n >> 13;
    (n & 0xFFFF) as f32 / 65535.0
}

/// Smooth 2D value noise in 0..1.
fn noise(x: f32, y: f32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (tx, ty) = (x - xf, y - yf);
    let (tx, ty) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let (i, j) = (xf as i32, yf as i32);
    let a = hash(i, j);
    let b = hash(i.wrapping_add(1), j);
    let c = hash(i, j.wrapping_add(1));
    let d = hash(i.wrapping_add(1), j.wrapping_add(1));
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * ty
}

fn fbm(x: f32, y: f32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut freq) = (0.0, 1.0, 0.0, 1.0);
    for i in 0..octaves {
        // Offset each octave so the lattices don't line up.
        sum += amp * noise(x * freq + i as f32 * 5.3, y * freq - i as f32 * 3.1);
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / norm
}

/// A deck of fbm clouds drifting overhead at dusk, seen in perspective down
/// to a dark horizon. The palette shifts with the balance of bass and treble,
/// the level thickens the clouds, and beats fire lightning that glows inside them.
pub struct Clouds {
    scroll: f32,
    hue: f32,
    /// Lightning: x, y as fractions of the canvas, intensity.
    flashes: Vec<(f32, f32, f32)>,
    rng: Rng,
    time: f32,
}

impl Clouds {
    pub fn new() -> Self {
        Self {
            scroll: 0.0,
            hue: 0.0,
            flashes: Vec::new(),
            rng: Rng::new(0xC10D),
            time: 0.0,
        }
    }
}

impl Effect for Clouds {
    fn name(&self) -> &'static str {
        "Clouds"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        self.time += dt;
        self.scroll += dt * (0.25 + 0.6 * f.bass);
        let target = (f.treble - f.bass) * 0.12 + 0.04 * (self.time * 0.05).sin();
        self.hue += (target - self.hue) * (1.0 - (-dt * 1.5).exp());

        for fl in &mut self.flashes {
            fl.2 *= (-dt * 5.0).exp();
        }
        self.flashes.retain(|fl| fl.2 > 0.02);
        if f.beat && self.flashes.len() < MAX_FLASHES {
            let (x, y) = (self.rng.range(0.1, 0.9), self.rng.range(0.15, 0.6));
            self.flashes.push((x, y, 0.8 + f.bass));
        }
        // Flicker: lightning rarely glows steadily.
        let flicker = 0.75 + 0.25 * (self.time * 53.0).sin();

        if c.w == 0 || c.h == 0 {
            return;
        }
        let (w, h) = (c.w as f32, c.h as f32);
        let hz = h * HORIZON;
        let hue = self.hue;
        let zenith = hsv(0.68 + hue * 0.5, 0.75, 0.22);
        let glow = hsv(0.04 + hue, 0.75, 0.95);
        let lit = hsv(0.03 + hue * 1.5, 0.55, 1.0);
        let shadow = hsv(0.78 + hue, 0.5, 0.2);
        let cover = 0.52 - 0.12 * f.level;
        let bolt = rgb(0.75, 0.8, 1.0);
        let radius2 = (h * 0.22).max(1.0).powi(2);

        let land: Vec<f32> = (0..c.w)
            .map(|x| {
                let u = x as f32 / w;
                hz - h * (0.02 + 0.03 * (u * 4.0 + 0.5).sin().abs() + 0.02 * noise(u * 25.0, 0.5))
            })
            .collect();

        let flash: f32 = self.flashes.iter().map(|fl| fl.2).sum::<f32>() * flicker;
        let land_col = scale(rgb(0.03, 0.02, 0.05), 1.0 + flash.min(2.0));

        for y in 0..c.h {
            let fy = y as f32;
            let g = (fy / hz.max(1.0)).min(1.0);
            let sky = mix(zenith, glow, g * g * g);
            // Distance to the cloud plane along this row's view ray.
            let z = 0.35 / ((hz - fy) / h + 0.04).max(0.04);
            let haze = (1.0 - z / 9.0).clamp(0.0, 1.0);
            for (x, &land_top) in land.iter().enumerate() {
                if fy >= land_top {
                    c.set(x, y, land_col);
                    continue;
                }
                let px = (x as f32 - w * 0.5) / h * z;
                let (nx, ny) = (px * 1.6, z * 1.6 + self.scroll);
                let n = fbm(nx, ny, 4);
                let density = ((n - cover) / 0.25).clamp(0.0, 1.0) * haze;
                let mut col = sky;
                if density > 0.0 {
                    // Brighter where the cloud thins out towards the sun below.
                    let light = (0.5 + (n - fbm(nx, ny + 0.25, 2)) * 3.0).clamp(0.0, 1.0);
                    let cloud = mix(shadow, lit, light * (0.4 + 0.6 * g));
                    col = mix(sky, cloud, density.sqrt());
                }
                for &(lx, ly, k) in &self.flashes {
                    let (dx, dy) = (x as f32 - lx * w, fy - ly * h);
                    let d2 = dx * dx + dy * dy;
                    let v = k * flicker * (-d2 / radius2).exp() * (0.15 + 1.2 * density);
                    col = mix(col, bolt, v);
                }
                c.set(x, y, col);
            }
        }
    }
}
