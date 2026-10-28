use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, mix, rgb, scale};

/// Needle deflection range in radians (±48°).
const SWING: f32 = 0.84;

/// One channel: needle with spring ballistics, LED bar with peak hold.
#[derive(Default, Clone, Copy)]
struct Channel {
    pos: f32,
    vel: f32,
    bar: f32,
    peak: f32,
    hold: f32,
    lamp: f32,
}

impl Channel {
    fn update(&mut self, target: f32, dt: f32) {
        // A damped spring with a little overshoot, substepped to stay stable.
        let steps = (dt * 240.0).ceil().clamp(1.0, 64.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            let acc = 160.0 * (target - self.pos) - 16.0 * self.vel;
            self.vel += acc * h;
            self.pos = (self.pos + self.vel * h).clamp(-0.05, 1.08);
        }
        let rate = if target > self.bar { 40.0 } else { 6.0 };
        self.bar += (target - self.bar) * (1.0 - (-rate * dt).exp());
        self.hold -= dt;
        if self.bar >= self.peak {
            self.peak = self.bar;
            self.hold = 0.8;
        } else if self.hold <= 0.0 {
            self.peak = (self.peak - dt * 0.5).max(self.bar);
        }
        self.lamp = if self.pos > 0.9 {
            1.0
        } else {
            (self.lamp - dt * 2.0).max(0.0)
        };
    }
}

/// Retro hi-fi deck: two backlit analog VU meters with swinging needles and
/// red peak lamps above a pair of green/yellow/red LED chains with peak hold.
/// The left channel leans on the bass, the right one on mids and treble.
pub struct VuMeter {
    ch: [Channel; 2],
}

impl VuMeter {
    pub fn new() -> Self {
        Self {
            ch: [Channel::default(); 2],
        }
    }
}

/// Fills a rectangle, clipped to the canvas.
fn rect(c: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, col: [u8; 3]) {
    let (xa, xb) = (x0.max(0.0) as usize, (x1.max(0.0) as usize).min(c.w));
    let (ya, yb) = (y0.max(0.0) as usize, (y1.max(0.0) as usize).min(c.h));
    for y in ya..yb {
        for x in xa..xb {
            c.set(x, y, col);
        }
    }
}

/// Draws one analog meter into the given rectangle.
fn meter(c: &mut Canvas, x0: f32, y0: f32, pw: f32, ph: f32, ch: &Channel, glow: f32) {
    if pw < 3.0 || ph < 3.0 {
        return;
    }
    rect(c, x0, y0, x0 + pw, y0 + ph, [12, 10, 8]);
    let (fx0, fy0, fw, fh) = (x0 + 1.0, y0 + 1.0, pw - 2.0, ph - 2.0);
    let (cx, cy) = (fx0 + fw * 0.5, fy0 + fh * 1.08);
    let r = (fw * 0.58).min(fh * 0.92).max(1.0);
    let needle = (ch.pos * 2.0 - 1.0) * SWING;
    let (ns, nc) = needle.sin_cos();
    let ink = [25, 20, 15];
    let red = [200, 30, 20];
    let lamp_c = (fx0 + fw - fw * 0.1, fy0 + fh * 0.18);
    let lamp_r = (fh * 0.07).max(1.0);

    let (xa, xb) = (fx0.max(0.0) as usize, ((fx0 + fw).max(0.0) as usize).min(c.w));
    let (ya, yb) = (fy0.max(0.0) as usize, ((fy0 + fh).max(0.0) as usize).min(c.h));
    for y in ya..yb {
        for x in xa..xb {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Warm backlight, brightest in the middle of the face.
            let ex = (px - cx) / (fw * 0.6);
            let ey = (py - (fy0 + fh * 0.45)) / (fh * 0.7);
            let light = (1.0 - 0.6 * (ex * ex + ey * ey)).max(0.25) * (0.85 + 0.15 * glow);
            let mut col = rgb(light, light * 0.82, light * 0.5);

            let (dx, dy) = (px - cx, py - cy);
            let d = (dx * dx + dy * dy).sqrt() / r;
            let a = dx.atan2(-dy);
            if a.abs() <= SWING + 0.02 {
                let red_zone = a > SWING * 0.45;
                if (0.84..0.87).contains(&d) || (red_zone && (0.84..0.91).contains(&d)) {
                    col = if red_zone { red } else { ink };
                }
                // Eleven ticks, every other one long.
                let k = (a + SWING) / (2.0 * SWING) * 10.0;
                let ki = k.round();
                let len = if ki as i32 % 2 == 0 { 0.96 } else { 0.92 };
                if ((k - ki) * 2.0 * SWING / 10.0 * d * r).abs() < 0.5 && (0.87..len).contains(&d) {
                    col = if red_zone { red } else { ink };
                }
            }
            // Needle: thin segment from the pivot.
            let along = dx * ns - dy * nc;
            let across = (dx * nc + dy * ns).abs();
            if along > 0.0 && along < r && across < 0.6 {
                col = [20, 15, 12];
            }
            let (lx, ly) = (px - lamp_c.0, py - lamp_c.1);
            if lx * lx + ly * ly < lamp_r * lamp_r {
                col = mix([70, 10, 8], [255, 60, 40], ch.lamp);
            }
            c.set(x, y, col);
        }
    }

    // "VU" in a tiny 3×5 font, if there is room.
    if fh >= 20.0 {
        const VU: [[u8; 5]; 2] = [[5, 5, 5, 5, 2], [5, 5, 5, 5, 7]];
        let (tx, ty) = ((cx - 3.5) as i32, (cy - r * 0.42) as i32);
        for (gi, g) in VU.iter().enumerate() {
            for (row, bits) in g.iter().enumerate() {
                for col in 0..3 {
                    if bits & (4 >> col) != 0 {
                        c.plot(tx + gi as i32 * 4 + col, ty + row as i32, ink);
                    }
                }
            }
        }
    }
}

/// Draws one LED chain with peak hold.
fn leds(c: &mut Canvas, x0: f32, y0: f32, bw: f32, bh: f32, ch: &Channel) {
    if bw < 2.0 || bh < 1.0 {
        return;
    }
    let n = (bw / 4.0).clamp(1.0, 40.0) as usize;
    let seg = bw / n as f32;
    let peak = ((ch.peak * n as f32) as usize).min(n - 1);
    for i in 0..n {
        let p = i as f32 / n as f32;
        let base = if p < 0.65 {
            [40, 230, 70]
        } else if p < 0.85 {
            [250, 210, 30]
        } else {
            [255, 40, 30]
        };
        let lit = p < ch.bar || (i == peak && ch.peak > 0.02);
        let col = if lit { base } else { scale(base, 0.13) };
        let sx = x0 + i as f32 * seg;
        let gap = if seg >= 3.0 { 1.0 } else { 0.0 };
        rect(c, sx, y0, sx + seg - gap, y0 + bh, col);
    }
}

impl Effect for VuMeter {
    fn name(&self) -> &'static str {
        "VU Meter"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        let kick = 0.12 * f.pulse;
        let wobble = 0.03 * (t * 7.0).sin();
        let left = 0.6 * f.level + 0.4 * f.bass + kick + wobble;
        let right = 0.6 * f.level + 0.2 * f.mid + 0.2 * f.treble + kick - wobble;
        self.ch[0].update(left.clamp(0.0, 1.05), dt);
        self.ch[1].update(right.clamp(0.0, 1.05), dt);

        let (w, h) = (c.w as f32, c.h as f32);
        // Brushed dark metal.
        for y in 0..c.h {
            let streak = ((y as f32 * 12.9898).sin() * 43_758.547).fract().abs();
            let col = rgb(0.07 + 0.03 * streak, 0.07 + 0.03 * streak, 0.08 + 0.03 * streak);
            for x in 0..c.w {
                c.set(x, y, col);
            }
        }
        let m = (w * 0.03).max(1.0);
        let mh = h * 0.66;
        let pw = (w - 3.0 * m) * 0.5;
        for (i, ch) in self.ch.iter().enumerate() {
            meter(c, m + i as f32 * (pw + m), m, pw, mh - m, ch, f.pulse);
        }
        let ly = mh + m;
        let bh = ((h - ly - 2.0 * m) * 0.5 - 1.0).max(1.0);
        for (i, ch) in self.ch.iter().enumerate() {
            leds(c, m, ly + i as f32 * (bh + m.min(3.0)), w - 2.0 * m, bh, ch);
        }
    }
}
