use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, hsv, mix, scale};
use std::f32::consts::{GOLDEN_RATIO as PHI, PI, TAU};

const MAX_LEVEL: usize = 8;

/// Robinson triangle: `thin` picks the half of a thin rhombus, otherwise a
/// thick one. B–C is the edge shared with its mirror half.
#[derive(Clone, Copy)]
struct Tri {
    thin: bool,
    a: (f32, f32),
    b: (f32, f32),
    c: (f32, f32),
}

fn lerp(p: (f32, f32), q: (f32, f32), t: f32) -> (f32, f32) {
    (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t)
}

/// One deflation step: every triangle splits into two or three smaller ones.
fn subdivide(tris: &[Tri]) -> Vec<Tri> {
    let mut out = Vec::with_capacity(tris.len() * 3);
    for t in tris {
        if !t.thin {
            let p = lerp(t.a, t.b, 1.0 / PHI);
            out.push(Tri {
                thin: false,
                a: t.c,
                b: p,
                c: t.b,
            });
            out.push(Tri {
                thin: true,
                a: p,
                b: t.c,
                c: t.a,
            });
        } else {
            let q = lerp(t.b, t.a, 1.0 / PHI);
            let r = lerp(t.b, t.c, 1.0 / PHI);
            out.push(Tri {
                thin: true,
                a: r,
                b: t.c,
                c: t.a,
            });
            out.push(Tri {
                thin: true,
                a: q,
                b: r,
                c: t.b,
            });
            out.push(Tri {
                thin: false,
                a: r,
                b: q,
                c: t.a,
            });
        }
    }
    out
}

/// Penrose P3 rhombus tiling built by deflating a wheel of Robinson
/// triangles, slowly rotating and zooming. Each rhombus is tied to a
/// spectrum band (bass in the middle, treble outside) and flashes with it;
/// beats send a bright ring outwards.
pub struct Penrose {
    levels: Vec<Vec<Tri>>,
    /// Spectrum bin and per-rhombus phase for each triangle of each level.
    tags: Vec<Vec<(u8, f32)>>,
    angle: f32,
    zoom: f32,
    hue: f32,
    ring: f32,
}

impl Penrose {
    pub fn new() -> Self {
        let mut wheel = Vec::new();
        for i in 0..10 {
            let a0 = (2 * i - 1) as f32 * PI / 10.0;
            let a1 = (2 * i + 1) as f32 * PI / 10.0;
            let mut b = (a0.cos(), a0.sin());
            let mut c = (a1.cos(), a1.sin());
            if i % 2 == 0 {
                std::mem::swap(&mut b, &mut c);
            }
            wheel.push(Tri {
                thin: false,
                a: (0.0, 0.0),
                b,
                c,
            });
        }
        let mut levels = vec![wheel];
        while let Some(last) = levels.last().filter(|_| levels.len() <= MAX_LEVEL) {
            let next = subdivide(last);
            levels.push(next);
        }
        let tags = levels
            .iter()
            .map(|tris| {
                tris.iter()
                    .map(|t| {
                        // Both halves of a rhombus share the B–C edge, so its
                        // midpoint gives them the same tag.
                        let m = lerp(t.b, t.c, 0.5);
                        let r = (m.0 * m.0 + m.1 * m.1).sqrt();
                        let hash = ((m.0 * 9_173.0) as i32 ^ (m.1 * 7_919.0) as i32).unsigned_abs();
                        let bin = (r * 44.0) as u32 + hash % 14;
                        (bin.min(63) as u8, (hash % 1000) as f32 / 1000.0)
                    })
                    .collect()
            })
            .collect();
        Self {
            levels,
            tags,
            angle: 0.0,
            zoom: 0.0,
            hue: 0.0,
            ring: 2.0,
        }
    }
}

impl Effect for Penrose {
    fn name(&self) -> &'static str {
        "Penrose"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        c.fill([0, 0, 0]);
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.angle += dt * (0.04 + 0.1 * f.bass);
        self.zoom += dt * 0.05;
        self.hue += dt * 0.015;
        self.ring += dt * 0.9;
        if f.beat && self.ring > 0.6 {
            self.ring = 0.0;
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let half_diag = 0.5 * (w * w + h * h).sqrt();
        let zoom = 1.0 + 1.2 * (0.5 - 0.5 * (self.zoom).cos()) + 0.04 * f.pulse;
        let radius = half_diag / 0.9 * zoom;
        // Pick the depth that keeps the smallest edges around 7 px.
        let level = ((radius / 7.0).max(1.0).ln() / PHI.ln()).round() as usize;
        let level = level.min(MAX_LEVEL);
        let lw = (radius * PHI.powi(-(level as i32)) * 0.09).clamp(0.6, 3.0);

        let (sn, cs) = self.angle.sin_cos();
        let (cx, cy) = (w * 0.5, h * 0.5);
        let to_screen = |p: (f32, f32)| (cx + (p.0 * cs - p.1 * sn) * radius, cy + (p.0 * sn + p.1 * cs) * radius);
        let bins = f.spectrum.len();
        let ring_r = self.ring * 1.2;
        for (tri, &(bin, ph)) in self.levels[level].iter().zip(&self.tags[level]) {
            let a = to_screen(tri.a);
            let b = to_screen(tri.b);
            let p = to_screen(tri.c);
            let xmin = a.0.min(b.0).min(p.0).max(0.0);
            let xmax = a.0.max(b.0).max(p.0).min(w - 1.0);
            let ymin = a.1.min(b.1).min(p.1).max(0.0);
            let ymax = a.1.max(b.1).max(p.1).min(h - 1.0);
            if xmin > xmax || ymin > ymax {
                continue;
            }
            let area = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
            if area.abs() < 1e-6 {
                continue;
            }
            let sign = area.signum();
            let lab = ((b.0 - a.0).hypot(b.1 - a.1)).max(1e-6);
            let lca = ((a.0 - p.0).hypot(a.1 - p.1)).max(1e-6);
            let lbc = ((p.0 - b.0).hypot(p.1 - b.1)).max(1e-6);

            let band = if bins > 0 {
                f.spectrum[(bin as usize).min(bins - 1)]
            } else {
                0.0
            };
            let m = lerp(tri.b, tri.c, 0.5);
            let r = (m.0 * m.0 + m.1 * m.1).sqrt();
            let ring = (1.0 - (r - ring_r).abs() * 6.0).max(0.0) * (1.0 - self.ring * 0.7).max(0.0);
            let twinkle = 0.5 + 0.5 * (t * 1.3 + ph * TAU).sin();
            let base_hue = if tri.thin { 0.55 } else { 0.08 };
            let value = (0.22 + 0.15 * twinkle + 0.6 * band * band + 0.6 * ring).min(1.0);
            let sat = if tri.thin { 0.7 } else { 0.8 } - 0.4 * ring.min(1.0);
            let fill = hsv(self.hue + base_hue + 0.08 * r, sat.max(0.0), value);
            let edge = scale(fill, 0.25);

            for y in ymin as usize..=ymax as usize {
                let py = y as f32 + 0.5;
                for x in xmin as usize..=xmax as usize {
                    let px = x as f32 + 0.5;
                    let dab = sign * ((b.0 - a.0) * (py - a.1) - (b.1 - a.1) * (px - a.0)) / lab;
                    let dbc = sign * ((p.0 - b.0) * (py - b.1) - (p.1 - b.1) * (px - b.0)) / lbc;
                    let dca = sign * ((a.0 - p.0) * (py - p.1) - (a.1 - p.1) * (px - p.0)) / lca;
                    if dab < -0.05 || dbc < -0.05 || dca < -0.05 {
                        continue;
                    }
                    // Outline only the rhombus edges, not the shared B–C edge.
                    let e = (dab.min(dca) / lw).min(1.0);
                    let col = if e >= 1.0 { fill } else { mix(edge, fill, e * e) };
                    c.set(x, y, col);
                }
            }
        }
    }
}
