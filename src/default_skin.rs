//! A procedurally drawn fallback skin. It follows the classic sprite-sheet layout, so any
//! bitmap a .wsz omits can be filled in from here (the classic behaviour of falling back to a base skin).

use crate::font::tiny_glyph;
use crate::gfx::{r, rgb, Image, Rect};
use crate::sprites::*;

const FACE: u32 = rgb(52, 54, 70);
const FACE_DARK: u32 = rgb(38, 40, 54);
const LIGHT: u32 = rgb(112, 116, 140);
const SHADOW: u32 = rgb(18, 18, 26);
const BTN: u32 = rgb(150, 154, 172);
const BTN_DOWN: u32 = rgb(110, 114, 132);
const BTN_LIGHT: u32 = rgb(214, 216, 228);
const BTN_DARK: u32 = rgb(64, 66, 82);
const INK: u32 = rgb(30, 32, 44);
const LCD: u32 = rgb(0, 0, 0);
const LCD_TEXT: u32 = rgb(0, 226, 0);
const GOLD: u32 = rgb(226, 186, 92);
const GOLD_DIM: u32 = rgb(140, 120, 76);
const LED: u32 = rgb(40, 230, 40);
const LED_OFF: u32 = rgb(20, 70, 20);

pub fn vis_colors() -> [u32; 24] {
    let mut c = [0u32; 24];
    c[0] = rgb(0, 0, 0);
    c[1] = rgb(24, 33, 41);
    // 2 = top of analyzer (red) .. 17 = bottom (green)
    for i in 0..16 {
        let t = i as f32 / 15.0;
        let (r0, g0, b0) = if t < 0.5 {
            let k = t / 0.5;
            (239.0 + (214.0 - 239.0) * k, 49.0 + (181.0 - 49.0) * k, 16.0 + 17.0 * k)
        } else {
            let k = (t - 0.5) / 0.5;
            (214.0 + (24.0 - 214.0) * k, 181.0 + (132.0 - 181.0) * k, 33.0 - 25.0 * k)
        };
        c[2 + i] = rgb(r0 as u8, g0 as u8, b0 as u8);
    }
    c[18] = rgb(255, 255, 255);
    c[19] = rgb(214, 214, 222);
    c[20] = rgb(181, 189, 189);
    c[21] = rgb(160, 170, 175);
    c[22] = rgb(148, 156, 165);
    c[23] = rgb(150, 150, 150);
    c
}

pub fn sheet(name: &str) -> Image {
    match name {
        "main" => main(),
        "titlebar" => titlebar(),
        "cbuttons" => cbuttons(),
        "posbar" => posbar(),
        "volume" => volume(68, 0),
        "balance" => volume(47, 9),
        "shufrep" => shufrep(),
        "monoster" => monoster(),
        "playpaus" => playpaus(),
        "numbers" => numbers(),
        "text" => text(),
        "pledit" => pledit(),
        "eqmain" => eqmain(),
        _ => Image::new(1, 1, 0),
    }
}

// ---------- drawing helpers ----------

fn bevel(img: &mut Image, rc: Rect, face: u32, light: u32, dark: u32) {
    img.fill(rc, face);
    img.hline(rc.x, rc.x + rc.w, rc.y, light);
    img.vline(rc.x, rc.y, rc.y + rc.h, light);
    img.hline(rc.x, rc.x + rc.w, rc.y + rc.h - 1, dark);
    img.vline(rc.x + rc.w - 1, rc.y, rc.y + rc.h, dark);
}

fn sunken(img: &mut Image, rc: Rect, face: u32) {
    bevel(img, rc, face, SHADOW, LIGHT);
}

fn button(img: &mut Image, rc: Rect, down: bool) {
    if down {
        bevel(img, rc, BTN_DOWN, BTN_DARK, BTN_LIGHT);
    } else {
        bevel(img, rc, BTN, BTN_LIGHT, BTN_DARK);
    }
}

fn label(img: &mut Image, x: i32, y: i32, s: &str, c: u32) {
    let mut cx = x;
    for ch in s.chars() {
        if let Some(g) = tiny_glyph(ch) {
            for (row, bits) in g.iter().enumerate() {
                for col in 0..3 {
                    if bits & (4 >> col) != 0 {
                        img.set(cx + col, y + row as i32, c);
                    }
                }
            }
        }
        cx += 4;
    }
}

fn label_w(s: &str) -> i32 {
    s.chars().count() as i32 * 4 - 1
}

fn label_centered(img: &mut Image, rc: Rect, s: &str, c: u32) {
    label(img, rc.x + (rc.w - label_w(s)) / 2, rc.y + (rc.h - 5) / 2, s, c);
}

/// Draw '#' cells of an ASCII pattern.
fn pattern(img: &mut Image, x: i32, y: i32, rows: &[&str], c: u32) {
    for (j, row) in rows.iter().enumerate() {
        for (i, ch) in row.chars().enumerate() {
            if ch == '#' {
                img.set(x + i as i32, y + j as i32, c);
            }
        }
    }
}

fn pattern_centered(img: &mut Image, rc: Rect, rows: &[&str], c: u32) {
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0) as i32;
    let h = rows.len() as i32;
    pattern(img, rc.x + (rc.w - w) / 2, rc.y + (rc.h - h) / 2, rows, c);
}

fn gradient_v(img: &mut Image, rc: Rect, top: u32, bottom: u32) {
    for j in 0..rc.h {
        let t = if rc.h > 1 { (j * 255 / (rc.h - 1)) as u8 } else { 0 };
        img.fill(r(rc.x, rc.y + j, rc.w, 1), crate::gfx::blend(top, bottom, t));
    }
}

const ICON_PREV: &[&str] = &["#   #", "#  ##", "# ###", "#  ##", "#   #"];
const ICON_PLAY: &[&str] = &["#  ", "## ", "###", "## ", "#  "];
const ICON_PAUSE: &[&str] = &["## ##", "## ##", "## ##", "## ##", "## ##"];
const ICON_STOP: &[&str] = &["####", "####", "####", "####"];
const ICON_NEXT: &[&str] = &["#   #", "##  #", "### #", "##  #", "#   #"];
const ICON_EJECT: &[&str] = &["  #  ", " ### ", "#####", "     ", "#####"];

// ---------- sheets ----------

fn window_frame(img: &mut Image, rc: Rect) {
    gradient_v(img, rc, rgb(62, 64, 84), FACE_DARK);
    bevel_outline(img, rc, LIGHT, SHADOW);
}

fn bevel_outline(img: &mut Image, rc: Rect, light: u32, dark: u32) {
    img.hline(rc.x, rc.x + rc.w, rc.y, light);
    img.vline(rc.x, rc.y, rc.y + rc.h, light);
    img.hline(rc.x, rc.x + rc.w, rc.y + rc.h - 1, dark);
    img.vline(rc.x + rc.w - 1, rc.y, rc.y + rc.h, dark);
}

fn main() -> Image {
    let mut img = Image::new(275, 116, FACE);
    window_frame(&mut img, r(0, 0, 275, 116));
    // LCD (clutter bar + time + vis)
    sunken(&mut img, r(9, 21, 96, 45), LCD);
    for y in (POS_VIS.y..POS_VIS.y + POS_VIS.h).step_by(2) {
        for x in (POS_VIS.x..POS_VIS.x + POS_VIS.w).step_by(2) {
            img.set(x, y + 1, rgb(24, 33, 41));
        }
    }
    // colon between minutes and seconds
    img.fill(r(72, 30, 2, 2), LCD_TEXT);
    img.fill(r(72, 34, 2, 2), LCD_TEXT);
    // marquee, kbps, khz
    sunken(&mut img, r(108, 23, 159, 14), LCD);
    sunken(&mut img, r(108, 40, 21, 10), LCD);
    label(&mut img, 132, 42, "KBPS", BTN_LIGHT);
    sunken(&mut img, r(153, 40, 16, 10), LCD);
    label(&mut img, 172, 42, "KHZ", BTN_LIGHT);
    // position slider groove
    sunken(&mut img, r(15, 71, 250, 12), FACE_DARK);
    label(&mut img, 245, 94, "RS", GOLD);
    label(&mut img, 244, 100, "AMP", GOLD_DIM);
    img
}

fn title_bar(img: &mut Image, rc: Rect, active: bool, text: &str) {
    let (top, bot, stripe, ink) = if active {
        (rgb(84, 88, 120), rgb(46, 48, 70), GOLD, BTN_LIGHT)
    } else {
        (rgb(60, 62, 80), rgb(40, 42, 56), GOLD_DIM, BTN_DOWN)
    };
    gradient_v(img, rc, top, bot);
    bevel_outline(img, rc, LIGHT, SHADOW);
    let tw = label_w(text);
    let cx = rc.x + (rc.w - tw) / 2;
    // decorative stripes on either side of the title
    for k in [4, 7] {
        img.hline(rc.x + 20, cx - 6, rc.y + k, stripe);
        img.hline(cx + tw + 6, rc.x + rc.w - 36, rc.y + k, stripe);
    }
    label(img, cx, rc.y + 4, text, ink);
}

fn small_button(img: &mut Image, x: i32, y: i32, down: bool, icon: &[&str]) {
    button(img, r(x, y, 9, 9), down);
    pattern_centered(img, r(x, y, 9, 9), icon, INK);
}

const IC_OPTIONS: &[&str] = &["###", "# #", "###"];
const IC_MIN: &[&str] = &["     ", "     ", "     ", "#####"];
const IC_SHADE: &[&str] = &["#####", "     ", "#####"];
const IC_UNSHADE: &[&str] = &["#####", "#   #", "#####"];
const IC_CLOSE: &[&str] = &["#   #", " # # ", "  #  ", " # # ", "#   #"];

fn titlebar() -> Image {
    let mut img = Image::new(344, 87, rgb(255, 0, 255));
    small_button(&mut img, 0, 0, false, IC_OPTIONS);
    small_button(&mut img, 0, 9, true, IC_OPTIONS);
    small_button(&mut img, 9, 0, false, IC_MIN);
    small_button(&mut img, 9, 9, true, IC_MIN);
    small_button(&mut img, 18, 0, false, IC_CLOSE);
    small_button(&mut img, 18, 9, true, IC_CLOSE);
    small_button(&mut img, 0, 18, false, IC_SHADE);
    small_button(&mut img, 9, 18, true, IC_SHADE);
    small_button(&mut img, 0, 27, false, IC_UNSHADE);
    small_button(&mut img, 9, 27, true, IC_UNSHADE);

    for (y, active) in [(0, true), (15, false)] {
        let rc = r(27, y, 275, 14);
        title_bar(&mut img, rc, active, "RSAMP");
        title_buttons(&mut img, rc);
    }
    for (y, active) in [(29, true), (42, false)] {
        let rc = r(27, y, 275, 14);
        title_bar(&mut img, rc, active, "");
        img.fill(r(rc.x + 20, y + 2, 200, 10), if active { rgb(60, 62, 84) } else { rgb(46, 48, 62) });
        label(&mut img, rc.x + 18, y + 4, "RSAMP", if active { BTN_LIGHT } else { BTN_DOWN });
        sunken(&mut img, r(rc.x + SHADE_VIS.x - 1, y + 4, SHADE_VIS.w + 2, 7), LCD);
        sunken(&mut img, r(rc.x + 124, y + 3, 34, 8), LCD);
        let icons = [ICON_PREV, ICON_PLAY, ICON_PAUSE, ICON_STOP, ICON_NEXT, ICON_EJECT];
        for (b, ic) in SHADE_BUTTONS.iter().zip(icons) {
            pattern_centered(&mut img, b.offset(rc.x, y), ic, if active { GOLD } else { GOLD_DIM });
        }
        sunken(&mut img, r(rc.x + SHADE_POS.x, y + SHADE_POS.y, SHADE_POS.w, SHADE_POS.h), FACE_DARK);
        small_button(&mut img, rc.x + POS_OPTIONS.x, y + 3, false, IC_OPTIONS);
        small_button(&mut img, rc.x + POS_MINIMIZE.x, y + 3, false, IC_MIN);
        small_button(&mut img, rc.x + POS_SHADE.x, y + 3, false, IC_UNSHADE);
        small_button(&mut img, rc.x + POS_CLOSE.x, y + 3, false, IC_CLOSE);
    }
    sunken(&mut img, MAIN_SHADE_POS_BG, FACE_DARK);
    for (x, c) in [(17, BTN_LIGHT), (20, BTN), (23, BTN_LIGHT)] {
        bevel(&mut img, r(x, 36, 3, 7), c, BTN_LIGHT, BTN_DARK);
    }

    // clutter bar: normal, disabled, and the five "selected" variants
    let letters = ["O", "A", "I", "D", "V"];
    let draw_bar = |img: &mut Image, x: i32, y: i32, hi: Option<usize>| {
        img.fill(r(x, y, 8, 43), LCD);
        for (i, (oy, h, _)) in CLUTTER_BUTTONS.iter().enumerate() {
            let c = if hi == Some(i) { LCD_TEXT } else { rgb(70, 110, 70) };
            if hi == Some(i) {
                img.fill(r(x, y + oy, 8, *h), rgb(20, 60, 20));
            }
            label(img, x + 3, y + oy + (h - 5) / 2, letters[i], c);
        }
    };
    draw_bar(&mut img, 304, 0, None);
    draw_bar(&mut img, 312, 0, None);
    for (i, (oy, _, sx)) in CLUTTER_BUTTONS.iter().enumerate() {
        draw_bar(&mut img, *sx, 44, Some(i));
        // keep only the selected cell visible at its expected spot
        let _ = oy;
    }
    img
}

fn title_buttons(img: &mut Image, rc: Rect) {
    small_button(img, rc.x + POS_OPTIONS.x, rc.y + 3, false, IC_OPTIONS);
    small_button(img, rc.x + POS_MINIMIZE.x, rc.y + 3, false, IC_MIN);
    small_button(img, rc.x + POS_SHADE.x, rc.y + 3, false, IC_SHADE);
    small_button(img, rc.x + POS_CLOSE.x, rc.y + 3, false, IC_CLOSE);
}

fn cbuttons() -> Image {
    let mut img = Image::new(136, 36, FACE);
    let icons = [ICON_PREV, ICON_PLAY, ICON_PAUSE, ICON_STOP, ICON_NEXT, ICON_EJECT];
    for (i, &(sx, w, h, _, _)) in CBUTTONS.iter().enumerate() {
        for (row, down) in [(0, false), (h, true)] {
            let rc = r(sx, row, w, h);
            button(&mut img, rc, down);
            pattern_centered(&mut img, rc.offset(down as i32, down as i32), icons[i], INK);
        }
    }
    img
}

fn posbar() -> Image {
    let mut img = Image::new(307, 10, FACE_DARK);
    sunken(&mut img, POSBAR_BG, FACE_DARK);
    img.hline(3, 245, 5, SHADOW);
    for (rc, down) in [(POSBAR_THUMB, false), (POSBAR_THUMB_DOWN, true)] {
        button(&mut img, rc, down);
        for k in [-3, 0, 3] {
            img.vline(rc.x + 14 + k, 3, 7, BTN_DARK);
        }
    }
    img
}

fn volume(w: i32, x0: i32) -> Image {
    let mut img = Image::new(x0 + w, 433, FACE_DARK);
    for i in 0..28 {
        let y = i * 15;
        let rc = r(x0, y, w, 13);
        sunken(&mut img, rc, rgb(16, 16, 22));
        let t = i as f32 / 27.0;
        let col = rgb((40.0 + 200.0 * t) as u8, (200.0 - 120.0 * t) as u8, 40);
        let filled = ((w - 4) as f32 * t) as i32;
        img.fill(r(x0 + 2, y + 5, filled.max(1), 3), col);
    }
    for (rc, down) in [(VOL_THUMB, false), (VOL_THUMB_DOWN, true)] {
        button(&mut img, rc, down);
        img.vline(rc.x + 6, rc.y + 3, rc.y + 8, BTN_DARK);
        img.vline(rc.x + 8, rc.y + 3, rc.y + 8, BTN_DARK);
    }
    img
}

fn shufrep() -> Image {
    let mut img = Image::new(92, 85, FACE);
    let toggle = |img: &mut Image, rc: Rect, text: &str, down: bool, on: bool| {
        button(img, rc, down);
        let d = down as i32;
        img.fill(r(rc.x + 4 + d, rc.y + (rc.h - 3) / 2 + d, 3, 3), if on { LED } else { LED_OFF });
        let lw = label_w(text);
        label(img, rc.x + 9 + (rc.w - 9 - lw) / 2 + d, rc.y + (rc.h - 5) / 2 + d, text, INK);
    };
    for (row, down, on) in [(0, false, false), (15, true, false), (30, false, true), (45, true, true)] {
        toggle(&mut img, r(0, row, 28, 15), "REP", down, on);
        toggle(&mut img, r(28, row, 47, 15), "SHUFFLE", down, on);
    }
    for (x, y, down, on) in [(0, 61, false, false), (0, 73, false, true), (46, 61, true, false), (46, 73, true, true)] {
        toggle(&mut img, r(x, y, 23, 12), "EQ", down, on);
        toggle(&mut img, r(x + 23, y, 23, 12), "PL", down, on);
    }
    img
}

fn monoster() -> Image {
    let mut img = Image::new(56, 24, FACE);
    for (row, on) in [(0, true), (12, false)] {
        let c = if on { LED } else { rgb(70, 74, 90) };
        img.fill(r(0, row, 56, 12), FACE);
        label_centered(&mut img, r(0, row, 29, 12), "STEREO", c);
        label_centered(&mut img, r(29, row, 27, 12), "MONO", c);
    }
    img
}

fn playpaus() -> Image {
    let mut img = Image::new(42, 9, LCD);
    pattern(&mut img, 3, 2, ICON_PLAY, LED);
    pattern(&mut img, 11, 2, &["# #", "# #", "# #", "# #", "# #"], rgb(230, 200, 40));
    pattern(&mut img, 20, 2, &["#####", "#####", "#####", "#####", "#####"], rgb(220, 50, 40));
    // working (green lit) / not working (red lit) LEDs
    img.fill(r(36, 0, 3, 3), LED);
    img.fill(r(36, 6, 3, 3), rgb(78, 15, 0));
    img.fill(r(39, 0, 3, 3), LED_OFF);
    img.fill(r(39, 6, 3, 3), rgb(255, 40, 51));
    img
}

fn numbers() -> Image {
    // seven-segment digits, 9x13 each, then a blank cell
    let mut img = Image::new(99, 13, LCD);
    const SEGS: [u8; 10] = [
        0b1110111, 0b0010010, 0b1011101, 0b1011011, 0b0111010,
        0b1101011, 0b1101111, 0b1010010, 0b1111111, 0b1111011,
    ];
    // bit order: top, top-left, top-right, middle, bottom-left, bottom-right, bottom
    for (d, &m) in SEGS.iter().enumerate() {
        let x = d as i32 * 9;
        let seg = |img: &mut Image, bit: u8, rc: Rect| {
            if m & (1 << (6 - bit)) != 0 {
                img.fill(rc, LCD_TEXT);
            }
        };
        seg(&mut img, 0, r(x + 2, 0, 5, 2));
        seg(&mut img, 1, r(x + 1, 1, 2, 5));
        seg(&mut img, 2, r(x + 6, 1, 2, 5));
        seg(&mut img, 3, r(x + 2, 6, 5, 1));
        seg(&mut img, 4, r(x + 1, 7, 2, 5));
        seg(&mut img, 5, r(x + 6, 7, 2, 5));
        seg(&mut img, 6, r(x + 2, 11, 5, 2));
    }
    img
}

fn text() -> Image {
    let mut img = Image::new(155, 18, LCD);
    for (ch, (col, row)) in crate::font::TEXT_MAP {
        if let Some(g) = tiny_glyph(*ch) {
            for (j, bits) in g.iter().enumerate() {
                for i in 0..3 {
                    if bits & (4 >> i) != 0 {
                        img.set(col * 5 + i, row * 6 + j as i32, LCD_TEXT);
                    }
                }
            }
        }
    }
    img
}

fn pledit() -> Image {
    let mut img = Image::new(280, 186, FACE);
    // top bars (selected at y 0, unselected at y 21)
    for (y, active) in [(0, true), (21, false)] {
        let (top, bot) = if active { (rgb(84, 88, 120), rgb(46, 48, 70)) } else { (rgb(60, 62, 80), rgb(40, 42, 56)) };
        let stripe = if active { GOLD } else { GOLD_DIM };
        for rc in [r(0, y, 25, 20), r(26, y, 100, 20), r(127, y, 25, 20), r(153, y, 25, 20)] {
            gradient_v(&mut img, rc, top, bot);
            img.hline(rc.x, rc.x + rc.w, y, LIGHT);
            img.hline(rc.x, rc.x + rc.w, y + 19, FACE_DARK);
        }
        img.vline(0, y, y + 20, LIGHT);
        img.vline(177, y, y + 20, SHADOW);
        for k in [5, 8] {
            img.hline(4, 25, y + k, stripe);
            img.hline(127, 152, y + k, stripe);
            img.hline(153, 158, y + k, stripe);
        }
        label_centered(&mut img, r(26, y + 2, 100, 10), "PLAYLIST", if active { BTN_LIGHT } else { BTN_DOWN });
        small_button(&mut img, 153 + 14, y + 3, false, IC_CLOSE);
        small_button(&mut img, 153 + 5, y + 3, false, IC_SHADE);
        // inner edge of list frame
        img.hline(0, 178, y + 19, SHADOW);
    }
    // side tiles
    let lt = PL_LEFT_TILE;
    img.fill(lt, FACE);
    img.vline(lt.x, lt.y, lt.y + lt.h, LIGHT);
    img.vline(lt.x + 10, lt.y, lt.y + lt.h, SHADOW);
    img.vline(lt.x + 11, lt.y, lt.y + lt.h, LCD);
    let rt = PL_RIGHT_TILE;
    img.fill(rt, FACE);
    img.vline(rt.x, rt.y, rt.y + rt.h, LIGHT);
    img.fill(r(rt.x + 4, rt.y, 10, rt.h), FACE_DARK);
    img.vline(rt.x + 19, rt.y, rt.y + rt.h, SHADOW);
    // scroll handles
    button(&mut img, PL_SCROLL_HANDLE, false);
    button(&mut img, PL_SCROLL_HANDLE_DOWN, true);
    // close/shade pressed
    small_button(&mut img, PL_CLOSE_DOWN.x, PL_CLOSE_DOWN.y, true, IC_CLOSE);
    small_button(&mut img, PL_SHADE_DOWN.x, PL_SHADE_DOWN.y, true, IC_SHADE);
    small_button(&mut img, PL_UNSHADE_DOWN.x, PL_UNSHADE_DOWN.y, true, IC_UNSHADE);
    // shade-mode pieces
    for (rc, active) in [(PL_SHADE_LEFT, true), (PL_SHADE_BG, true)] {
        title_bar(&mut img, rc, active, "");
    }
    for (rc, active) in [(PL_SHADE_RIGHT, false), (PL_SHADE_RIGHT_SEL, true)] {
        title_bar(&mut img, rc, active, "");
        small_button(&mut img, rc.x + 32, rc.y + 3, false, IC_UNSHADE);
        small_button(&mut img, rc.x + 41, rc.y + 3, false, IC_CLOSE);
    }
    // bottom pieces
    for rc in [PL_BOTTOM_TILE, PL_BOTTOM_LEFT, PL_BOTTOM_RIGHT, PL_VIS_BG] {
        gradient_v(&mut img, rc, FACE, FACE_DARK);
        img.hline(rc.x, rc.x + rc.w, rc.y, SHADOW);
        img.hline(rc.x, rc.x + rc.w, rc.y + rc.h - 1, SHADOW);
    }
    img.vline(PL_BOTTOM_LEFT.x, PL_BOTTOM_LEFT.y, PL_BOTTOM_LEFT.y + 38, LIGHT);
    img.vline(PL_BOTTOM_RIGHT.x + 149, PL_BOTTOM_RIGHT.y, PL_BOTTOM_RIGHT.y + 38, SHADOW);
    sunken(&mut img, r(PL_VIS_BG.x + 2, PL_VIS_BG.y + 6, 70, 26), LCD);
    for (i, t) in ["ADD", "REM", "SEL", "MISC"].iter().enumerate() {
        let rc = r(PL_BOTTOM_LEFT.x + 14 + i as i32 * 29, PL_BOTTOM_LEFT.y + 12, 22, 18);
        button(&mut img, rc, false);
        label_centered(&mut img, rc, t, INK);
    }
    let br = PL_BOTTOM_RIGHT;
    sunken(&mut img, r(br.x + 5, br.y + 8, 92, 10), LCD);
    let icons = [ICON_PREV, ICON_PLAY, ICON_PAUSE, ICON_STOP, ICON_NEXT, ICON_EJECT];
    for (i, b) in PL_MINI_BUTTONS.iter().enumerate() {
        pattern_centered(&mut img, r(br.x + b.0, br.y + 22, b.1, 8), icons[i], GOLD);
    }
    sunken(&mut img, r(br.x + 64, br.y + 21, 34, 9), LCD);
    let lo = r(br.x + 106, br.y + 12, 22, 18);
    button(&mut img, lo, false);
    label(&mut img, lo.x + 3, lo.y + 3, "LIST", INK);
    label(&mut img, lo.x + 3, lo.y + 10, "OPT", INK);
    for k in 0..4 {
        img.hline(br.x + 146 - k * 3, br.x + 148, br.y + 34 - k * 3, LIGHT);
    }
    // popup menu items
    let names: [&[&str]; 5] = [
        &["URL", "DIR", "FILE"],
        &["ALL", "CROP", "SEL", "MISC"],
        &["INV", "ZERO", "ALL"],
        &["SORT", "INFO", "OPTS"],
        &["NEW", "SAVE", "LOAD"],
    ];
    let heads = ["ADD", "REM", "SEL", "MSC", "LST"];
    for (m, menu) in PL_MENUS.iter().enumerate() {
        img.fill(menu.bar, rgb(24, 24, 32));
        img.vline(menu.bar.x + 1, menu.bar.y, menu.bar.y + menu.bar.h, BTN_DARK);
        // items are listed top-to-bottom in menu order; sheet y is fixed per item
        for &(nx, sx, y) in menu.items {
            let name = names[m][((y - 111) / 19) as usize];
            for (x, sel) in [(nx, false), (sx, true)] {
                let rc = r(x, y, 22, 18);
                if sel {
                    bevel(&mut img, rc, BTN_DOWN, BTN_DARK, BTN_LIGHT);
                } else {
                    button(&mut img, rc, false);
                }
                label_centered(&mut img, r(x, y + 1, 22, 8), heads[m], INK);
                label_centered(&mut img, r(x, y + 9, 22, 8), name, INK);
            }
        }
    }
    img
}

/// Mini transport buttons in the playlist's bottom-right corner: (x offset, width).
pub const PL_MINI_BUTTONS: [(i32, i32); 6] = [(6, 8), (14, 10), (24, 10), (34, 9), (43, 8), (52, 9)];

fn eqmain() -> Image {
    let mut img = Image::new(275, 315, FACE);
    window_frame(&mut img, r(0, 0, 275, 116));
    sunken(&mut img, r(POS_EQ_GRAPH.x - 1, POS_EQ_GRAPH.y - 1, 115, 21), LCD);
    // slider tracks + scale labels
    for (i, &x) in EQ_SLIDER_X.iter().enumerate() {
        sunken(&mut img, r(x + 5, EQ_SLIDER_Y, 4, EQ_SLIDER_H), rgb(16, 16, 22));
        let names = ["PRE", "60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"];
        label_centered(&mut img, r(x - 4, 104, 22, 6), names[i], BTN_LIGHT);
    }
    label(&mut img, 44, EQ_SLIDER_Y + 1, "+12", GOLD);
    label(&mut img, 48, EQ_SLIDER_Y + 29, "+0", GOLD);
    label(&mut img, 44, EQ_SLIDER_Y + 57, "-12", GOLD);
    // close button pressed
    small_button(&mut img, EQ_CLOSE_DOWN.x, EQ_CLOSE_DOWN.y, true, IC_CLOSE);
    // on/auto buttons
    let onauto = |img: &mut Image, x: i32, w: i32, text: &str, down: bool, on: bool| {
        let rc = r(x, 119, w, 12);
        button(img, rc, down);
        img.fill(r(x + 3, 123, 3, 3), if on { LED } else { LED_OFF });
        label(img, x + 8, 122, text, INK);
    };
    onauto(&mut img, EQ_ON.x, 26, "ON", false, false);
    onauto(&mut img, EQ_AUTO.x, 32, "AUTO", false, false);
    onauto(&mut img, EQ_ON_SEL.x, 26, "ON", false, true);
    onauto(&mut img, EQ_AUTO_SEL.x, 32, "AUTO", false, true);
    onauto(&mut img, EQ_ON_DOWN.x, 26, "ON", true, false);
    onauto(&mut img, EQ_AUTO_DOWN.x, 32, "AUTO", true, false);
    onauto(&mut img, EQ_ON_SEL_DOWN.x, 26, "ON", true, true);
    onauto(&mut img, EQ_AUTO_SEL_DOWN.x, 32, "AUTO", true, true);
    // title bars
    for (rc, active) in [(EQ_TITLE_SELECTED, true), (EQ_TITLE, false)] {
        title_bar(&mut img, rc, active, "EQUALIZER");
        small_button(&mut img, rc.x + 264, rc.y + 3, false, IC_CLOSE);
        small_button(&mut img, rc.x + 254, rc.y + 3, false, IC_SHADE);
    }
    // slider bars: 28 frames, 14 per row, 15x65 cells
    for n in 0..28 {
        let x = EQ_SLIDER_BG.x + (n % 14) * 15;
        let y = EQ_SLIDER_BG.y + (n / 14) * 65;
        img.fill(r(x, y, 14, 63), FACE);
        sunken(&mut img, r(x + 5, y, 4, 63), rgb(16, 16, 22));
        let t = n as f32 / 27.0;
        let col = rgb((40.0 + 200.0 * t) as u8, (200.0 - 120.0 * t) as u8, 40);
        img.fill(r(x + 6, y + 1, 2, 61), col);
    }
    for (rc, down) in [(EQ_THUMB, false), (EQ_THUMB_DOWN, true)] {
        button(&mut img, rc, down);
        img.hline(rc.x + 3, rc.x + 8, rc.y + 5, BTN_DARK);
    }
    for (rc, down) in [(EQ_PRESETS, false), (EQ_PRESETS_DOWN, true)] {
        button(&mut img, rc, down);
        label_centered(&mut img, rc, "PRESETS", INK);
    }
    // graph background + line colours
    img.fill(EQ_GRAPH_BG, LCD);
    img.hline(0, 113, 294 + 9, rgb(40, 40, 56));
    for j in 0..19 {
        let t = j as f32 / 18.0;
        let c = rgb((240.0 - 200.0 * t) as u8, (60.0 + 160.0 * t) as u8, 40);
        img.set(EQ_GRAPH_COLORS.x, EQ_GRAPH_COLORS.y + j, c);
    }
    img.hline(0, 113, EQ_PREAMP_LINE.y, rgb(120, 120, 140));
    img
}
