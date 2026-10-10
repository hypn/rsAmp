//! Lenient BMP decoder for skin bitmaps that stricter decoders reject (bad size fields,
//! truncated pixel data, odd header versions). Supports 1/4/8/16/24/32 bpp, RLE4 and RLE8.

use crate::gfx::{rgb, Image};

fn u16le(d: &[u8], o: usize) -> Option<u32> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?) as u32)
}
fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

pub fn decode(d: &[u8]) -> Option<Image> {
    if d.get(0..2)? != b"BM" {
        return None;
    }
    let hsize = u32le(d, 14)? as usize;
    let (w, h, bpp, comp, ncolors, pal_entry) = if hsize == 12 {
        (u16le(d, 18)? as i32, u16le(d, 20)? as i16 as i32, u16le(d, 24)?, 0, 0, 3)
    } else {
        (u32le(d, 18)? as i32, u32le(d, 22)? as i32, u16le(d, 28)?, u32le(d, 30)?, u32le(d, 46).unwrap_or(0), 4)
    };
    let top_down = h < 0;
    let h = h.abs();
    if w <= 0 || h <= 0 || w > 8192 || h > 8192 {
        return None;
    }
    let pal_start = 14 + hsize + if comp == 3 && hsize == 40 { 12 } else { 0 };
    let pal_len = if bpp <= 8 { if ncolors == 0 || ncolors > 256 { 1 << bpp } else { ncolors as usize } } else { 0 };
    let palette: Vec<u32> = (0..pal_len)
        .map(|i| {
            let o = pal_start + i * pal_entry;
            match d.get(o..o + 3) {
                Some(c) => rgb(c[2], c[1], c[0]),
                None => 0,
            }
        })
        .collect();
    let mut off = u32le(d, 10)? as usize;
    if off == 0 || off >= d.len() {
        off = pal_start + pal_len * pal_entry;
    }
    let px = d.get(off..)?;
    let mut img = Image::new(w, h, 0);
    let row_y = |r: i32| if top_down { r } else { h - 1 - r };
    let pal = |i: usize| palette.get(i).copied().unwrap_or(0);

    match comp {
        1 | 2 => {
            // RLE8 / RLE4
            let (mut x, mut r, mut i) = (0i32, 0i32, 0usize);
            while i + 1 < px.len() && r < h {
                let (n, v) = (px[i] as usize, px[i + 1]);
                i += 2;
                if n > 0 {
                    for k in 0..n {
                        let idx = if comp == 1 { v as usize } else if k % 2 == 0 { (v >> 4) as usize } else { (v & 15) as usize };
                        img.set(x, row_y(r), pal(idx));
                        x += 1;
                    }
                } else {
                    match v {
                        0 => {
                            x = 0;
                            r += 1;
                        }
                        1 => break,
                        2 => {
                            x += *px.get(i)? as i32;
                            r += *px.get(i + 1)? as i32;
                            i += 2;
                        }
                        cnt => {
                            let cnt = cnt as usize;
                            let bytes = if comp == 1 { cnt } else { cnt.div_ceil(2) };
                            for k in 0..cnt {
                                let b = *px.get(i + if comp == 1 { k } else { k / 2 })?;
                                let idx = if comp == 1 { b as usize } else if k % 2 == 0 { (b >> 4) as usize } else { (b & 15) as usize };
                                img.set(x, row_y(r), pal(idx));
                                x += 1;
                            }
                            i += bytes + (bytes & 1);
                        }
                    }
                }
            }
        }
        0 | 3 => {
            let stride = ((w as usize * bpp as usize + 31) / 32) * 4;
            for r in 0..h {
                let Some(row) = px.get(r as usize * stride..) else { break };
                for x in 0..w as usize {
                    let c = match bpp {
                        1 => pal(row.get(x / 8).map(|b| (b >> (7 - x % 8)) & 1).unwrap_or(0) as usize),
                        4 => pal(row.get(x / 2).map(|b| if x % 2 == 0 { b >> 4 } else { b & 15 }).unwrap_or(0) as usize),
                        8 => pal(*row.get(x).unwrap_or(&0) as usize),
                        16 => {
                            let v = row.get(x * 2..x * 2 + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).unwrap_or(0);
                            let (r5, g5, b5) = ((v >> 10) & 31, (v >> 5) & 31, v & 31);
                            rgb((r5 * 255 / 31) as u8, (g5 * 255 / 31) as u8, (b5 * 255 / 31) as u8)
                        }
                        24 | 32 => {
                            let n = bpp as usize / 8;
                            match row.get(x * n..x * n + 3) {
                                Some(b) => rgb(b[2], b[1], b[0]),
                                None => 0,
                            }
                        }
                        _ => return None,
                    };
                    img.set(x as i32, row_y(r), c);
                }
            }
        }
        _ => return None,
    }
    Some(img)
}

#[cfg(test)]
mod tests {
    #[test]
    fn decodes_base_skin_like_image_crate() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("base-2.91.wsz");
        let Ok(rd) = std::fs::read_dir(&dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if !p.extension().map(|x| x.eq_ignore_ascii_case("bmp")).unwrap_or(false) {
                continue;
            }
            let data = std::fs::read(&p).unwrap();
            let ours = super::decode(&data).unwrap_or_else(|| panic!("{}", p.display()));
            let theirs = image::load_from_memory(&data).unwrap().to_rgba8();
            let theirs = crate::gfx::Image::from_rgba(theirs.width(), theirs.height(), theirs.as_raw());
            assert_eq!((ours.w, ours.h), (theirs.w, theirs.h), "{}", p.display());
            assert!(ours.px == theirs.px, "pixel mismatch in {}", p.display());
        }
    }
}
