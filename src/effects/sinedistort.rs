use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, rgb};
use std::f32::consts::TAU;

/// A procedural picture of glowing orbs on rainbow stripes, warped by
/// per-line sine offsets like a raster distortion. Bass sets the amplitude,
/// beats add a jolt, the spectrum shifts single lines.
pub struct SineDistort {
    phase: f32,
    scroll: f32,
    kick: f32,
    hue: f32,
}

impl SineDistort {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            scroll: 0.0,
            kick: 0.0,
            hue: 0.0,
        }
    }
}

/// Source image in normalized units (one unit = a quarter of the height).
fn picture(u: f32, v: f32, hue: f32) -> Rgb {
    // Diagonal rainbow stripes as background.
    let stripe = (u + v) * 0.5;
    let band = stripe - stripe.floor();
    let bg = hsv(hue + stripe.floor() * 0.13, 0.7, 0.25 + 0.15 * (band * TAU).sin().abs());
    // A grid of shaded orbs on top.
    let (cu, cv) = (u.floor(), v.floor());
    let (du, dv) = (u - cu - 0.5, v - cv - 0.5);
    let d2 = du * du + dv * dv;
    if d2 < 0.13 {
        let shade = 1.0 - d2 / 0.13;
        let spec = (1.0 - ((du + 0.12).powi(2) + (dv + 0.12).powi(2)) * 30.0).max(0.0);
        let base = hsv(hue + (cu * 0.17 + cv * 0.29), 0.85, 0.35 + 0.65 * shade.sqrt());
        let s = spec * 0.8;
        return rgb(
            base[0] as f32 / 255.0 + s,
            base[1] as f32 / 255.0 + s,
            base[2] as f32 / 255.0 + s,
        );
    }
    bg
}

impl Effect for SineDistort {
    fn name(&self) -> &'static str {
        "Sine Distort"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.phase += dt * (1.5 + f.mid * 2.0);
        self.scroll += dt * (0.3 + f.level * 0.6);
        self.hue += dt * 0.02;
        self.kick = if f.beat { 1.0 } else { (self.kick - dt * 2.5).max(0.0) };
        if c.w == 0 || c.h == 0 {
            return;
        }

        let unit = 4.0 / c.h as f32;
        let amp = 0.15 + f.bass * 0.9 + self.kick * 0.5;
        let bins = f.spectrum.len();
        let p = self.phase;
        for y in 0..c.h {
            let fy = y as f32 * unit;
            // Classic raster distortion: every line gets its own horizontal shift.
            let mut shift = amp * ((fy * 1.7 + p * 2.0).sin() * 0.6 + (fy * 0.6 - p * 1.3).sin() * 0.4);
            if bins > 0 {
                let b = f.spectrum[y * 7 % bins];
                shift += (b - 0.5).max(0.0) * 0.3 * (fy * 9.0 + t * 5.0).sin();
            }
            let v = fy + (fy * 0.9 + p).sin() * amp * 0.15 + self.scroll * 0.4;
            for x in 0..c.w {
                let u = x as f32 * unit + shift + self.scroll;
                c.set(x, y, picture(u, v, self.hue));
            }
        }
    }
}
