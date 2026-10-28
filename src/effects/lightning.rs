use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rng, rgb};

const MAX_SEGMENTS: usize = 3000;
const MAX_BOLTS: usize = 6;

/// (x0, y0, x1, y1, brightness)
type Segment = (f32, f32, f32, f32, f32);

struct Bolt {
    segs: Vec<Segment>,
    life: f32,
}

/// Branching lightning (recursive midpoint displacement) striking from the
/// sky on every beat, with glow, sky flash and a fading afterimage. Treble
/// controls how much the bolts branch, bass how bright they are; loud
/// passages without beats still crackle now and then.
pub struct Lightning {
    core: Vec<f32>,
    glow: Vec<f32>,
    tmp: Vec<f32>,
    w: usize,
    h: usize,
    bolts: Vec<Bolt>,
    rng: Rng,
    flash: f32,
    quiet: f32,
}

impl Lightning {
    pub fn new() -> Self {
        Self {
            core: Vec::new(),
            glow: Vec::new(),
            tmp: Vec::new(),
            w: 0,
            h: 0,
            bolts: Vec::new(),
            rng: Rng::new(0x7A5E),
            flash: 0.0,
            quiet: 0.0,
        }
    }

    fn build(&mut self, segs: &mut Vec<Segment>, p0: (f32, f32), p1: (f32, f32), bright: f32, depth: u32, branch: f32) {
        let (dx, dy) = (p1.0 - p0.0, p1.1 - p0.1);
        let len = (dx * dx + dy * dy).sqrt();
        if segs.len() >= MAX_SEGMENTS {
            return;
        }
        if depth == 0 || len < 2.0 {
            segs.push((p0.0, p0.1, p1.0, p1.1, bright));
            return;
        }
        let off = self.rng.range(-0.5, 0.5) * len * 0.45;
        let (nx, ny) = (-dy / len.max(1e-3), dx / len.max(1e-3));
        let mid = ((p0.0 + p1.0) * 0.5 + nx * off, (p0.1 + p1.1) * 0.5 + ny * off);
        self.build(segs, p0, mid, bright, depth - 1, branch);
        self.build(segs, mid, p1, bright, depth - 1, branch);
        if len > 6.0 && self.rng.f32() < branch {
            // A side branch roughly continuing the local direction.
            let a = (p1.1 - mid.1).atan2(p1.0 - mid.0) + self.rng.range(0.3, 0.9) * self.sign();
            let bl = len * self.rng.range(0.5, 0.9);
            let end = (mid.0 + a.cos() * bl, mid.1 + a.sin() * bl);
            self.build(segs, mid, end, bright * 0.55, depth - 1, branch * 0.8);
        }
    }

    fn sign(&mut self) -> f32 {
        if self.rng.next_u32() & 1 == 0 { 1.0 } else { -1.0 }
    }

    fn strike(&mut self, strength: f32, branch: f32) {
        let (w, h) = (self.w as f32, self.h as f32);
        let start = (self.rng.range(0.15, 0.85) * w, 0.0);
        let end = (
            (start.0 + self.rng.range(-0.35, 0.35) * w).clamp(0.05 * w, 0.95 * w),
            h * self.rng.range(0.8, 1.05),
        );
        let mut segs = Vec::new();
        let depth = ((h.max(w)).log2().ceil() as u32).clamp(1, 8);
        self.build(&mut segs, start, end, strength, depth, branch);
        if self.bolts.len() >= MAX_BOLTS {
            self.bolts.remove(0);
        }
        self.bolts.push(Bolt { segs, life: 1.0 });
        self.flash = (self.flash + 0.6 * strength).min(1.0);
    }
}

/// Plots a line into a float buffer, keeping the brighter value.
fn line_max(buf: &mut [f32], w: usize, h: usize, s: Segment, v: f32) {
    let (x0, y0, x1, y1, _) = s;
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).ceil().clamp(1.0, 500.0) as usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let (x, y) = ((x0 + (x1 - x0) * t).round(), (y0 + (y1 - y0) * t).round());
        if x >= 0.0 && y >= 0.0 && (x as usize) < w && (y as usize) < h {
            let k = y as usize * w + x as usize;
            buf[k] = buf[k].max(v);
        }
    }
}

/// Box blur along rows (stride 1) or columns (stride w) with radius r.
fn blur(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize, vertical: bool) {
    let (n, lines, step, line_step) = if vertical { (h, w, w, 1) } else { (w, h, 1, w) };
    let norm = 1.0 / (2 * r + 1) as f32;
    for l in 0..lines {
        let base = l * line_step;
        let at = |i: isize| src[base + (i.clamp(0, n as isize - 1) as usize) * step];
        let mut sum: f32 = (-(r as isize)..=r as isize).map(at).sum();
        for i in 0..n {
            dst[base + i * step] = sum * norm;
            sum += at(i as isize + r as isize + 1) - at(i as isize - r as isize);
        }
    }
}

impl Effect for Lightning {
    fn name(&self) -> &'static str {
        "Lightning"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        if (self.w, self.h) != (c.w, c.h) {
            (self.w, self.h) = (c.w, c.h);
            self.core = vec![0.0; c.w * c.h];
            self.glow = vec![0.0; c.w * c.h];
            self.tmp = vec![0.0; c.w * c.h];
            self.bolts.clear();
        }
        let (w, h) = (self.w, self.h);
        if w == 0 || h == 0 {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        let branch = 0.12 + 0.5 * f.treble;

        self.quiet += dt;
        if f.beat {
            self.quiet = 0.0;
            self.strike(0.7 + 0.3 * f.bass, branch);
        } else if self.quiet > 1.2 && self.rng.f32() < f.level * dt * 2.0 {
            self.quiet = 0.0;
            self.strike(0.45, branch * 0.6);
        }

        // Afterimage: the core buffer only decays, live bolts are drawn on top.
        let decay = (-dt * 7.0).exp();
        for v in &mut self.core {
            *v *= decay;
        }
        for bolt in &mut self.bolts {
            let flicker = if self.rng.f32() < 0.75 { 1.0 } else { 0.35 };
            let k = bolt.life.max(0.0).sqrt() * flicker;
            for &s in &bolt.segs {
                line_max(&mut self.core, w, h, s, s.4 * k);
                // The main channel is two pixels wide.
                if s.4 > 0.6 {
                    line_max(
                        &mut self.core,
                        w,
                        h,
                        (s.0 + 1.0, s.1, s.2 + 1.0, s.3, s.4),
                        s.4 * k * 0.6,
                    );
                }
            }
            bolt.life -= dt * 3.0;
        }
        self.bolts.retain(|b| b.life > 0.0);
        self.flash *= (-dt * 9.0).exp();

        let r = (h / 30).clamp(1, 4);
        blur(&self.core, &mut self.tmp, w, h, r, false);
        blur(&self.tmp, &mut self.glow, w, h, r, true);
        blur(&self.glow, &mut self.tmp, w, h, r, false);
        blur(&self.tmp, &mut self.glow, w, h, r, true);

        let flash = self.flash + 0.05 * f.treble * self.rng.f32();
        for y in 0..h {
            let sky = 1.0 - y as f32 / h as f32;
            for x in 0..w {
                let i = y * w + x;
                let (core, glow) = (self.core[i], self.glow[i] * 3.0);
                let bg = 0.03 + 0.05 * sky + flash * 0.45 * sky;
                c.set(
                    x,
                    y,
                    rgb(
                        bg * 0.5 + glow * 0.55 + core * 0.9,
                        bg * 0.45 + glow * 0.35 + core * 0.9,
                        bg * 1.1 + glow * 1.0 + core,
                    ),
                );
            }
        }
    }
}
