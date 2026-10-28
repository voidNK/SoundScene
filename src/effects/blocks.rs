use super::{Effect, digits};
use crate::analysis::Features;
use crate::gfx::{Canvas, Rgb, Rng, hsv, mix, scale};

const BW: usize = 10;
const BH: usize = 20;
/// Cells of each piece in rotation 0, inside a `BOX[k]`-sized square.
const PIECES: [[(i32, i32); 4]; 7] = [
    [(0, 1), (1, 1), (2, 1), (3, 1)],
    [(0, 0), (1, 0), (0, 1), (1, 1)],
    [(1, 0), (0, 1), (1, 1), (2, 1)],
    [(1, 0), (2, 0), (0, 1), (1, 1)],
    [(0, 0), (1, 0), (1, 1), (2, 1)],
    [(0, 0), (0, 1), (1, 1), (2, 1)],
    [(2, 0), (0, 1), (1, 1), (2, 1)],
];
const BOX: [i32; 7] = [4, 2, 3, 3, 3, 3, 3];
const COLORS: [Rgb; 7] = [
    [40, 220, 230],
    [240, 220, 40],
    [170, 70, 230],
    [60, 220, 80],
    [235, 60, 60],
    [50, 100, 240],
    [245, 150, 30],
];
const CLEAR_TIME: f32 = 0.35;
const WIPE_TIME: f32 = 1.2;
/// The bot rarely loses, so the board starts over after this many lines.
const MAX_LINES: u32 = 150;

type Shape = [(i32, i32); 4];

struct Piece {
    kind: usize,
    rot: usize,
    x: i32,
    y: i32,
}

/// A falling-blocks game playing itself: a small bot picks the placement with
/// the fewest holes and the flattest stack. Loudness sets the drop speed, a
/// beat hard-drops the piece once it is lined up, cleared lines flash, and a
/// meter of blocks beside the well shows the level.
pub struct Blocks {
    shapes: [[Shape; 4]; 7],
    board: [u8; BW * BH],
    piece: Option<Piece>,
    target: (usize, i32),
    next: usize,
    bag: Vec<usize>,
    rng: Rng,
    fall: f32,
    moves: f32,
    clearing: Vec<usize>,
    clear_t: f32,
    wipe: Option<f32>,
    lines: u32,
    hue: f32,
}

impl Blocks {
    pub fn new() -> Self {
        let mut shapes = [[[(0, 0); 4]; 4]; 7];
        for (k, rots) in shapes.iter_mut().enumerate() {
            let mut cells = PIECES[k];
            for shape in rots.iter_mut() {
                let (mx, my) = (
                    cells.iter().map(|p| p.0).min().unwrap_or(0),
                    cells.iter().map(|p| p.1).min().unwrap_or(0),
                );
                *shape = cells.map(|(x, y)| (x - mx, y - my));
                cells = cells.map(|(x, y)| (BOX[k] - 1 - y, x));
            }
        }
        let mut b = Self {
            shapes,
            board: [0; BW * BH],
            piece: None,
            target: (0, 0),
            next: 0,
            bag: Vec::new(),
            rng: Rng::new(0xB10C),
            fall: 0.0,
            moves: 0.0,
            clearing: Vec::new(),
            clear_t: 0.0,
            wipe: None,
            lines: 0,
            hue: 0.6,
        };
        b.next = b.draw_piece();
        b
    }

    /// 7-bag randomizer: every piece once per bag.
    fn draw_piece(&mut self) -> usize {
        if self.bag.is_empty() {
            self.bag = (0..7).collect();
            for i in (1..7).rev() {
                let j = self.rng.next_u32() as usize % (i + 1);
                self.bag.swap(i, j);
            }
        }
        self.bag.pop().unwrap_or(0)
    }

    fn shape(&self, kind: usize, rot: usize) -> &Shape {
        &self.shapes[kind][rot % 4]
    }

    fn fits(board: &[u8; BW * BH], shape: &Shape, x: i32, y: i32) -> bool {
        shape.iter().all(|&(dx, dy)| {
            let (cx, cy) = (x + dx, y + dy);
            (0..BW as i32).contains(&cx) && (0..BH as i32).contains(&cy) && board[cy as usize * BW + cx as usize] == 0
        })
    }

    fn drop_y(board: &[u8; BW * BH], shape: &Shape, x: i32, mut y: i32) -> i32 {
        while Self::fits(board, shape, x, y + 1) {
            y += 1;
        }
        y
    }

    fn full_rows(board: &[u8; BW * BH]) -> Vec<usize> {
        (0..BH)
            .filter(|&r| board[r * BW..(r + 1) * BW].iter().all(|&v| v != 0))
            .collect()
    }

    fn remove_rows(board: &mut [u8; BW * BH], rows: &[usize]) {
        let mut out = BH;
        for r in (0..BH).rev() {
            if rows.contains(&r) {
                continue;
            }
            out -= 1;
            if out != r {
                board.copy_within(r * BW..(r + 1) * BW, out * BW);
            }
        }
        board[..out * BW].fill(0);
    }

    /// Classic placement heuristic: height, cleared lines, holes, bumpiness.
    fn evaluate(board: &[u8; BW * BH], lines: usize) -> f32 {
        let mut heights = [0i32; BW];
        let mut holes = 0;
        for (x, height) in heights.iter_mut().enumerate() {
            let top = (0..BH).find(|&y| board[y * BW + x] != 0).unwrap_or(BH);
            *height = (BH - top) as i32;
            holes += (top..BH).filter(|&y| board[y * BW + x] == 0).count();
        }
        let agg: i32 = heights.iter().sum();
        let bump: i32 = heights.windows(2).map(|p| (p[0] - p[1]).abs()).sum();
        -0.51 * agg as f32 + 0.76 * lines as f32 - 0.36 * holes as f32 - 0.18 * bump as f32
    }

    fn choose(&self, kind: usize) -> (usize, i32) {
        let mut best = (f32::MIN, (0, 0));
        for rot in 0..4 {
            let shape = self.shape(kind, rot);
            let w = shape.iter().map(|p| p.0).max().unwrap_or(0) + 1;
            for x in 0..=(BW as i32 - w) {
                if !Self::fits(&self.board, shape, x, 0) {
                    continue;
                }
                let y = Self::drop_y(&self.board, shape, x, 0);
                let mut b = self.board;
                for &(dx, dy) in shape {
                    b[(y + dy) as usize * BW + (x + dx) as usize] = 1;
                }
                let rows = Self::full_rows(&b);
                Self::remove_rows(&mut b, &rows);
                let score = Self::evaluate(&b, rows.len());
                if score > best.0 {
                    best = (score, (rot, x));
                }
            }
        }
        best.1
    }

    fn spawn(&mut self) {
        let kind = self.next;
        self.next = self.draw_piece();
        let w = self.shape(kind, 0).iter().map(|p| p.0).max().unwrap_or(0) + 1;
        let x = (BW as i32 - w) / 2;
        if !Self::fits(&self.board, self.shape(kind, 0), x, 0) || self.lines >= MAX_LINES {
            self.wipe = Some(0.0);
            return;
        }
        self.target = self.choose(kind);
        self.piece = Some(Piece { kind, rot: 0, x, y: 0 });
    }

    fn lock(&mut self) {
        let Some(p) = self.piece.take() else { return };
        for (dx, dy) in *self.shape(p.kind, p.rot) {
            let (x, y) = (p.x + dx, p.y + dy);
            if (0..BW as i32).contains(&x) && (0..BH as i32).contains(&y) {
                self.board[y as usize * BW + x as usize] = p.kind as u8 + 1;
            }
        }
        self.clearing = Self::full_rows(&self.board);
        if !self.clearing.is_empty() {
            self.clear_t = CLEAR_TIME;
            self.lines += self.clearing.len() as u32;
            self.hue += 0.07;
        }
    }

    fn update(&mut self, dt: f32, f: &Features) {
        if let Some(w) = &mut self.wipe {
            *w += dt;
            if *w > WIPE_TIME {
                self.wipe = None;
                self.board = [0; BW * BH];
                self.lines = 0;
            }
            return;
        }
        if self.clear_t > 0.0 {
            self.clear_t -= dt;
            if self.clear_t <= 0.0 {
                let rows = std::mem::take(&mut self.clearing);
                Self::remove_rows(&mut self.board, &rows);
            }
            return;
        }
        if self.piece.is_none() {
            self.spawn();
            return;
        }

        self.moves = (self.moves + dt * 25.0).min(10.0);
        while self.moves >= 1.0 {
            self.moves -= 1.0;
            let Some(p) = &self.piece else { break };
            let (kind, rot, x, y) = (p.kind, p.rot, p.x, p.y);
            let (trot, tx) = self.target;
            let (nrot, nx) = if rot != trot && Self::fits(&self.board, self.shape(kind, rot + 1), x, y) {
                (rot + 1, x)
            } else if x != tx && Self::fits(&self.board, self.shape(kind, rot), x + (tx - x).signum(), y) {
                (rot, x + (tx - x).signum())
            } else {
                (rot, x)
            };
            if let Some(p) = &mut self.piece {
                (p.rot, p.x) = (nrot % 4, nx);
            }
        }

        let Some(p) = &self.piece else { return };
        let aligned = (p.rot, p.x) == self.target;
        let (kind, rot, x, y) = (p.kind, p.rot, p.x, p.y);
        if aligned && f.beat {
            let y = Self::drop_y(&self.board, self.shape(kind, rot), x, y);
            if let Some(p) = &mut self.piece {
                p.y = y;
            }
            self.lock();
            return;
        }
        let rate = if aligned {
            6.0 + f.level * 30.0 + f.bass * 15.0
        } else {
            1.5 + f.level * 4.0
        };
        self.fall = (self.fall + dt * rate).min(BH as f32);
        while self.fall >= 1.0 {
            self.fall -= 1.0;
            let Some(p) = &mut self.piece else { break };
            let shape = &self.shapes[p.kind][p.rot % 4];
            if Self::fits(&self.board, shape, p.x, p.y + 1) {
                p.y += 1;
            } else {
                self.fall = 0.0;
                self.lock();
                break;
            }
        }
    }
}

/// A beveled block of `cs`×`cs` pixels.
fn cell(c: &mut Canvas, x: i32, y: i32, cs: i32, col: Rgb) {
    let (light, dark) = (mix(col, [255, 255, 255], 0.45), scale(col, 0.5));
    for dy in 0..cs {
        for dx in 0..cs {
            let shade = if cs < 3 {
                col
            } else if dy == 0 || dx == 0 {
                light
            } else if dy == cs - 1 || dx == cs - 1 {
                dark
            } else {
                col
            };
            c.plot(x + dx, y + dy, shade);
        }
    }
}

impl Effect for Blocks {
    fn name(&self) -> &'static str {
        "Blocks"
    }

    fn render(&mut self, c: &mut Canvas, t: f32, dt: f32, f: &Features) {
        self.update(dt.min(0.1), f);

        for y in 0..c.h {
            let v = 0.03 + 0.06 * y as f32 / c.h.max(1) as f32;
            let bg = hsv(self.hue + 0.05 * (t * 0.2).sin(), 0.7, v + 0.03 * f.pulse);
            for x in 0..c.w {
                c.set(x, y, bg);
            }
        }
        if c.w == 0 || c.h == 0 {
            return;
        }

        let cs = ((c.h / 22).min(c.w / 20).max(1)) as i32;
        let (bw, bh) = (BW as i32 * cs, BH as i32 * cs);
        let (x0, y0) = ((c.w as i32 - bw) / 2, (c.h as i32 - bh) / 2);

        let frame = hsv(self.hue, 0.4, 0.3 + 0.6 * f.pulse);
        for x in x0 - 1..=x0 + bw {
            c.plot(x, y0 - 1, frame);
            c.plot(x, y0 + bh, frame);
        }
        for y in y0 - 1..=y0 + bh {
            c.plot(x0 - 1, y, frame);
            c.plot(x0 + bw, y, frame);
        }
        let well = [8, 8, 16];
        for y in y0..y0 + bh {
            for x in x0..x0 + bw {
                c.plot(x, y, well);
            }
        }

        let flash = if self.clear_t > 0.0 {
            (self.clear_t / CLEAR_TIME * 12.0).sin().abs()
        } else {
            0.0
        };
        let wiped = self.wipe.map_or(0, |w| ((w / WIPE_TIME) * BH as f32) as usize);
        for r in 0..BH {
            for col in 0..BW {
                let (px, py) = (x0 + col as i32 * cs, y0 + r as i32 * cs);
                if BH - r <= wiped {
                    cell(c, px, py, cs, hsv(self.hue + r as f32 * 0.02, 0.2, 0.5));
                    continue;
                }
                let v = self.board[r * BW + col];
                if v == 0 {
                    continue;
                }
                let mut color = COLORS[(v as usize - 1) % 7];
                if self.clearing.contains(&r) {
                    color = mix(color, [255, 255, 255], 0.4 + 0.6 * flash);
                }
                cell(c, px, py, cs, color);
            }
        }

        if let Some(p) = &self.piece {
            let shape = self.shape(p.kind, p.rot);
            let gy = Self::drop_y(&self.board, shape, p.x, p.y);
            let col = COLORS[p.kind];
            for &(dx, dy) in shape {
                let (px, py) = (x0 + (p.x + dx) * cs, y0 + (gy + dy) * cs);
                cell(c, px, py, cs, scale(col, 0.22));
            }
            for &(dx, dy) in shape {
                cell(c, x0 + (p.x + dx) * cs, y0 + (p.y + dy) * cs, cs, col);
            }
        }

        // Next piece and line count to the right of the well.
        let side = x0 + bw + 2 * cs;
        if side + 4 * cs < c.w as i32 {
            for &(dx, dy) in self.shape(self.next, 0) {
                cell(c, side + dx * cs, y0 + cs + dy * cs, cs, COLORS[self.next]);
            }
            let ds = (cs / 2).max(1);
            digits::draw(c, side, y0 + 5 * cs, self.lines, ds, hsv(self.hue, 0.3, 0.9));
        }
        // Loudness meter made of blocks to the left.
        let mx = x0 - 3 * cs;
        if mx >= 0 {
            let n = (f.level * BH as f32).round() as i32;
            for k in 0..n.min(BH as i32) {
                let col = COLORS[k as usize % 7];
                cell(
                    c,
                    mx,
                    y0 + bh - (k + 1) * cs,
                    cs,
                    scale(col, 0.4 + 0.6 * f.pulse.max(0.3)),
                );
            }
        }
    }
}
