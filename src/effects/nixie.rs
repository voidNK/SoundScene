use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, add, rgb, scale};
use std::f32::consts::PI;

/// Points along an elliptic arc from angle `a0` to `a1` (y points down).
fn arc(cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32) -> Vec<(f32, f32)> {
    let n = 18;
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

/// The cathode wires of one digit as polylines in a unit box.
fn strokes(d: usize) -> Vec<Vec<(f32, f32)>> {
    match d {
        0 => vec![arc(0.5, 0.5, 0.42, 0.48, 0.0, 2.0 * PI)],
        1 => vec![vec![(0.28, 0.16), (0.52, 0.0), (0.52, 1.0)]],
        2 => {
            let mut s = arc(0.5, 0.28, 0.42, 0.28, PI, 2.0 * PI);
            s.extend([(0.08, 1.0), (0.92, 1.0)]);
            vec![s]
        }
        3 => vec![
            arc(0.5, 0.26, 0.38, 0.25, PI * 1.05, PI * 2.5),
            arc(0.5, 0.75, 0.42, 0.25, PI * 1.5, PI * 2.95),
        ],
        4 => vec![vec![(0.7, 1.0), (0.7, 0.0), (0.05, 0.7), (0.95, 0.7)]],
        5 => {
            let mut s = vec![(0.88, 0.0), (0.16, 0.0), (0.1, 0.45)];
            s.extend(arc(0.5, 0.68, 0.42, 0.31, PI * 1.25, PI * 2.85));
            vec![s]
        }
        6 => vec![
            arc(0.5, 0.5, 0.42, 0.48, PI * 1.65, PI * 0.95),
            arc(0.5, 0.72, 0.42, 0.28, 0.0, 2.0 * PI),
        ],
        7 => vec![vec![(0.08, 0.0), (0.92, 0.0), (0.35, 1.0)]],
        8 => vec![
            arc(0.5, 0.26, 0.36, 0.25, 0.0, 2.0 * PI),
            arc(0.5, 0.74, 0.42, 0.26, 0.0, 2.0 * PI),
        ],
        _ => vec![
            arc(0.5, 0.28, 0.42, 0.28, 0.0, 2.0 * PI),
            arc(0.5, 0.5, 0.42, 0.48, -0.05 * PI, 0.65 * PI),
        ],
    }
}

fn seg_dist(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((px - a.0) * dx + (py - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (px - a.0 - dx * t).hypot(py - a.1 - dy * t)
}

/// Per-pixel data of one tube, shared by all tubes of the same size.
struct TubeCache {
    tw: usize,
    th: usize,
    /// Distance in pixels to each digit's wires.
    dist: Vec<Vec<f32>>,
    /// Distance to the nearest wire of any digit (the dark cathode stack).
    ghost: Vec<f32>,
    /// 0 = outside, 1 = glass, 2 = glass edge, 3 = socket.
    kind: Vec<u8>,
    mesh: Vec<bool>,
}

impl TubeCache {
    fn new(tw: usize, th: usize) -> Self {
        let (twf, thf) = (tw as f32, th as f32);
        let (mx, top, dh) = (twf * 0.2, thf * 0.2, thf * 0.56);
        let dw = twf - 2.0 * mx;
        let base = (thf * 0.12).max(1.0);
        let r = twf * 0.5;
        let mut dist = Vec::with_capacity(10);
        for d in 0..10 {
            let lines: Vec<Vec<(f32, f32)>> = strokes(d)
                .into_iter()
                .map(|l| l.into_iter().map(|(u, v)| (mx + u * dw, top + v * dh)).collect())
                .collect();
            dist.push(
                (0..tw * th)
                    .map(|i| {
                        let (px, py) = ((i % tw) as f32 + 0.5, (i / tw) as f32 + 0.5);
                        let mut best = f32::MAX;
                        for l in &lines {
                            for s in l.windows(2) {
                                best = best.min(seg_dist(px, py, s[0], s[1]));
                            }
                        }
                        best
                    })
                    .collect::<Vec<f32>>(),
            );
        }
        let ghost = (0..tw * th)
            .map(|i| dist.iter().map(|d| d[i]).fold(f32::MAX, f32::min))
            .collect();
        let m = (tw / 6).max(3);
        let kind = (0..tw * th)
            .map(|i| {
                let (px, py) = ((i % tw) as f32 + 0.5, (i / tw) as f32 + 0.5);
                // Rounded top: inside the half circle of radius tw/2.
                let edge = if py < r {
                    r - (px - r).hypot(py - r)
                } else {
                    px.min(twf - px)
                };
                if edge < 0.0 {
                    0
                } else if py > thf - base {
                    3
                } else if edge < 1.0 {
                    2
                } else {
                    1
                }
            })
            .collect();
        let mesh = (0..tw * th)
            .map(|i| {
                let (x, y) = (i % tw, i / tw);
                tw >= 10 && ((x + y).is_multiple_of(m) || (x + m * 8 - y % (m * 8)).is_multiple_of(m))
            })
            .collect();
        Self {
            tw,
            th,
            dist,
            ghost,
            kind,
            mesh,
        }
    }
}

/// A row of nixie tubes: glass, mesh and a dark stack of cathodes, one of
/// them glowing orange. They count the level, the bass and the beats (with
/// a slot-machine roll on every beat); brightness flickers like a real gas
/// discharge and surges with the pulse, and the neon separators blink on beats.
pub struct Nixie {
    cache: Option<TubeCache>,
    shown: [u32; 6],
    prev: [u32; 6],
    fade: [f32; 6],
    timer: f32,
    beats: u32,
    roll: f32,
    rng: Rng,
}

impl Nixie {
    pub fn new() -> Self {
        Self {
            cache: None,
            shown: [0; 6],
            prev: [0; 6],
            fade: [0.0; 6],
            timer: 0.0,
            beats: 0,
            roll: 0.0,
            rng: Rng::new(0x41C1),
        }
    }

    fn show(&mut self, i: usize, d: u32) {
        if self.shown[i] != d {
            self.prev[i] = self.shown[i];
            self.shown[i] = d;
            self.fade[i] = 1.0;
        }
    }
}

impl Effect for Nixie {
    fn name(&self) -> &'static str {
        "Nixie"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        if f.beat {
            self.beats = (self.beats + 1) % 100;
            self.roll = 0.3;
        }
        self.roll = (self.roll - dt).max(0.0);
        for v in &mut self.fade {
            *v = (*v - dt * 12.0).max(0.0);
        }
        self.timer -= dt;
        let narrow = c.w < c.h;
        let n = if narrow { 4 } else { 6 };
        if self.timer <= 0.0 {
            self.timer = 0.08;
            let lv = (f.level * 99.0) as u32;
            let bs = (f.bass * 99.0) as u32;
            let values = if narrow {
                [lv, self.beats, 0]
            } else {
                [lv, bs, self.beats]
            };
            for (k, v) in values.iter().take(n / 2).enumerate() {
                let rolling = k == n / 2 - 1 && self.roll > 0.0;
                let (a, b) = if rolling {
                    (self.rng.next_u32() % 10, self.rng.next_u32() % 10)
                } else {
                    (v / 10 % 10, v % 10)
                };
                self.show(2 * k, a);
                self.show(2 * k + 1, b);
            }
        }
        c.fill([6, 4, 5]);
        if c.w < 4 || c.h < 6 {
            return;
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let pairs = n / 2;
        let units = n as f32 + (n - 1) as f32 * 0.12 + (pairs - 1) as f32 * 0.4;
        let mut tw = w * 0.94 / units;
        let th = (h * 0.88).min(tw * 2.2);
        tw = tw.min(th / 1.6);
        let (twi, thi) = (tw.max(2.0) as usize, th.max(3.0) as usize);
        if self.cache.as_ref().is_none_or(|k| k.tw != twi || k.th != thi) {
            self.cache = Some(TubeCache::new(twi, thi));
        }
        let Some(tube) = self.cache.as_ref() else {
            return;
        };
        let (tw, th) = (twi as f32, thi as f32);
        let gap = tw * 0.12;
        let sep = tw * 0.4;
        let total = n as f32 * tw + (n - 1) as f32 * gap + (pairs - 1) as f32 * sep;
        let x0 = (w - total) * 0.5;
        let y0 = ((h - th) * 0.5) as i32;
        let wire = (th * 0.018).max(0.55);
        let halo = (th * 0.05).max(0.9);
        let surge = 0.85 + 0.35 * f.pulse;

        for i in 0..n {
            let x = (x0 + i as f32 * (tw + gap) + (i / 2) as f32 * sep) as i32;
            let mut b = surge * (0.9 + 0.1 * (t * 37.0 + i as f32 * 2.1).sin() * (t * 23.0).cos());
            if self.rng.f32() < 0.006 {
                b *= 0.35;
            }
            let (cur, old, fade) = (
                &tube.dist[self.shown[i] as usize],
                &tube.dist[self.prev[i] as usize],
                self.fade[i],
            );
            for py in 0..thi {
                for px in 0..twi {
                    let k = py * twi + px;
                    let kind = tube.kind[k];
                    if kind == 0 {
                        continue;
                    }
                    let glow_of = |d: f32| {
                        let core = (1.0 - (d - wire) / 0.8).clamp(0.0, 1.0);
                        let halo = (-(d / halo)).exp() * 0.55;
                        (core, halo)
                    };
                    let (core, hl) = glow_of(cur[k]);
                    let (oc, oh) = glow_of(old[k]);
                    let core = core.max(oc * fade) * b;
                    let hl = hl.max(oh * fade) * b;
                    let col = match kind {
                        3 => {
                            let v = 0.08 + 0.04 * (px as f32 / tw);
                            rgb(v, v * 0.9, v * 0.85)
                        }
                        _ => {
                            let mut col: Rgb = [12, 8, 8];
                            if tube.mesh[k] {
                                col = add(col, scale([40, 30, 24], 0.6 + hl));
                            }
                            if tube.ghost[k] < wire {
                                col = [48, 38, 32];
                            }
                            col = add(col, rgb(hl, hl * 0.32, hl * 0.06));
                            col = add(col, rgb(core, core * 0.62, core * 0.3));
                            if kind == 2 {
                                col = add(col, [55, 55, 66]);
                            }
                            // Reflection streak on the glass.
                            if (px as f32 - tw * 0.22).abs() < (tw * 0.04).max(0.5) && py as f32 > tw * 0.3 {
                                col = add(col, [22, 22, 28]);
                            }
                            col
                        }
                    };
                    c.plot(x + px as i32, y0 + py as i32, col);
                }
            }
        }

        // Neon separator dots between the pairs.
        let r = (tw * 0.09).max(0.8);
        let neon = 0.3 + 0.7 * f.pulse;
        for p in 1..pairs {
            let cx = x0 + p as f32 * 2.0 * (tw + gap) - gap * 0.5 + sep * (p as f32 - 0.5);
            for cy in [y0 as f32 + th * 0.38, y0 as f32 + th * 0.62] {
                let ri = (r * 2.5).ceil() as i32;
                for dy in -ri..=ri {
                    for dx in -ri..=ri {
                        let d = (dx as f32).hypot(dy as f32) / r;
                        let v = if d < 1.0 {
                            neon
                        } else {
                            neon * 0.5 * (-(d - 1.0) * 1.5).exp()
                        };
                        c.plot_add(cx as i32 + dx, cy as i32 + dy, rgb(v, v * 0.38, v * 0.15));
                    }
                }
            }
        }
    }
}
