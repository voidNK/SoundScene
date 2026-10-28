use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, add, hsv, rgb, scale};
use std::f32::consts::{PI, TAU};

const TRAIL: usize = 48;

struct Planet {
    /// Orbit radius as a fraction of the system size.
    orbit: f32,
    /// Angular speed in radians per second at normal tempo.
    speed: f32,
    size: f32,
    hue: f32,
    moons: usize,
    ring: bool,
    angle: f32,
}

const PLANETS: [(f32, f32, f32, f32, usize, bool); 6] = [
    (0.22, 1.6, 0.022, 0.08, 0, false),
    (0.34, 1.1, 0.032, 0.12, 0, false),
    (0.48, 0.75, 0.036, 0.58, 1, false),
    (0.63, 0.5, 0.03, 0.02, 2, false),
    (0.8, 0.33, 0.055, 0.1, 0, true),
    (0.96, 0.22, 0.042, 0.52, 3, false),
];

/// A little solar system: planets with moons and fading orbit trails circle
/// a pulsing sun. The sun's size follows the bass and its corona the
/// spectrum, loudness and beats speed the orbits up.
pub struct Orbits {
    planets: Vec<Planet>,
    stars: Vec<(f32, f32, f32)>,
    moon_phase: f32,
    boost: f32,
}

impl Orbits {
    pub fn new() -> Self {
        let mut rng = Rng::new(0x501A);
        let planets = PLANETS
            .iter()
            .map(|&(orbit, speed, size, hue, moons, ring)| Planet {
                orbit,
                speed,
                size,
                hue,
                moons,
                ring,
                angle: rng.range(0.0, TAU),
            })
            .collect();
        let stars = (0..120).map(|_| (rng.f32(), rng.f32(), rng.range(0.1, 0.5))).collect();
        Self {
            planets,
            stars,
            moon_phase: 0.0,
            boost: 0.0,
        }
    }
}

/// Lit sphere of radius `r` at (x, y); `light` points from it to the sun.
fn ball(c: &mut Canvas, x: f32, y: f32, r: f32, light: [f32; 3], col: Rgb) {
    if r < 0.8 {
        c.plot_add(x.round() as i32, y.round() as i32, col);
        return;
    }
    let (x0, x1) = ((x - r).floor() as i32, (x + r).ceil() as i32);
    let (y0, y1) = ((y - r).floor() as i32, (y + r).ceil() as i32);
    for py in y0..=y1 {
        for px in x0..=x1 {
            let dx = (px as f32 + 0.5 - x) / r;
            let dy = (py as f32 + 0.5 - y) / r;
            let d2 = dx * dx + dy * dy;
            if d2 < 1.0 {
                let nz = (1.0 - d2).sqrt();
                let lit = (dx * light[0] + dy * light[1] + nz * light[2]).max(0.0);
                c.plot(px, py, scale(col, 0.08 + 0.92 * lit));
            }
        }
    }
}

impl Effect for Orbits {
    fn name(&self) -> &'static str {
        "Orbits"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.boost = (self.boost + 1.5).min(4.0);
        }
        self.boost *= (-dt * 2.0).exp();
        let tempo = 0.25 + 0.9 * f.level + self.boost;
        for p in &mut self.planets {
            p.angle = (p.angle + dt * p.speed * tempo).rem_euclid(TAU);
        }
        self.moon_phase = (self.moon_phase + dt * tempo * 2.5).rem_euclid(TAU * 5.0);

        c.clear();
        let (w, h) = (c.w as f32, c.h as f32);
        for &(sx, sy, b) in &self.stars {
            c.plot_add((sx * w) as i32, (sy * h) as i32, rgb(b * 0.8, b * 0.85, b));
        }

        let (cx, cy) = (w * 0.5, h * 0.5);
        let elev = 0.42 + 0.12 * (t * 0.11).sin();
        let (se, ce) = elev.sin_cos();
        let yaw = t * 0.03;
        let size = (w * 0.46).min(h * 0.46 / se.max(0.2)).max(0.0);
        // World point on the orbit plane -> (screen x, screen y, depth toward viewer).
        let view = |x: f32, z: f32| {
            let (s, co) = yaw.sin_cos();
            let (x, z) = (x * co - z * s, x * s + z * co);
            let k = 4.0 / (4.0 - z * ce);
            (cx + x * size * k, cy + z * se * size * k, x, z * se, z * ce)
        };

        // Faint orbit lines and bright trails behind each planet.
        for p in &self.planets {
            let col = hsv(p.hue, 0.5, 0.07);
            for i in 0..90 {
                let a = i as f32 / 90.0 * TAU;
                let (x, y, ..) = view(p.orbit * a.cos(), p.orbit * a.sin());
                c.plot_add(x as i32, y as i32, col);
            }
            let arc = 1.4 / (1.0 + p.orbit);
            for i in 0..TRAIL {
                let k = i as f32 / TRAIL as f32;
                let a = p.angle - k * arc;
                let (x, y, ..) = view(p.orbit * a.cos(), p.orbit * a.sin());
                c.plot_add(x as i32, y as i32, hsv(p.hue, 0.6, 0.45 * (1.0 - k) * (1.0 - k)));
            }
        }

        let bins = f.spectrum.len();
        let band = |i: usize| {
            if bins == 0 {
                0.0
            } else {
                f.spectrum[(i * bins / 6).min(bins - 1)]
            }
        };
        let sun_r = size * (0.075 + 0.05 * f.bass + 0.025 * f.pulse);
        let mut order: Vec<(f32, usize)> = self
            .planets
            .iter()
            .enumerate()
            .map(|(i, p)| (view(p.orbit * p.angle.cos(), p.orbit * p.angle.sin()).4, i))
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut sun_drawn = false;
        for &(depth, i) in &order {
            if depth > 0.0 && !sun_drawn {
                self.draw_sun(c, cx, cy, sun_r, f);
                sun_drawn = true;
            }
            let p = &self.planets[i];
            let (wx, wz) = (p.orbit * p.angle.cos(), p.orbit * p.angle.sin());
            let (x, y, vx, vy, vz) = view(wx, wz);
            let l = (vx * vx + vy * vy + vz * vz).sqrt().max(1e-4);
            let light = [-vx / l, -vy / l, -vz / l];
            let r = p.size * size * (1.0 + 0.4 * band(i));
            let col = hsv(p.hue, 0.65, 1.0);
            if p.ring {
                for k in 0..80 {
                    let a = k as f32 / 80.0 * TAU;
                    let (rx, ry) = (a.cos() * r * 2.0, a.sin() * r * 2.0 * se);
                    // Back half of the ring first; the front half is drawn after the planet.
                    if ry < 0.0 {
                        c.plot_add((x + rx) as i32, (y + ry) as i32, hsv(p.hue + 0.05, 0.4, 0.35));
                    }
                }
            }
            ball(c, x, y, r, light, col);
            if p.ring {
                for k in 0..80 {
                    let a = k as f32 / 80.0 * TAU;
                    let (rx, ry) = (a.cos() * r * 2.0, a.sin() * r * 2.0 * se);
                    if ry >= 0.0 {
                        c.plot_add((x + rx) as i32, (y + ry) as i32, hsv(p.hue + 0.05, 0.4, 0.5));
                    }
                }
            }
            for m in 0..p.moons {
                let a = self.moon_phase * (1.0 + 0.4 * m as f32) + m as f32 * 2.1;
                let mr = p.size * (2.2 + 0.9 * m as f32);
                let (mx, my, ..) = view(wx + mr * a.cos(), wz + mr * a.sin());
                let mcol = hsv(p.hue + 0.5, 0.15, 0.8);
                c.plot(mx as i32, my as i32, mcol);
                if size > 60.0 {
                    c.plot(mx as i32 + 1, my as i32, scale(mcol, 0.6));
                }
            }
        }
        if !sun_drawn {
            self.draw_sun(c, cx, cy, sun_r, f);
        }
    }
}

impl Orbits {
    fn draw_sun(&self, c: &mut Canvas, cx: f32, cy: f32, r: f32, f: &Features) {
        if r < 0.5 {
            c.plot_add(cx as i32, cy as i32, rgb(1.0, 0.8, 0.3));
            return;
        }
        let bins = f.spectrum.len();
        let reach = r * 3.2;
        let (x0, x1) = (
            (cx - reach).max(0.0) as usize,
            ((cx + reach).ceil().max(0.0) as usize).min(c.w),
        );
        let (y0, y1) = (
            (cy - reach).max(0.0) as usize,
            ((cy + reach).ceil().max(0.0) as usize).min(c.h),
        );
        for y in y0..y1 {
            let dy = y as f32 + 0.5 - cy;
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let d = (dx * dx + dy * dy).sqrt() / r;
                let i = y * c.w + x;
                if d < 1.0 {
                    let limb = 1.0 - 0.45 * d * d;
                    c.px[i] = rgb(1.0 * limb + 0.3 * f.pulse, 0.85 * limb, 0.35 * limb + 0.2 * f.pulse);
                } else {
                    // Corona shaped by the spectrum, mirrored left/right.
                    let a = dy.atan2(dx).abs() / PI;
                    let s = if bins > 0 {
                        f.spectrum[((a * (bins - 1) as f32) as usize).min(bins - 1)]
                    } else {
                        0.0
                    };
                    let reach = 0.25 + 1.4 * s;
                    let g = (-(d - 1.0) / reach * 2.2).exp() * (0.5 + 0.5 * f.level);
                    c.px[i] = add(c.px[i], rgb(g, g * 0.55, g * 0.15));
                }
            }
        }
    }
}
