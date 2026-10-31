use super::Effect;
use super::reaction::upscale;
use crate::analysis::Features;
use crate::gfx::Canvas;
use crate::gfx::hsv;
use std::f32::consts::TAU;

type V3 = [f32; 3];

/// Upper bound for traced rays per frame; larger canvases get upscaled.
const MAX_RAYS: f32 = 20000.0;
const SPHERES: usize = 3;

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-6 { mul(a, 1.0 / l) } else { [0.0, 1.0, 0.0] }
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[derive(Clone, Copy)]
enum Hit {
    Sphere(usize),
    Plane,
}

struct Scene {
    centers: [V3; SPHERES],
    radii: [f32; SPHERES],
    colors: [V3; SPHERES],
    light: V3,
    sky: V3,
    scroll: f32,
}

impl Scene {
    /// Nearest hit along the ray, as (distance, what).
    fn intersect(&self, ro: V3, rd: V3) -> Option<(f32, Hit)> {
        let mut best: Option<(f32, Hit)> = None;
        for i in 0..SPHERES {
            let oc = sub(ro, self.centers[i]);
            let b = dot(oc, rd);
            let disc = b * b - (dot(oc, oc) - self.radii[i] * self.radii[i]);
            if disc > 0.0 {
                let t = -b - disc.sqrt();
                if t > 1e-3 && best.is_none_or(|(bt, _)| t < bt) {
                    best = Some((t, Hit::Sphere(i)));
                }
            }
        }
        if rd[1] < -1e-4 {
            let t = -ro[1] / rd[1];
            if t > 1e-3 && best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, Hit::Plane));
            }
        }
        best
    }

    fn shadowed(&self, p: V3) -> bool {
        (0..SPHERES).any(|i| {
            let oc = sub(p, self.centers[i]);
            let b = dot(oc, self.light);
            let disc = b * b - (dot(oc, oc) - self.radii[i] * self.radii[i]);
            disc > 0.0 && -b - disc.sqrt() > 1e-3
        })
    }

    fn background(&self, rd: V3) -> V3 {
        let k = rd[1].max(0.0);
        let sun = dot(rd, self.light).max(0.0).powi(64) * 2.0;
        [
            self.sky[0] * (1.0 - k) + 0.05 * k + sun,
            self.sky[1] * (1.0 - k) + 0.08 * k + sun * 0.9,
            self.sky[2] * (1.0 - k) + 0.25 * k + sun * 0.7,
        ]
    }

    /// Local shading at a hit point, plus the reflection ray if any.
    fn shade(&self, ro: V3, rd: V3, t: f32, hit: Hit) -> (V3, V3, V3, f32) {
        let p = add(ro, mul(rd, t));
        let (n, base, refl) = match hit {
            Hit::Sphere(i) => (norm(sub(p, self.centers[i])), self.colors[i], 0.45),
            Hit::Plane => {
                let cx = (p[0] * 0.8).floor() as i32 + ((p[2] + self.scroll) * 0.8).floor() as i32;
                let c = if cx.rem_euclid(2) == 0 {
                    [0.85, 0.85, 0.9]
                } else {
                    [0.12, 0.1, 0.2]
                };
                ([0.0, 1.0, 0.0], c, 0.25)
            }
        };
        let lit = if self.shadowed(add(p, mul(n, 1e-3))) { 0.0 } else { 1.0 };
        let diff = dot(n, self.light).max(0.0) * lit;
        let r = sub(rd, mul(n, 2.0 * dot(rd, n)));
        let spec = dot(r, self.light).max(0.0).powi(40) * lit;
        let k = 0.12 + 0.88 * diff;
        let mut col = [base[0] * k + spec, base[1] * k + spec, base[2] * k + spec];
        if let Hit::Plane = hit {
            // Fade the floor into the sky with distance.
            let fog = (t / 30.0).clamp(0.0, 1.0);
            let bg = self.background(rd);
            col = [
                col[0] + (bg[0] - col[0]) * fog,
                col[1] + (bg[1] - col[1]) * fog,
                col[2] + (bg[2] - col[2]) * fog,
            ];
        }
        (col, add(p, mul(n, 1e-3)), r, refl)
    }

    fn trace(&self, ro: V3, rd: V3) -> V3 {
        let Some((t, hit)) = self.intersect(ro, rd) else {
            return self.background(rd);
        };
        let (col, p, r, refl) = self.shade(ro, rd, t, hit);
        let bounce = match self.intersect(p, r) {
            Some((t2, hit2)) => self.shade(p, r, t2, hit2).0,
            None => self.background(r),
        };
        [
            col[0] * (1.0 - refl) + bounce[0] * refl,
            col[1] * (1.0 - refl) + bounce[1] * refl,
            col[2] * (1.0 - refl) + bounce[2] * refl,
        ]
    }
}

/// Real ray tracing: three mirror-like spheres over a scrolling checker
/// floor, with shadows and one reflection bounce, traced at reduced
/// resolution and upscaled. Each sphere bobs with its own band (bass, mid,
/// treble); beats flash the sky.
pub struct Raytrace {
    orbit: f32,
    spin: f32,
    scroll: f32,
    hue: f32,
    bob: [f32; SPHERES],
    flash: f32,
    buf: Vec<[f32; 3]>,
}

impl Raytrace {
    pub fn new() -> Self {
        Self {
            orbit: 0.0,
            spin: 0.0,
            scroll: 0.0,
            hue: 0.0,
            bob: [0.0; SPHERES],
            flash: 0.0,
            buf: Vec::new(),
        }
    }
}

impl Effect for Raytrace {
    fn name(&self) -> &'static str {
        "Raytrace"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.orbit += dt * 0.15;
        self.spin += dt * (0.3 + 0.6 * f.level);
        self.scroll += dt * (0.5 + 1.5 * f.level);
        self.hue += dt * 0.02;
        self.flash = if f.beat { 1.0 } else { self.flash * (-dt * 5.0).exp() };
        let bands = [f.bass, f.mid, f.treble];
        for (b, &v) in self.bob.iter_mut().zip(&bands) {
            let k = if v > *b {
                (dt * 20.0).min(1.0)
            } else {
                (dt * 4.0).min(1.0)
            };
            *b += (v - *b) * k;
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        let k = (MAX_RAYS / (c.w * c.h) as f32).sqrt().min(1.0);
        let gw = ((c.w as f32 * k).ceil() as usize).clamp(1, c.w);
        let gh = ((c.h as f32 * k).ceil() as usize).clamp(1, c.h);

        let radii = [0.75, 0.6, 0.5];
        let mut centers = [[0.0; 3]; SPHERES];
        let mut colors = [[0.0; 3]; SPHERES];
        for i in 0..SPHERES {
            let a = self.spin + i as f32 * TAU / SPHERES as f32;
            let lift = 0.15 + 1.4 * self.bob[i] + 0.12 * (t * 1.5 + i as f32 * 2.0).sin();
            centers[i] = [a.cos() * 1.5, radii[i] + lift, a.sin() * 1.5];
            colors[i] = hsv(self.hue + i as f32 / SPHERES as f32, 0.7, 1.0).map(|v| v as f32 / 255.0);
        }
        let fl = self.flash * 0.35;
        let scene = Scene {
            centers,
            radii,
            colors,
            light: norm([-0.5 + 0.3 * (t * 0.2).sin(), 0.8, -0.4]),
            sky: [0.08 + fl, 0.03 + fl * 0.5, 0.15 + fl],
            scroll: self.scroll,
        };

        // Camera orbiting the spheres, looking slightly down.
        let (so, co) = self.orbit.sin_cos();
        let eye = [4.6 * so, 1.7 + 0.3 * (t * 0.13).sin(), -4.6 * co];
        let target = [0.0, 0.9, 0.0];
        let fwd = norm(sub(target, eye));
        let right = norm(cross([0.0, 1.0, 0.0], fwd));
        let up = cross(fwd, right);
        // Cells are square, so one focal length works for both axes.
        let focal = gw.min(gh) as f32 * 1.1;
        let (hx, hy) = (gw as f32 * 0.5, gh as f32 * 0.5);

        self.buf.clear();
        self.buf.reserve(gw * gh);
        for y in 0..gh {
            for x in 0..gw {
                let u = (x as f32 + 0.5 - hx) / focal;
                let v = (hy - y as f32 - 0.5) / focal;
                let rd = norm(add(fwd, add(mul(right, u), mul(up, v))));
                let col = scene.trace(eye, rd);
                self.buf.push([col[0].min(1.0), col[1].min(1.0), col[2].min(1.0)]);
            }
        }
        upscale(c, &self.buf, gw, gh);
    }
}
