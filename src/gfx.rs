//! Pixel canvas, color helpers and a tiny RNG shared by all effects.

pub type Rgb = [u8; 3];

/// RGB framebuffer. One terminal cell holds two vertically stacked pixels,
/// so `h` is always twice the number of terminal rows used for drawing.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pub px: Vec<Rgb>,
}

impl Canvas {
    pub fn new() -> Self {
        Self { w: 0, h: 0, px: Vec::new() }
    }

    /// Returns true if the size changed (pixels are cleared in that case).
    pub fn resize(&mut self, w: usize, h: usize) -> bool {
        if w == self.w && h == self.h {
            return false;
        }
        self.w = w;
        self.h = h;
        self.px = vec![[0; 3]; w * h];
        true
    }

    pub fn clear(&mut self) {
        self.fill([0; 3]);
    }

    pub fn fill(&mut self, c: Rgb) {
        self.px.fill(c);
    }

    /// Multiplies every pixel by `k`, used for motion trails.
    pub fn fade(&mut self, k: f32) {
        for p in &mut self.px {
            *p = scale(*p, k);
        }
    }

    /// Caller guarantees `x < w` and `y < h`.
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, c: Rgb) {
        self.px[y * self.w + x] = c;
    }

    /// Additive plot with bounds check.
    pub fn plot_add(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let i = y as usize * self.w + x as usize;
            self.px[i] = add(self.px[i], c);
        }
    }

    pub fn line_add(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgb) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).ceil().clamp(1.0, 2000.0) as i32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = x0 + (x1 - x0) * t;
            let y = y0 + (y1 - y0) * t;
            self.plot_add(x.round() as i32, y.round() as i32, c);
        }
    }
}

pub fn rgb(r: f32, g: f32, b: f32) -> Rgb {
    [to_u8(r), to_u8(g), to_u8(b)]
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0) as u8
}

/// HSV to RGB; `h` wraps around, `s` and `v` are 0..1.
pub fn hsv(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    rgb(r, g, b)
}

pub fn scale(c: Rgb, k: f32) -> Rgb {
    [
        (c[0] as f32 * k) as u8,
        (c[1] as f32 * k) as u8,
        (c[2] as f32 * k) as u8,
    ]
}

pub fn add(a: Rgb, b: Rgb) -> Rgb {
    [
        a[0].saturating_add(b[0]),
        a[1].saturating_add(b[1]),
        a[2].saturating_add(b[2]),
    ]
}

/// xorshift32 — fast and good enough for visual noise.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in 0..1.
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
}
