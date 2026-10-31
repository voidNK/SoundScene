use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, rgb};

/// Corridor half-width and half-height in world units.
const HALF_W: f32 = 1.3;
const HALF_H: f32 = 1.0;
/// Distance between ceiling lights.
const LIGHT_GAP: f32 = 3.0;

/// Flight down a paneled fake-3D corridor with ceiling lights. Bass makes
/// the lights flicker, level sets the speed, beats flash the far end.
pub struct Corridor {
    travel: f32,
    flicker: f32,
    flash: f32,
    rng: Rng,
}

impl Corridor {
    pub fn new() -> Self {
        Self {
            travel: 0.0,
            flicker: 1.0,
            flash: 0.0,
            rng: Rng::new(4242),
        }
    }
}

impl Effect for Corridor {
    fn name(&self) -> &'static str {
        "Corridor"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.travel += dt * (1.5 + f.level * 3.0 + f.pulse * 2.0);
        self.flash = if f.beat { 1.0 } else { (self.flash - dt * 2.5).max(0.0) };
        // Strong bass makes the lights stutter between dim and bright.
        let target = 0.75 + 0.25 * f.level + (self.rng.f32() - 0.5) * f.bass * 0.9;
        let k = (dt * 12.0).min(1.0);
        self.flicker += (target - self.flicker) * k;
        if c.w == 0 || c.h == 0 {
            return;
        }

        let inv = 2.0 / c.h as f32;
        let sway = (t * 0.4).sin() * 0.25;
        let bob = (t * 0.9).sin() * 0.06;
        let (cx, cy) = (c.w as f32 * 0.5, c.h as f32 * 0.5);
        let lights = self.flicker.clamp(0.2, 1.3);
        for y in 0..c.h {
            let dy = (y as f32 + 0.5 - cy) * inv;
            for x in 0..c.w {
                let dx = (x as f32 + 0.5 - cx) * inv;
                // Ray (dx, dy, 1) from a slightly swaying camera; nearest of the
                // side walls and floor/ceiling wins.
                let zw = if dx.abs() > 1e-4 {
                    (HALF_W - sway * dx.signum()) / dx.abs()
                } else {
                    f32::MAX
                };
                let zf = if dy.abs() > 1e-4 {
                    (HALF_H - bob * dy.signum()) / dy.abs()
                } else {
                    f32::MAX
                };
                let wall = zw < zf;
                let z = zw.min(zf).min(60.0);
                let along = z + self.travel;
                let across = if wall { dy * z } else { dx * z + sway };
                // Distance to the nearest ceiling light along the corridor.
                let lz = (along / LIGHT_GAP).fract() - 0.5;
                let near_light = (1.0 - lz.abs() * 2.0).max(0.0);
                let lit = 0.55 + 1.2 * near_light * near_light * lights;
                let fog = 1.0 / (1.0 + z * 0.12);

                let (mut r, mut g, mut b);
                if wall {
                    // Wall panels with dark seams and a glowing stripe at mid height.
                    let seam = (along * 1.2).fract() < 0.06 || across.abs() > 0.92 || (across + 0.35).abs() < 0.03;
                    let base = if seam { 0.08 } else { 0.32 };
                    r = base * 0.8;
                    g = base * 0.9;
                    b = base * 1.2;
                    let stripe = (1.0 - (across - 0.1).abs() * 25.0).max(0.0);
                    r += stripe * 0.1;
                    g += stripe * 0.6 * lights;
                    b += stripe * 0.9 * lights;
                } else if dy > 0.0 {
                    // Floor tiles.
                    let check = ((along.floor() as i64 + (across * 1.5).floor() as i64) & 1) == 0;
                    let base = if check { 0.3 } else { 0.18 };
                    r = base;
                    g = base * 0.85;
                    b = base * 0.75;
                } else {
                    // Ceiling with a light panel in the middle.
                    let panel = across.abs() < 0.35 && lz.abs() < 0.12;
                    if panel {
                        r = 1.4 * lights;
                        g = 1.2 * lights;
                        b = 0.9 * lights;
                    } else {
                        r = 0.15;
                        g = 0.15;
                        b = 0.18;
                    }
                }
                let far = self.flash * (z / 60.0).powi(2) * 0.8;
                r = r * lit * fog + far;
                g = g * lit * fog + far * 0.6;
                b = b * lit * fog + far * 0.9;
                c.set(x, y, rgb(r, g, b));
            }
        }
    }
}
