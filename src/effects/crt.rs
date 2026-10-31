use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, rgb, scale};

/// Barrel distortion strength of the curved glass.
const CURVE: f32 = 0.09;
const BLOOM: usize = 4;

/// Where one canvas pixel looks on the tube face.
#[derive(Clone, Copy)]
struct Texel {
    /// Position in screen space, or negative x for the bezel.
    sx: f32,
    sy: f32,
    /// Vignette, or the bezel shade.
    shade: f32,
}

/// An old monitor: curved glass with scanlines, phosphor bloom, persistence
/// and a dark vignette inside a plastic bezel, showing a green waveform
/// trace that bounces up and down. Beats kick the trace, flash the picture
/// and send a roll bar and a sync jitter through it; the tube warms up from
/// a bright line whenever the effect comes on screen.
pub struct Crt {
    w: usize,
    h: usize,
    map: Vec<Texel>,
    sw: usize,
    sh: usize,
    buf: Vec<[f32; 3]>,
    bloom: Vec<[f32; 3]>,
    tmp: Vec<[f32; 3]>,
    age: f32,
    pos: f32,
    vel: f32,
    roll: f32,
    hue: f32,
}

impl Crt {
    pub fn new() -> Self {
        Self {
            w: 0,
            h: 0,
            map: Vec::new(),
            sw: 0,
            sh: 0,
            buf: Vec::new(),
            bloom: Vec::new(),
            tmp: Vec::new(),
            age: 0.0,
            pos: 0.5,
            vel: 0.0,
            roll: 2.0,
            hue: 0.0,
        }
    }

    fn layout(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        let mx = (w as f32 * 0.06).max(1.0);
        let my = (h as f32 * 0.08).max(1.0);
        let (sw, sh) = ((w as f32 - 2.0 * mx).max(1.0), (h as f32 - 2.0 * my).max(1.0));
        self.sw = sw as usize;
        self.sh = sh as usize;
        self.buf = vec![[0.0; 3]; self.sw * self.sh];
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        self.map = (0..w * h)
            .map(|i| {
                let u = ((i % w) as f32 + 0.5 - cx) / (sw * 0.5);
                let v = ((i / w) as f32 + 0.5 - cy) / (sh * 0.5);
                let (cu, cv) = (u * (1.0 + CURVE * v * v), v * (1.0 + CURVE * u * u));
                // Rounded screen corners.
                let (ex, ey) = ((cu.abs() - 0.82).max(0.0) / 0.18, (cv.abs() - 0.82).max(0.0) / 0.18);
                let out = ex * ex + ey * ey;
                if out >= 1.0 || cu.abs() >= 1.0 || cv.abs() >= 1.0 {
                    // Bezel: lighter bevel right around the glass.
                    let bevel = (1.0 - (out.sqrt() - 1.0).max(cu.abs().max(cv.abs()) - 1.0) * 6.0).clamp(0.0, 1.0);
                    let top = 0.9 + 0.2 * (1.0 - (i / w) as f32 / h as f32);
                    Texel {
                        sx: -1.0,
                        sy: 0.0,
                        shade: (0.6 + 0.6 * bevel) * top,
                    }
                } else {
                    let vig = (1.0 - 0.35 * cu * cu) * (1.0 - 0.45 * cv * cv) * (1.0 - out * 0.6);
                    Texel {
                        sx: (cu + 1.0) * 0.5 * sw,
                        sy: (cv + 1.0) * 0.5 * sh,
                        shade: vig,
                    }
                }
            })
            .collect();
    }

    /// Additive anti-aliased-ish line into the phosphor buffer.
    fn beam(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, col: [f32; 3]) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).ceil().clamp(1.0, 400.0) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            if x < 0.0 || y < 0.0 {
                continue;
            }
            let (xi, yi) = (x as usize, y as usize);
            if xi < self.sw && yi < self.sh {
                let p = &mut self.buf[yi * self.sw + xi];
                for ch in 0..3 {
                    p[ch] = (p[ch] + col[ch]).min(4.0);
                }
            }
        }
    }

    /// Box-blurred quarter-size copy of the phosphor buffer for the bloom.
    fn blur(&mut self) -> (usize, usize) {
        let (bw, bh) = (self.sw.div_ceil(BLOOM), self.sh.div_ceil(BLOOM));
        self.bloom.clear();
        self.bloom.resize(bw * bh, [0.0; 3]);
        for y in 0..self.sh {
            for x in 0..self.sw {
                let s = self.buf[y * self.sw + x];
                let d = &mut self.bloom[(y / BLOOM) * bw + x / BLOOM];
                for ch in 0..3 {
                    d[ch] += s[ch] / (BLOOM * BLOOM) as f32;
                }
            }
        }
        for _ in 0..2 {
            self.tmp.clear();
            self.tmp.extend_from_slice(&self.bloom);
            for y in 0..bh {
                for x in 0..bw {
                    let mut acc = [0.0; 3];
                    let mut n = 0.0;
                    for yy in y.saturating_sub(1)..(y + 2).min(bh) {
                        for xx in x.saturating_sub(1)..(x + 2).min(bw) {
                            let s = self.tmp[yy * bw + xx];
                            for ch in 0..3 {
                                acc[ch] += s[ch];
                            }
                            n += 1.0;
                        }
                    }
                    self.bloom[y * bw + x] = acc.map(|v| v / n);
                }
            }
        }
        (bw, bh)
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Effect for Crt {
    fn name(&self) -> &'static str {
        "CRT"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if c.w != self.w || c.h != self.h || self.map.len() != c.w * c.h {
            self.layout(c.w, c.h);
        }
        if c.w == 0 || c.h == 0 || self.sw == 0 || self.sh == 0 {
            return;
        }
        // The bezel corner is never black, so a black one means the canvas
        // was just cleared: this effect came on screen and the tube warms up.
        if c.px[0] == [0, 0, 0] {
            self.age = 0.0;
        }
        self.age += dt;

        // Trace physics: gravity pulls it down, beats kick it up.
        self.vel += dt * 2.2;
        if f.beat {
            self.vel -= 0.8 + 0.8 * f.bass;
            self.roll = 0.0;
        }
        self.pos += self.vel * dt;
        if self.pos > 0.72 {
            self.pos = 0.72;
            self.vel = -self.vel.abs() * 0.75;
        } else if self.pos < 0.28 {
            self.pos = 0.28;
            self.vel = self.vel.abs() * 0.5;
        }
        self.roll += dt * 0.9;
        self.hue = (self.hue + dt * 0.03 * (1.0 + f.level)).rem_euclid(1.0);

        let keep = (-dt * 14.0).exp();
        for p in &mut self.buf {
            *p = p.map(|v| v * keep);
        }
        let (sw, sh) = (self.sw as f32, self.sh as f32);
        let n = f.waveform.len();
        let amp = sh * 0.24 * (0.3 + 0.7 * f.level + 0.3 * f.pulse);
        let base = self.pos * sh;
        // Mostly green phosphor, drifting slightly towards cyan.
        let tint = 0.25 + 0.2 * (self.hue * std::f32::consts::TAU).sin();
        let col = [0.25 + 0.2 * f.pulse, 1.0, tint + 0.25];
        if n > 1 {
            let steps = self.sw.clamp(2, 300);
            let mut prev = (0.0, base - f.waveform[0] * amp);
            for i in 1..steps {
                let x = i as f32 / (steps - 1) as f32 * (sw - 1.0);
                let y = base - f.waveform[(i * n / steps).min(n - 1)] * amp;
                self.beam(prev.0, prev.1, x, y, col);
                prev = (x, y);
            }
        } else {
            self.beam(0.0, base, sw - 1.0, base, col);
        }
        let (bw, bh) = self.blur();

        // Warm-up: a bright line first, then the picture opens vertically.
        let open_w = smooth(0.0, 0.35, self.age).max(0.02);
        let open_h = smooth(0.25, 1.3, self.age).max(0.015);
        let bright = (0.75 + 0.45 * f.pulse) * (1.0 + 1.5 * (1.0 - open_h)) * smooth(0.0, 0.15, self.age).max(0.2);
        let scan_period = (sh / 45.0).max(2.0);
        let scan_depth = if sh >= 40.0 { 0.38 } else { 0.18 };
        let grid = (sw / 10.0).max(4.0);
        let jitter = f.pulse * f.pulse * (sw * 0.012).max(0.6);
        let bezel: Rgb = [30, 29, 34];
        // Small tubes get less bloom, or the glow would cover the whole face.
        let bloom = 2.2 * (sh / 80.0).clamp(0.2, 1.0);
        for (i, tx) in self.map.iter().enumerate() {
            if tx.sx < 0.0 {
                c.px[i] = scale(bezel, tx.shade);
                continue;
            }
            let sy = sh * 0.5 + (tx.sy - sh * 0.5) / open_h;
            let sx = sw * 0.5 + (tx.sx - sw * 0.5) / open_w + jitter * (sy * 0.9 + t * 60.0).sin();
            let mut out = [0.02, 0.045, 0.03];
            if sy >= 0.0 && sy < sh && sx >= 0.0 && sx < sw {
                let p = self.buf[sy as usize * self.sw + sx as usize];
                // Bilinear bloom sample.
                let (bx, by) = ((sx / BLOOM as f32 - 0.5).max(0.0), (sy / BLOOM as f32 - 0.5).max(0.0));
                let (x0, y0) = ((bx as usize).min(bw - 1), (by as usize).min(bh - 1));
                let (x1, y1) = ((x0 + 1).min(bw - 1), (y0 + 1).min(bh - 1));
                let (fx, fy) = (bx - x0 as f32, by - y0 as f32);
                let b = |x: usize, y: usize| self.bloom[y * bw + x];
                let (b00, b10, b01, b11) = (b(x0, y0), b(x1, y0), b(x0, y1), b(x1, y1));
                let on_grid = (sx % grid) < 0.8 || (sy % grid) < 0.8;
                for ch in 0..3 {
                    let top = b00[ch] + (b10[ch] - b00[ch]) * fx;
                    let bot = b01[ch] + (b11[ch] - b01[ch]) * fx;
                    out[ch] += p[ch] * 0.9 + (top + (bot - top) * fy) * bloom;
                }
                if on_grid {
                    out[0] += 0.02;
                    out[1] += 0.06;
                    out[2] += 0.04;
                }
            } else {
                out = [0.0; 3];
            }
            let scan = 1.0 - scan_depth * (0.5 + 0.5 * (sy / scan_period * std::f32::consts::TAU).cos());
            let rv = (sy / sh - self.roll.rem_euclid(1.6) + 0.3) * 8.0;
            let roll = 1.0 - 0.3 * (-(rv * rv)).exp();
            let k = tx.shade * scan * roll * bright;
            c.px[i] = rgb(out[0] * k, out[1] * k, out[2] * k);
        }
        // Keep the corner pixel non-black for the warm-up detection.
        if c.px[0] == [0, 0, 0] {
            c.px[0] = [1, 1, 1];
        }
    }
}
