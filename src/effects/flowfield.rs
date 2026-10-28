use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, hsv, scale};

const MAX_PARTICLES: usize = 5000;
/// Plane waves of the stream function: (kx, ky, omega, amplitude).
const WAVES: [(f32, f32, f32, f32); 5] = [
    (1.7, 0.9, 0.31, 1.0),
    (-0.8, 2.1, -0.23, 0.8),
    (2.6, -1.9, 0.17, 0.5),
    (-3.1, -0.7, 0.41, 0.35),
    (0.4, -3.6, -0.29, 0.3),
];

struct Particle {
    x: f32,
    y: f32,
    life: f32,
    band: usize,
}

/// Thousands of particles drifting along a divergence-free (curl) flow field
/// and leaving fading trails. Bass makes the field swirl faster and harder,
/// each particle's color and brightness follows one spectrum band.
pub struct FlowField {
    particles: Vec<Particle>,
    rng: Rng,
    time: f32,
    hue: f32,
}

impl FlowField {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            rng: Rng::new(0xF10F),
            time: 0.0,
            hue: 0.0,
        }
    }

    fn spawn(rng: &mut Rng, w: f32, h: f32) -> Particle {
        Particle {
            x: rng.f32() * w,
            y: rng.f32() * h,
            life: rng.range(1.0, 5.0),
            band: rng.next_u32() as usize % 64,
        }
    }
}

impl Effect for FlowField {
    fn name(&self) -> &'static str {
        "Flow Field"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if c.w == 0 || c.h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        c.fade(0.9);
        let (w, h) = (c.w as f32, c.h as f32);
        let unit = h;

        self.time += dt * (0.4 + 1.6 * f.bass);
        self.hue += dt * 0.02;
        if f.beat {
            self.hue += 0.05;
            self.time += 0.4;
        }

        let target = (c.w * c.h / 8).clamp(16, MAX_PARTICLES);
        self.particles.truncate(target);
        while self.particles.len() < target {
            let p = Self::spawn(&mut self.rng, w, h);
            self.particles.push(p);
        }

        // Stream function psi = sum A sin(k·p + omega t); velocity = (dpsi/dy, -dpsi/dx).
        let freq = 3.0 / unit;
        let speed = unit * (0.06 + 0.22 * f.bass + 0.15 * f.pulse);
        let phases: Vec<f32> = WAVES.iter().map(|&(_, _, om, _)| om * self.time * 3.0).collect();
        let bins = f.spectrum.len();
        for p in &mut self.particles {
            let (u, v) = (p.x * freq, p.y * freq);
            let (mut vx, mut vy) = (0.0, 0.0);
            for (&(kx, ky, _, a), &ph) in WAVES.iter().zip(&phases) {
                let cs = (kx * u + ky * v + ph).cos() * a;
                vx += ky * cs;
                vy -= kx * cs;
            }
            let (ox, oy) = (p.x, p.y);
            p.x += vx * speed * dt;
            p.y += vy * speed * dt;
            p.life -= dt;

            let band = if bins > 0 { f.spectrum[p.band * bins / 64] } else { 0.0 };
            let col = hsv(self.hue + p.band as f32 / 64.0 * 0.75, 0.8, 0.06 + 0.24 * band);
            let fade_in = p.life.clamp(0.0, 1.0);
            c.line_add(ox, oy, p.x, p.y, scale(col, fade_in));

            if p.life <= 0.0 || !(0.0..w).contains(&p.x) || !(0.0..h).contains(&p.y) {
                *p = Self::spawn(&mut self.rng, w, h);
            }
        }
    }
}
