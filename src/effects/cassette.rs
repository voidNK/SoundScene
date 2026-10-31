use super::Effect;
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, mix, rgb, scale};
use std::f32::consts::TAU;

/// Cassette size in millimeters, the unit of all geometry below.
const CW: f32 = 100.0;
const CH: f32 = 64.0;
/// Hub centers and tape pack radii.
const HUB_X: [f32; 2] = [28.75, 71.25];
const HUB_Y: f32 = 29.0;
const R_HUB: f32 = 6.5;
const R_MIN: f32 = 9.0;
const R_MAX: f32 = 20.0;

/// Signed distance to a rounded rectangle centered at (cx, cy).
fn rbox(x: f32, y: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (x - cx).abs() - hw + r;
    let qy = (y - cy).abs() - hh + r;
    let out = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    out + qx.max(qy).min(0.0) - r
}

/// A compact cassette seen from the front, playing in a lit deck: two reels
/// spin behind the window while the tape winds from one pack onto the other
/// and flips sides when it runs out. Level sets the tape speed, the deck
/// light and label stripes glow with the music, and beats flash them.
pub struct Cassette {
    /// 0..1, how much tape sits on the right reel.
    wound: f32,
    /// +1 plays side A (left to right), -1 side B.
    dir: f32,
    angle: [f32; 2],
    glow: f32,
    vu: [f32; 2],
}

impl Cassette {
    pub fn new() -> Self {
        Self {
            wound: 0.3,
            dir: 1.0,
            angle: [0.0, 0.7],
            glow: 0.0,
            vu: [0.0; 2],
        }
    }

    fn pack_radii(&self) -> [f32; 2] {
        // Tape area is conserved: r_l² + r_r² stays constant.
        let total = R_MAX * R_MAX - R_MIN * R_MIN;
        let r = |share: f32| (R_MIN * R_MIN + total * share).sqrt();
        [r(1.0 - self.wound), r(self.wound)]
    }
}

impl Effect for Cassette {
    fn name(&self) -> &'static str {
        "Cassette"
    }

    fn render(&mut self, c: &mut Canvas, _t: f32, dt: f32, f: &Features) {
        // Tape speed in mm/s; visibly exaggerated.
        let speed = 6.0 + 40.0 * f.level + 25.0 * f.pulse;
        let radii = self.pack_radii();
        for (a, r) in self.angle.iter_mut().zip(radii) {
            *a = (*a + dt * speed / r * self.dir).rem_euclid(TAU);
        }
        self.wound += dt * speed / 2500.0 * self.dir;
        if !(0.0..=1.0).contains(&self.wound) {
            self.wound = self.wound.clamp(0.0, 1.0);
            self.dir = -self.dir;
        }
        self.glow += (f.level.max(f.pulse) - self.glow) * (1.0 - (-dt * 8.0).exp());
        let bars = [0.6 * f.bass + 0.4 * f.level, 0.5 * f.mid + 0.5 * f.treble];
        for (v, b) in self.vu.iter_mut().zip(bars) {
            let rate = if b > *v { 25.0 } else { 4.0 };
            *v += (b - *v) * (1.0 - (-dt * rate).exp());
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        let (w, h) = (c.w as f32, c.h as f32);
        let vu_h = if h >= 16.0 { (h * 0.08).max(1.0) } else { 0.0 };
        let cw = (w * 0.92).min((h - vu_h * 3.0) * 0.88 * CW / CH).max(1.0);
        let k = cw / CW;
        let (x0, y0) = ((w - cw) * 0.5, (h - vu_h * 3.0 - CH * k) * 0.5);
        let px = 1.0 / k;
        let bins = f.spectrum.len();
        let band = |i: usize| -> f32 {
            if bins == 0 {
                0.0
            } else {
                f.spectrum[(i * bins / 4 + bins / 8).min(bins - 1)]
            }
        };
        let stripe_cols: [Rgb; 3] = [[230, 90, 30], [240, 170, 40], [200, 40, 60]];
        let warm = 0.25 + 0.75 * self.glow;
        let (sa, ca) = (self.angle.map(f32::sin), self.angle.map(f32::cos));

        for y in 0..c.h {
            for x in 0..c.w {
                // Deck backdrop: dark with a warm glow behind the cassette.
                let (nx, ny) = (x as f32 / w - 0.5, y as f32 / h - 0.45);
                let back = (1.0 - (nx * nx * 2.0 + ny * ny * 3.0)).max(0.0) * warm * 0.35;
                let mut col = rgb(0.05 + back, 0.04 + back * 0.55, 0.05 + back * 0.2);

                let mx = (x as f32 + 0.5 - x0) / k;
                let my = (y as f32 + 0.5 - y0) / k;
                let body = rbox(mx, my, CW * 0.5, CH * 0.5, CW * 0.5, CH * 0.5, 3.5);
                if body < 0.0 {
                    let shade = 0.85 + 0.15 * (1.0 - my / CH);
                    col = scale([46, 44, 52], shade);
                    if body > -px * 1.2 {
                        col = [80, 78, 90];
                    }
                    // Label with colored stripes.
                    if rbox(mx, my, 50.0, 25.0, 44.0, 20.5, 2.0) < 0.0 {
                        col = [222, 214, 190];
                        if (6.5..16.0).contains(&my) {
                            let i = ((my - 6.5) / 3.17) as usize;
                            let lift = 0.55 + 0.45 * band(i + 1) + 0.3 * f.pulse;
                            col = scale(stripe_cols[i.min(2)], lift.min(1.2));
                        }
                    }
                    // Trapezoid at the bottom with the guide holes.
                    let trap_half = 32.0 - (my - 52.0) * 0.3;
                    if my > 52.0 && (mx - 50.0).abs() < trap_half {
                        col = [36, 34, 40];
                        for hx in [18.0, 35.0, 65.0, 82.0] {
                            let r = if hx == 35.0 || hx == 65.0 { 2.6 } else { 1.6 };
                            if (mx - hx).hypot(my - 58.5) < r {
                                col = [8, 8, 10];
                            }
                        }
                    }
                    for (sx, sy) in [(5.0, 5.0), (95.0, 5.0), (5.0, 59.0), (95.0, 59.0), (50.0, 49.0)] {
                        let d = (mx - sx).hypot(my - sy);
                        if d < 1.6 {
                            col = if (mx - sx).abs() < 0.35 * d.max(px) {
                                [40, 40, 44]
                            } else {
                                [150, 150, 160]
                            };
                        }
                    }
                    // Window: lit tape packs and spinning hubs.
                    if rbox(mx, my, 50.0, HUB_Y, 36.0, 11.5, 3.0) < 0.0 {
                        let lamp = 0.35 + 0.65 * warm;
                        col = rgb(0.45 * lamp, 0.3 * lamp, 0.12 * lamp);
                        for i in 0..2 {
                            let (dx, dy) = (mx - HUB_X[i], my - HUB_Y);
                            let d = dx.hypot(dy);
                            if d < radii[i] {
                                // Wound tape with fine rings and a sheen.
                                let ring = 0.85 + 0.15 * (d * 3.1).sin();
                                let sheen = (1.0 - ((dx + dy) / radii[i] + 0.5).abs()).max(0.0) * 0.4;
                                col = rgb(
                                    (0.32 * ring + sheen) * lamp,
                                    (0.17 * ring + sheen * 0.8) * lamp,
                                    (0.09 * ring + sheen * 0.5) * lamp,
                                );
                            }
                            if d < R_HUB {
                                // Rotate into the hub frame for the six teeth.
                                let (rx, ry) = (dx * ca[i] + dy * sa[i], -dx * sa[i] + dy * ca[i]);
                                let a = ry.atan2(rx).rem_euclid(TAU / 6.0) - TAU / 12.0;
                                let tooth = a.abs() < 0.22 && d > 2.2;
                                col = if d < 3.6 && !tooth {
                                    [20, 18, 16]
                                } else if d > R_HUB - px.max(0.5) {
                                    [150, 146, 136]
                                } else {
                                    [235, 232, 222]
                                };
                                // A spoke mark so slow spins read in tiny tiles.
                                if rx > 3.6 && ry.abs() < px.max(0.6) {
                                    col = [200, 60, 40];
                                }
                            }
                        }
                    }
                }
                c.set(x, y, col);
            }
        }

        // Two LED meters under the cassette.
        if vu_h > 0.0 {
            let n = (cw / 4.0).clamp(4.0, 40.0) as usize;
            let seg = cw / n as f32;
            for (row, v) in self.vu.iter().enumerate() {
                let yy = y0 + CH * k + vu_h * (0.6 + 1.2 * row as f32);
                for i in 0..n {
                    let p = i as f32 / n as f32;
                    let base: Rgb = if p < 0.65 {
                        [60, 230, 90]
                    } else if p < 0.85 {
                        [250, 200, 40]
                    } else {
                        [255, 50, 40]
                    };
                    let on = p < *v;
                    let col = if on {
                        mix(base, [255, 255, 255], 0.2 * f.pulse)
                    } else {
                        scale(base, 0.12)
                    };
                    let xa = x0 + i as f32 * seg;
                    let xb = xa + seg - if seg >= 3.0 { 1.0 } else { 0.0 };
                    for y in yy as i32..(yy + vu_h).max(yy + 1.0) as i32 {
                        for x in xa as i32..xb.max(xa + 1.0) as i32 {
                            c.plot(x, y, col);
                        }
                    }
                }
            }
        }
    }
}
