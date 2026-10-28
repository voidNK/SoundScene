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
        Self {
            w: 0,
            h: 0,
            px: Vec::new(),
        }
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

    /// Bounds-checked overwrite.
    pub fn plot(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }

    /// Caller guarantees `x < w` and `y < h`.
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Rgb {
        self.px[y * self.w + x]
    }

    /// Copies `src` with its top-left corner at (x0, y0), clipped to this canvas.
    pub fn blit(&mut self, src: &Canvas, x0: usize, y0: usize) {
        let n = src.w.min(self.w.saturating_sub(x0));
        for y in 0..src.h.min(self.h.saturating_sub(y0)) {
            let dst = (y0 + y) * self.w + x0;
            self.px[dst..dst + n].copy_from_slice(&src.px[y * src.w..y * src.w + n]);
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

/// Rotates the hue of every pixel of `src` by `degrees` into `dst`, keeping
/// luminance roughly constant (same matrix as CSS `hue-rotate()`).
pub fn hue_rotate(dst: &mut Canvas, src: &Canvas, degrees: f32) {
    dst.resize(src.w, src.h);
    let (s, c) = degrees.to_radians().sin_cos();
    let m = [
        [
            0.213 + c * 0.787 - s * 0.213,
            0.715 - c * 0.715 - s * 0.715,
            0.072 - c * 0.072 + s * 0.928,
        ],
        [
            0.213 - c * 0.213 + s * 0.143,
            0.715 + c * 0.285 + s * 0.140,
            0.072 - c * 0.072 - s * 0.283,
        ],
        [
            0.213 - c * 0.213 - s * 0.787,
            0.715 - c * 0.715 + s * 0.715,
            0.072 + c * 0.928 + s * 0.072,
        ],
    ];
    for (d, p) in dst.px.iter_mut().zip(&src.px) {
        let [r, g, b] = p.map(|v| v as f32);
        *d = m.map(|row| (row[0] * r + row[1] * g + row[2] * b).clamp(0.0, 255.0) as u8);
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

/// Linear blend from `a` (t = 0) to `b` (t = 1).
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    [l(a[0], b[0]), l(a[1], b[1]), l(a[2], b[2])]
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
