use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, add, mix, rgb, scale};
use std::f32::consts::PI;

const BLOBS: usize = 7;

struct Lamp {
    wax: Rgb,
    hot: Rgb,
    liquid: Rgb,
    /// Each blob travels up and down the glass on its own cycle.
    phases: [f32; BLOBS],
}

/// Pixel geometry of one lamp for the current frame.
struct Shape {
    cx: f32,
    top: f32,
    glass_top: f32,
    glass_bottom: f32,
    bottom: f32,
    r: f32,
}

impl Shape {
    /// Half width of the silhouette at row `y`, `None` outside the lamp.
    fn half_width(&self, y: f32) -> Option<f32> {
        if y < self.top || y >= self.bottom {
            None
        } else if y < self.glass_top {
            let v = (y - self.top) / (self.glass_top - self.top).max(1.0);
            Some(self.r * (0.28 + 0.14 * v))
        } else if y < self.glass_bottom {
            let v = (y - self.glass_top) / (self.glass_bottom - self.glass_top).max(1.0);
            Some(self.r * (0.42 + 0.58 * v))
        } else {
            let v = (y - self.glass_bottom) / (self.bottom - self.glass_bottom).max(1.0);
            Some(self.r * (1.0 - 0.35 * (PI * v).sin()))
        }
    }
}

/// Lava lamps glowing in a dark room: warm wax blobs (a metaball field)
/// rise, merge and sink inside tapered glass. Bass heats the lamps, which
/// speeds up the wax and swells the blobs; beats make them bulge, and the
/// level brightens the bulbs and the glow on the wall.
pub struct LavaLamp {
    lamps: Vec<Lamp>,
    heat: f32,
}

impl LavaLamp {
    pub fn new() -> Self {
        let lamp = |wax, hot, liquid, seed: f32| Lamp {
            wax,
            hot,
            liquid,
            phases: std::array::from_fn(|i| seed + i as f32 * 2.4),
        };
        Self {
            lamps: vec![
                lamp(rgb(1.0, 0.3, 0.05), rgb(1.0, 0.85, 0.3), rgb(0.35, 0.05, 0.12), 0.0),
                lamp(rgb(1.0, 0.2, 0.55), rgb(1.0, 0.7, 0.85), rgb(0.08, 0.06, 0.35), 1.3),
                lamp(rgb(0.6, 0.95, 0.1), rgb(1.0, 1.0, 0.6), rgb(0.02, 0.2, 0.22), 2.9),
            ],
            heat: 0.3,
        }
    }
}

impl Effect for LavaLamp {
    fn name(&self) -> &'static str {
        "Lava Lamp"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        // Wax heats up slowly and cools even slower.
        let target = 0.25 + 0.75 * f.bass;
        let rate = if target > self.heat { 1.5 } else { 0.4 };
        self.heat += (target - self.heat) * (1.0 - (-dt * rate).exp());
        for lamp in &mut self.lamps {
            for (i, p) in lamp.phases.iter_mut().enumerate() {
                *p += dt * (0.12 + 0.04 * (i % 3) as f32) * (0.4 + 1.6 * self.heat);
            }
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let light = 0.55 + 0.45 * f.level;
        let len = h * 0.9;
        let n = ((w / (len * 0.7)) as usize).clamp(1, 3);
        let order: &[usize] = match n {
            1 => &[0],
            2 => &[0, 1],
            _ => &[1, 0, 2],
        };
        let floor = h * 0.95;
        let shapes: Vec<Shape> = (0..n)
            .map(|k| {
                let s = if n == 3 && k != 1 { 0.82 } else { 1.0 };
                let l = len * s;
                let top = floor - l;
                Shape {
                    cx: w * (k as f32 + 0.5) / n as f32,
                    top,
                    glass_top: top + 0.16 * l,
                    glass_bottom: top + 0.72 * l,
                    bottom: floor,
                    r: (w / n as f32 * 0.42).min(l * 0.2).max(1.0),
                }
            })
            .collect();

        // Wall and table, lit by the lamps.
        for y in 0..c.h {
            let fy = y as f32;
            let base = if fy < floor {
                mix(rgb(0.02, 0.01, 0.03), rgb(0.06, 0.03, 0.04), fy / h)
            } else {
                rgb(0.03, 0.015, 0.01)
            };
            for x in 0..c.w {
                let mut col = base;
                for (k, s) in shapes.iter().enumerate() {
                    let lamp = &self.lamps[order[k]];
                    let (dx, dy) = (x as f32 - s.cx, fy - (s.glass_top + s.glass_bottom) * 0.5);
                    let d = (dx * dx + dy * dy * 0.5).sqrt();
                    let k = 0.45 * light * (0.6 + 0.4 * self.heat) * (-d / (s.r * 1.6)).exp();
                    col = add(col, scale(lamp.wax, if fy < floor { k } else { k * 0.6 }));
                }
                c.set(x, y, col);
            }
        }

        let swell = (0.8 + 0.4 * self.heat) * (1.0 + 0.15 * f.pulse);
        for (k, s) in shapes.iter().enumerate() {
            let lamp = &self.lamps[order[k]];
            let span = s.glass_bottom - s.glass_top;
            // Blobs as (x, y, r², vertical stretch).
            let blobs: Vec<(f32, f32, f32, f32)> = lamp
                .phases
                .iter()
                .enumerate()
                .map(|(i, &p)| {
                    let v = 0.5 + 0.5 * p.cos();
                    let y = s.glass_top + span * (0.1 + 0.85 * v);
                    let hw = s.r * (0.42 + 0.58 * v);
                    let x = s.cx + hw * 0.45 * (p * 0.6 + i as f32 * 1.9).sin();
                    let r = s.r * (0.2 + 0.06 * (i as f32 * 1.7).sin()) * swell;
                    (x, y, r * r, 1.0 + 0.5 * p.sin().abs())
                })
                .collect();
            let pool = (s.r * 0.2 * swell).powi(2);

            let y0 = (s.top.max(0.0) as usize).min(c.h);
            let y1 = (s.bottom.max(0.0).ceil() as usize).min(c.h);
            for y in y0..y1 {
                let fy = y as f32 + 0.5;
                let Some(hw) = s.half_width(fy) else { continue };
                let x0 = ((s.cx - hw).max(0.0).round() as usize).min(c.w);
                let x1 = ((s.cx + hw).max(0.0).round() as usize).min(c.w);
                let in_glass = fy >= s.glass_top && fy < s.glass_bottom;
                for x in x0..x1 {
                    let u = ((x as f32 + 0.5 - s.cx) / hw.max(0.5)).clamp(-1.0, 1.0);
                    let col = if in_glass {
                        let mut field = pool / ((s.glass_bottom - fy).powi(2) + 1.0);
                        for &(bx, by, r2, st) in &blobs {
                            let (dx, dy) = (x as f32 - bx, (fy - by) / st);
                            field += r2 / (dx * dx + dy * dy + 1.0);
                        }
                        // The bulb sits below: the liquid is brighter at the bottom.
                        let v = (fy - s.glass_top) / span.max(1.0);
                        let mut col = scale(lamp.liquid, (0.4 + 0.8 * v) * light);
                        if field >= 1.0 {
                            let shade = 0.55 + 0.45 * ((field - 1.0) * 1.5).min(1.0);
                            let wax = mix(lamp.wax, lamp.hot, (field - 1.0) * 0.4 + 0.3 * v);
                            col = scale(wax, shade * (0.7 + 0.3 * light));
                        } else if field > 0.5 {
                            col = add(col, scale(lamp.wax, (field - 0.5) * 0.5));
                        }
                        let col = scale(col, 1.0 - 0.55 * u.powi(4));
                        add(
                            col,
                            scale(rgb(1.0, 0.95, 0.9), 0.3 * (-((u + 0.55) / 0.13).powi(2)).exp()),
                        )
                    } else {
                        // Brushed metal cap and base reflecting the wax color.
                        let shine = 0.25 + 0.35 * (1.0 - u * u) + 0.5 * (-((u + 0.4) / 0.15).powi(2)).exp();
                        mix(scale(rgb(0.5, 0.48, 0.45), shine), lamp.wax, 0.15 * light)
                    };
                    c.set(x, y, col);
                }
            }
        }
    }
}
