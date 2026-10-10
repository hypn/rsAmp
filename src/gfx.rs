//! Minimal software rendering: 0RGB pixel images with clipped blits.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

pub const fn r(x: i32, y: i32, w: i32, h: i32) -> Rect {
    Rect { x, y, w, h }
}

impl Rect {
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    pub fn offset(&self, dx: i32, dy: i32) -> Rect {
        r(self.x + dx, self.y + dy, self.w, self.h)
    }
}

pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

pub fn blend(dst: u32, src: u32, a: u8) -> u32 {
    let a = a as u32;
    let ia = 255 - a;
    let ch = |s: u32| (((src >> s) & 0xff) * a + ((dst >> s) & 0xff) * ia) / 255;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

#[derive(Clone)]
pub struct Image {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u32>,
}

impl Image {
    pub fn new(w: i32, h: i32, color: u32) -> Self {
        Image { w, h, px: vec![color; (w.max(0) * h.max(0)) as usize] }
    }

    pub fn from_rgba(w: u32, h: u32, data: &[u8]) -> Self {
        let px = data.chunks_exact(4).map(|c| rgb(c[0], c[1], c[2])).collect();
        Image { w: w as i32, h: h as i32, px }
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return 0;
        }
        self.px[(y * self.w + x) as usize]
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: u32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        self.px[(y * self.w + x) as usize] = c;
    }

    pub fn fill(&mut self, rc: Rect, c: u32) {
        let x0 = rc.x.max(0);
        let y0 = rc.y.max(0);
        let x1 = (rc.x + rc.w).min(self.w);
        let y1 = (rc.y + rc.h).min(self.h);
        for y in y0..y1 {
            let row = (y * self.w) as usize;
            self.px[row + x0 as usize..row + x1.max(x0) as usize].fill(c);
        }
    }

    /// Copy `src_rc` of `src` to (dx, dy). Out-of-sheet source pixels are skipped,
    /// so a skin with a short sprite sheet degrades gracefully.
    pub fn blit(&mut self, src: &Image, src_rc: Rect, dx: i32, dy: i32) {
        self.blit_clipped(src, src_rc, dx, dy, r(0, 0, self.w, self.h));
    }

    pub fn blit_clipped(&mut self, src: &Image, src_rc: Rect, dx: i32, dy: i32, clip: Rect) {
        let cx0 = clip.x.max(0);
        let cy0 = clip.y.max(0);
        let cx1 = (clip.x + clip.w).min(self.w);
        let cy1 = (clip.y + clip.h).min(self.h);
        for j in 0..src_rc.h {
            let sy = src_rc.y + j;
            let ty = dy + j;
            if ty < cy0 || ty >= cy1 || sy < 0 || sy >= src.h {
                continue;
            }
            for i in 0..src_rc.w {
                let sx = src_rc.x + i;
                let tx = dx + i;
                if tx < cx0 || tx >= cx1 || sx < 0 || sx >= src.w {
                    continue;
                }
                self.px[(ty * self.w + tx) as usize] = src.px[(sy * src.w + sx) as usize];
            }
        }
    }

    /// Tile `src_rc` over `dst` (used by playlist edges).
    pub fn tile(&mut self, src: &Image, src_rc: Rect, dst: Rect) {
        if src_rc.w <= 0 || src_rc.h <= 0 {
            return;
        }
        let mut y = dst.y;
        while y < dst.y + dst.h {
            let mut x = dst.x;
            while x < dst.x + dst.w {
                self.blit_clipped(src, src_rc, x, y, dst);
                x += src_rc.w;
            }
            y += src_rc.h;
        }
    }

    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, c: u32) {
        self.fill(r(x0, y, x1 - x0, 1), c);
    }
    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, c: u32) {
        self.fill(r(x, y0, 1, y1 - y0), c);
    }

    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len() * 4);
        for &p in &self.px {
            out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
        }
        out
    }
}

/// For each output pixel along one axis: the first source pixel it covers and the share
/// (in 1/256ths) each of up to three consecutive source pixels contributes.
fn area_taps(src_len: i32, dst_len: i32, f: f64) -> Vec<(i32, [u32; 3])> {
    (0..dst_len)
        .map(|o| {
            let a = o as f64 / f;
            let b = ((o + 1) as f64 / f).min(src_len as f64);
            if a >= src_len as f64 || b <= a {
                return (-1, [0; 3]);
            }
            let first = a.floor() as i32;
            let mut w = [0u32; 3];
            for (k, wk) in w.iter_mut().enumerate() {
                let i = (first + k as i32) as f64;
                let cover = (b.min(i + 1.0) - a.max(i)).max(0.0);
                *wk = (cover / (b - a) * 256.0).round() as u32;
            }
            // make the shares add up to exactly 256 so flat colours stay exact
            let sum: u32 = w.iter().sum();
            let big = (0..3).max_by_key(|&k| w[k]).unwrap();
            w[big] = (w[big] + 256).saturating_sub(sum);
            (first, w)
        })
        .collect()
}

/// Scale `src` by factor `f` into a `dw`×`dh` 0RGB buffer.
///
/// Uses area averaging: each output pixel is the average of the source area it covers.
/// At whole-number factors this is plain pixel duplication (perfectly sharp). In between,
/// every skin pixel still comes out the same size, with at most a one-pixel blend at its
/// edges, instead of nearest-neighbour's mix of wide and narrow pixels.
pub fn scale_into(src: &Image, f: f64, dst: &mut [u32], dw: i32, dh: i32) {
    let tx = area_taps(src.w, dw, f);
    let ty = area_taps(src.h, dh, f);
    for (y, &(sy, wy)) in ty.iter().enumerate() {
        let row = &mut dst[y * dw as usize..(y + 1) * dw as usize];
        if sy < 0 {
            row.fill(0);
            continue;
        }
        for (out, &(sx, wx)) in row.iter_mut().zip(&tx) {
            if sx < 0 {
                *out = 0;
                continue;
            }
            if wx[0] == 256 && wy[0] == 256 {
                *out = src.px[(sy * src.w + sx) as usize];
                continue;
            }
            let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
            for (j, &wyj) in wy.iter().enumerate() {
                if wyj == 0 || sy + j as i32 >= src.h {
                    continue;
                }
                let base = ((sy + j as i32) * src.w) as usize;
                for (i, &wxi) in wx.iter().enumerate() {
                    if wxi == 0 || sx + i as i32 >= src.w {
                        continue;
                    }
                    let p = src.px[base + (sx + i as i32) as usize];
                    let w = wxi * wyj;
                    r += ((p >> 16) & 0xff) * w;
                    g += ((p >> 8) & 0xff) * w;
                    b += (p & 0xff) * w;
                }
            }
            let ch = |v: u32| ((v + 32768) >> 16).min(255);
            *out = (ch(r) << 16) | (ch(g) << 8) | ch(b);
        }
    }
}

/// Output length of `n` source pixels at factor `f`.
pub fn scaled_len(n: i32, f: f64) -> i32 {
    ((n as f64 * f).round() as i32).max(1)
}
