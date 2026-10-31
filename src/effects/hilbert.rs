use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, hsv, mix};

const MAX_ORDER: u32 = 7;
/// One cycle: draw (0..1), hold, erase from the tail, pause.
const CYCLE: f32 = 2.6;

/// Hilbert curve drawn by a glowing head, then eaten from its tail. Wide
/// screens chain several curves side by side. The spectrum travels along
/// the path as a brightness wave, bass speeds up the pen, beats flash it.
pub struct Hilbert {
    phase: f32,
    travel: f32,
    hue: f32,
    pts: Vec<(f32, f32)>,
}

impl Hilbert {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            travel: 0.0,
            hue: 0.0,
            pts: Vec::new(),
        }
    }
}

/// Position `d` on a Hilbert curve filling an `n`×`n` grid (n a power of
/// two), starting at (0, 0) and ending at (n - 1, 0).
fn d2xy(n: u32, d: u32) -> (u32, u32) {
    let (mut x, mut y, mut t, mut s) = (0, 0, d, 1);
    while s < n {
        let rx = 1 & (t / 2);
        let ry = 1 & (t ^ rx);
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        x += s * rx;
        y += s * ry;
        t /= 4;
        s *= 2;
    }
    (x, y)
}

fn line(c: &mut Canvas, a: (f32, f32), b: (f32, f32), col: Rgb) {
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().clamp(1.0, 2000.0) as i32;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        c.plot(
            (a.0 + (b.0 - a.0) * t).round() as i32,
            (a.1 + (b.1 - a.1) * t).round() as i32,
            col,
        );
    }
}

impl Effect for Hilbert {
    fn name(&self) -> &'static str {
        "Hilbert Curve"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        let speed = 1.0 / 9.0 * (0.6 + 1.2 * f.bass);
        self.phase += dt * speed;
        if self.phase >= CYCLE {
            self.phase -= CYCLE;
            self.hue += 0.21;
        }
        self.travel += dt * (0.15 + 0.3 * f.mid);
        self.hue += dt * 0.02;
        c.fill([2, 2, 6]);
        if c.w < 2 || c.h < 2 {
            return;
        }

        // Chain squares across wide canvases, keep at least ~3 px per cell.
        let squares = ((c.w as f32 / c.h as f32).round() as u32).clamp(1, 4);
        let side = (c.w as f32 / squares as f32).min(c.h as f32);
        let order = ((side / 3.0).log2().floor().max(1.0) as u32).min(MAX_ORDER);
        let n = 1u32 << order;
        let sp = side / n as f32;
        let x0 = (c.w as f32 - sp * (n * squares) as f32) * 0.5 + sp * 0.5;
        let y0 = (c.h as f32 + sp * n as f32) * 0.5 - sp * 0.5;
        self.pts.clear();
        for j in 0..squares {
            for d in 0..n * n {
                let (x, y) = d2xy(n, d);
                self.pts.push((x0 + (x + j * n) as f32 * sp, y0 - y as f32 * sp));
            }
        }

        let total = self.pts.len() - 1;
        let head = (self.phase.min(1.0) * total as f32) as usize;
        let tail = ((self.phase - 1.3).clamp(0.0, 1.0) * total as f32) as usize;
        let bins = f.spectrum.len();
        let ghost = hsv(self.hue + 0.6, 0.5, 0.12);
        for i in 0..total {
            let col = if i < tail || i >= head {
                ghost
            } else {
                let u = i as f32 / total as f32;
                let w = (u * 3.0 - self.travel).rem_euclid(1.0);
                let s = if bins > 0 {
                    f.spectrum[((w * bins as f32) as usize).min(bins - 1)]
                } else {
                    0.0
                };
                // Freshly drawn part next to the pen glows white.
                let fresh = (1.0 - (head - i) as f32 / (total as f32 * 0.04 + 1.0)).max(0.0);
                let base = hsv(self.hue + u * 0.7, 0.85, 0.3 + 0.6 * s + 0.3 * f.pulse);
                mix(base, [255, 255, 255], fresh * 0.8)
            };
            line(c, self.pts[i], self.pts[i + 1], col);
        }
        if head > tail && head < total {
            let (hx, hy) = self.pts[head];
            let r = (sp * 0.6 + 1.0 + f.pulse * 2.0).max(1.0) as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let k = 1.0 - (dx * dx + dy * dy) as f32 / ((r * r) as f32 + 1.0);
                    if k > 0.0 {
                        let v = (k * 200.0) as u8;
                        c.plot_add(hx.round() as i32 + dx, hy.round() as i32 + dy, [v, v, v]);
                    }
                }
            }
        }
    }
}
