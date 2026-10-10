//! Drawing of the main, equalizer and playlist windows from the skin's sprite sheets.

use crate::app::{App, Drag, Hit, Status, Win, ROW_H};
use crate::font::{draw_text, TtfFont};
use crate::gfx::{r, Image, Rect};
use crate::media::fmt_time;
use crate::sprites::*;

impl App {
    pub fn render(&mut self) -> Image {
        self.pl_text.clear();
        let l = self.layout();
        let mut img = Image::new(l.w, l.h, 0);
        let mut main = Image::new(275, l.main.h, 0);
        self.draw_main(&mut main);
        img.blit(&main, r(0, 0, main.w, main.h), 0, 0);
        if let Some(e) = l.eq {
            let mut eq = Image::new(275, e.h, 0);
            self.draw_eq(&mut eq);
            img.blit(&eq, r(0, 0, eq.w, eq.h), e.x, e.y);
        }
        if let Some(p) = l.pl {
            let mut pl = Image::new(p.w, p.h, 0);
            self.draw_playlist(&mut pl);
            img.blit(&pl, r(0, 0, pl.w, pl.h), p.x, p.y);
            for t in &mut self.pl_text {
                t.offset(p.x, p.y);
            }
        }
        img
    }

    /// Draw the playlist text collected by `render` onto the frame after it has been scaled
    /// by `text_scale`, so the TrueType font is rasterized at screen resolution.
    pub fn draw_zoomed_text(&mut self, img: &mut Image) {
        let f = self.text_scale;
        let rows = std::mem::take(&mut self.pl_text);
        if let Some(font) = self.zoomed_font(f) {
            for t in &rows {
                t.draw(font, img, f);
            }
        }
        self.pl_text = rows;
    }

    fn is_active(&self, w: Win) -> bool {
        self.focused && self.active == w
    }

    fn down(&self, h: Hit) -> bool {
        self.pressed == Some(h) && self.hit_test(self.mouse.0, self.mouse.1) == h
    }

    fn time_parts(&self) -> Option<(bool, u64, u64)> {
        if self.status == Status::Stopped {
            return None;
        }
        let pos = match (self.seek_preview, self.duration()) {
            (Some(f), Some(d)) if self.drag == Drag::Slider(Hit::Pos) => f * d,
            _ => self.position(),
        };
        let (neg, t) = match (self.cfg.time_remaining, self.duration()) {
            (true, Some(d)) => (true, (d - pos).max(0.0)),
            _ => (false, pos),
        };
        let t = t as u64;
        Some((neg, (t / 60) % 100, t % 60))
    }

    fn blink_hidden(&self) -> bool {
        self.status == Status::Paused && (self.start.elapsed().as_millis() / 500) % 2 == 1
    }

    fn pos_fraction(&self) -> Option<f64> {
        if self.status == Status::Stopped {
            return None;
        }
        if self.drag == Drag::Slider(Hit::Pos) {
            return self.seek_preview;
        }
        let d = self.duration()?;
        (d > 0.0).then(|| (self.position() / d).clamp(0.0, 1.0))
    }

    // ------------------------------------------------------------------ main

    fn draw_main(&mut self, img: &mut Image) {
        let s = &self.skin;
        let active = self.is_active(Win::Main);
        if self.cfg.main_shade {
            img.blit(&s.titlebar, if active { MAIN_SHADE_BG_SELECTED } else { MAIN_SHADE_BG }, 0, 0);
            self.draw_title_buttons(img, true);
            self.analyzer.draw_small(img, SHADE_VIS, &self.skin.vis_colors, self.cfg.vis_mode);
            let s = &self.skin;
            if let Some((neg, m, sec)) = self.time_parts() {
                if !self.blink_hidden() {
                    let (x, y) = SHADE_TIME;
                    let clip = r(0, 0, 275, 14);
                    draw_text(img, s, x, y, if neg { "-" } else { " " }, clip);
                    draw_text(img, s, x + 6, y, &format!("{:02}", m), clip);
                    draw_text(img, s, x + 20, y, &format!("{:02}", sec), clip);
                }
            }
            if let Some(f) = self.pos_fraction() {
                img.blit(&s.titlebar, MAIN_SHADE_POS_BG, SHADE_POS.x, SHADE_POS.y);
                let x = SHADE_POS.x + (f * (SHADE_POS.w - 3) as f64).round() as i32;
                let spr = if f < 0.33 {
                    MAIN_SHADE_POS_THUMB_LEFT
                } else if f > 0.66 {
                    MAIN_SHADE_POS_THUMB_RIGHT
                } else {
                    MAIN_SHADE_POS_THUMB
                };
                img.blit(&s.titlebar, spr, x, SHADE_POS.y);
            }
            return;
        }

        img.blit(&s.main, r(0, 0, 275, 116), 0, 0);
        img.blit(&s.titlebar, if active { MAIN_TITLE_BAR_SELECTED } else { MAIN_TITLE_BAR }, 0, 0);
        self.draw_title_buttons(img, false);
        let s = &self.skin;

        // clutter bar
        img.blit(&s.titlebar, CLUTTER_BG, POS_CLUTTER.x, POS_CLUTTER.y);
        for (i, &(oy, h, sx)) in CLUTTER_BUTTONS.iter().enumerate() {
            let on = match i {
                1 => self.cfg.always_on_top,
                3 => self.cfg.double_size,
                _ => false,
            };
            if on || self.down(Hit::Clutter(i as u8)) {
                img.blit(&s.titlebar, r(sx, 44 + oy, 8, h), POS_CLUTTER.x, POS_CLUTTER.y + oy);
            }
        }

        // play state indicator
        let (px, py) = POS_PLAYPAUS;
        match self.status {
            Status::Playing => {
                img.blit(&s.playpaus, IND_PLAYING, px, py);
                img.blit(&s.playpaus, IND_WORKING, px - 2, py);
            }
            Status::Paused => img.blit(&s.playpaus, IND_PAUSED, px, py),
            Status::Stopped => img.blit(&s.playpaus, IND_STOPPED, px, py),
        }

        // time
        if let Some((neg, m, sec)) = self.time_parts() {
            if !self.blink_hidden() {
                if s.nums_ex {
                    img.blit(&s.numbers, if neg { r(99, 0, 9, 13) } else { r(90, 0, 9, 13) }, 36, 26);
                } else if neg {
                    img.blit(&s.numbers, r(20, 6, 5, 1), 36, 32);
                }
                let d = |n: u64| r(n as i32 * 9, 0, 9, 13);
                img.blit(&s.numbers, d(m / 10), 48, 26);
                img.blit(&s.numbers, d(m % 10), 60, 26);
                img.blit(&s.numbers, d(sec / 10), 78, 26);
                img.blit(&s.numbers, d(sec % 10), 90, 26);
            }
        }

        // visualizer
        self.analyzer.draw(img, POS_VIS, &s.vis_colors, self.cfg.vis_mode);

        // marquee
        let (text, scroll) = self.marquee_text();
        let w = text.chars().count() as i32 * 5;
        if scroll && w > POS_MARQUEE.w {
            let full = format!("{text}  ***  ");
            let fw = full.chars().count() as i32 * 5;
            let off = self.marquee_offset % fw;
            let doubled = format!("{full}{full}");
            draw_text(img, s, POS_MARQUEE.x - off, POS_MARQUEE.y, &doubled, POS_MARQUEE);
        } else {
            draw_text(img, s, POS_MARQUEE.x, POS_MARQUEE.y, &text, POS_MARQUEE);
        }

        // kbps / khz / mono / stereo
        if self.status != Status::Stopped {
            if let Some(info) = &self.current_info {
                if let Some(k) = info.kbps {
                    let t = format!("{:>3}", k.min(999));
                    draw_text(img, s, POS_KBPS.0, POS_KBPS.1, &t, r(POS_KBPS.0, POS_KBPS.1, 15, 6));
                }
                let khz = info.khz.or_else(|| self.audio.info().map(|i| i.sample_rate / 1000));
                if let Some(k) = khz {
                    let t = format!("{:>2}", k.min(99));
                    draw_text(img, s, POS_KHZ.0, POS_KHZ.1, &t, r(POS_KHZ.0, POS_KHZ.1, 10, 6));
                }
            }
        }
        let channels = if self.status == Status::Stopped {
            0
        } else {
            self.current_info
                .as_ref()
                .and_then(|i| i.channels)
                .or_else(|| self.audio.info().map(|i| i.channels))
                .unwrap_or(2)
        };
        img.blit(&s.monoster, if channels >= 2 { STEREO_ON } else { STEREO_OFF }, POS_STEREO.0, POS_STEREO.1);
        img.blit(&s.monoster, if channels == 1 { MONO_ON } else { MONO_OFF }, POS_MONO.0, POS_MONO.1);

        // volume
        let v = self.cfg.volume;
        let frame = (v * 27.0).round() as i32;
        img.blit(&s.volume, r(0, frame * 15, 68, 13), POS_VOLUME.x, POS_VOLUME.y);
        let tx = POS_VOLUME.x + (v * (POS_VOLUME.w - 14) as f32).round() as i32;
        let vol_down = self.drag == Drag::Slider(Hit::Volume);
        img.blit(&s.volume, if vol_down { VOL_THUMB_DOWN } else { VOL_THUMB }, tx, POS_VOLUME.y + 1);

        // balance
        let b = self.cfg.balance;
        let frame = (b.abs() * 27.0).round() as i32;
        img.blit(&s.balance, r(9, frame * 15, 38, 13), POS_BALANCE.x, POS_BALANCE.y);
        let tx = POS_BALANCE.x + ((b + 1.0) / 2.0 * (POS_BALANCE.w - 14) as f32).round() as i32;
        let bal_down = self.drag == Drag::Slider(Hit::Balance);
        img.blit(&s.balance, if bal_down { VOL_THUMB_DOWN } else { VOL_THUMB }, tx, POS_BALANCE.y + 1);

        // EQ / PL toggles
        let tog = |on: bool, down: bool, base_x: i32| -> Rect {
            let x = if down { base_x + 46 } else { base_x };
            r(x, if on { 73 } else { 61 }, 23, 12)
        };
        img.blit(&s.shufrep, tog(self.cfg.eq_visible, self.down(Hit::EqBtn), 0), POS_EQ_BUTTON.x, POS_EQ_BUTTON.y);
        img.blit(&s.shufrep, tog(self.cfg.pl_visible, self.down(Hit::PlBtn), 23), POS_PL_BUTTON.x, POS_PL_BUTTON.y);

        // position bar
        img.blit(&s.posbar, POSBAR_BG, POS_POSBAR.x, POS_POSBAR.y);
        if let Some(f) = self.pos_fraction() {
            let x = POS_POSBAR.x + (f * (POS_POSBAR.w - 29) as f64).round() as i32;
            let down = self.drag == Drag::Slider(Hit::Pos);
            img.blit(&s.posbar, if down { POSBAR_THUMB_DOWN } else { POSBAR_THUMB }, x, POS_POSBAR.y);
        }

        // transport buttons
        for (i, &(sx, w, h, x, y)) in CBUTTONS.iter().enumerate() {
            let down = self.down(Hit::CButton(i as u8));
            img.blit(&s.cbuttons, r(sx, if down { h } else { 0 }, w, h), x, y);
        }

        // shuffle / repeat
        let sr = |on: bool, down: bool| match (on, down) {
            (false, false) => 0,
            (false, true) => 15,
            (true, false) => 30,
            (true, true) => 45,
        };
        let y = sr(self.cfg.shuffle, self.down(Hit::Shuffle));
        img.blit(&s.shufrep, r(28, y, 47, 15), POS_SHUFFLE.x, POS_SHUFFLE.y);
        let y = sr(self.cfg.repeat, self.down(Hit::Repeat));
        img.blit(&s.shufrep, r(0, y, 28, 15), POS_REPEAT.x, POS_REPEAT.y);
    }

    fn draw_title_buttons(&self, img: &mut Image, shade: bool) {
        let s = &self.skin;
        img.blit(&s.titlebar, if self.down(Hit::MOptions) { MAIN_OPTIONS_DOWN } else { MAIN_OPTIONS }, POS_OPTIONS.x, POS_OPTIONS.y);
        img.blit(&s.titlebar, if self.down(Hit::MMinimize) { MAIN_MINIMIZE_DOWN } else { MAIN_MINIMIZE }, POS_MINIMIZE.x, POS_MINIMIZE.y);
        let shade_spr = match (shade, self.down(Hit::MShade)) {
            (false, false) => MAIN_SHADE,
            (false, true) => MAIN_SHADE_DOWN,
            (true, false) => MAIN_UNSHADE,
            (true, true) => MAIN_UNSHADE_DOWN,
        };
        img.blit(&s.titlebar, shade_spr, POS_SHADE.x, POS_SHADE.y);
        img.blit(&s.titlebar, if self.down(Hit::MClose) { MAIN_CLOSE_DOWN } else { MAIN_CLOSE }, POS_CLOSE.x, POS_CLOSE.y);
    }

    // ------------------------------------------------------------------ equalizer

    fn draw_eq(&self, img: &mut Image) {
        let s = &self.skin;
        let active = self.is_active(Win::Eq);
        if self.eq_shaded() {
            let ex = s.eq_ex.as_ref().unwrap();
            img.blit(ex, if active { EQS_BG_SELECTED } else { EQS_BG }, 0, 0);
            if self.down(Hit::EShade) {
                img.blit(ex, EQS_UNSHADE_DOWN, 254, 3);
            }
            if self.down(Hit::EClose) {
                img.blit(ex, EQS_CLOSE_DOWN, 264, 3);
            }
            let pick = |f: f32, l: Rect, c: Rect, rr: Rect| if f < 0.33 { l } else if f > 0.66 { rr } else { c };
            let v = self.cfg.volume;
            let x = POS_EQS_VOL.x + (v * (POS_EQS_VOL.w - 3) as f32).round() as i32;
            img.blit(ex, pick(v, EQS_VOL_THUMB_LEFT, EQS_VOL_THUMB_CENTER, EQS_VOL_THUMB_RIGHT), x, POS_EQS_VOL.y);
            let b = (self.cfg.balance + 1.0) / 2.0;
            let x = POS_EQS_BAL.x + (b * (POS_EQS_BAL.w - 3) as f32).round() as i32;
            img.blit(ex, pick(b, EQS_BAL_THUMB_LEFT, EQS_BAL_THUMB_CENTER, EQS_BAL_THUMB_RIGHT), x, POS_EQS_BAL.y);
            return;
        }
        img.blit(&s.eqmain, r(0, 0, 275, 116), 0, 0);
        img.blit(&s.eqmain, if active { EQ_TITLE_SELECTED } else { EQ_TITLE }, 0, 0);
        if self.down(Hit::EClose) {
            img.blit(&s.eqmain, EQ_CLOSE_DOWN, POS_EQ_CLOSE.x, POS_EQ_CLOSE.y);
        }
        if let (true, Some(ex)) = (self.down(Hit::EShade), s.eq_ex.as_ref()) {
            img.blit(ex, EQS_SHADE_DOWN, POS_EQ_SHADE.x, POS_EQ_SHADE.y);
        }
        let on = self.eq.on;
        let spr = match (on, self.down(Hit::EOn)) {
            (false, false) => EQ_ON,
            (false, true) => EQ_ON_DOWN,
            (true, false) => EQ_ON_SEL,
            (true, true) => EQ_ON_SEL_DOWN,
        };
        img.blit(&s.eqmain, spr, POS_EQ_ON.x, POS_EQ_ON.y);
        let spr = match (self.cfg.eq_auto, self.down(Hit::EAuto)) {
            (false, false) => EQ_AUTO,
            (false, true) => EQ_AUTO_DOWN,
            (true, false) => EQ_AUTO_SEL,
            (true, true) => EQ_AUTO_SEL_DOWN,
        };
        img.blit(&s.eqmain, spr, POS_EQ_AUTO.x, POS_EQ_AUTO.y);
        img.blit(&s.eqmain, if self.down(Hit::EPresets) { EQ_PRESETS_DOWN } else { EQ_PRESETS }, POS_EQ_PRESETS.x, POS_EQ_PRESETS.y);

        // response graph
        let g = POS_EQ_GRAPH;
        img.blit(&s.eqmain, EQ_GRAPH_BG, g.x, g.y);
        let db_to_y = |db: f32| ((1.0 - (db + 12.0) / 24.0) * 18.0).round().clamp(0.0, 18.0) as i32;
        img.blit(&s.eqmain, EQ_PREAMP_LINE, g.x, g.y + db_to_y(self.eq.preamp));
        let pts: Vec<f32> = self.eq.bands.to_vec();
        let mut prev: Option<i32> = None;
        for x in 0..g.w {
            // Catmull-Rom through the ten band values
            let t = x as f32 / (g.w - 1) as f32 * 9.0;
            let i = (t.floor() as usize).min(8);
            let f = t - i as f32;
            let p = |k: isize| pts[(i as isize + k).clamp(0, 9) as usize];
            let (p0, p1, p2, p3) = (p(-1), p(0), p(1), p(2));
            let v = 0.5
                * ((2.0 * p1)
                    + (-p0 + p2) * f
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * f * f
                    + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * f * f * f);
            let y = db_to_y(v);
            let (a, b) = match prev {
                Some(py) => (py.min(y), py.max(y)),
                None => (y, y),
            };
            for yy in a..=b {
                let c = s.eqmain.get(EQ_GRAPH_COLORS.x, EQ_GRAPH_COLORS.y + yy);
                img.set(g.x + x, g.y + yy, c);
            }
            prev = Some(y);
        }

        // sliders
        for (i, &x) in EQ_SLIDER_X.iter().enumerate() {
            let db = if i == 0 { self.eq.preamp } else { self.eq.bands[i - 1] };
            let f = (db + 12.0) / 24.0;
            let n = (f * 27.0).round() as i32;
            let sx = EQ_SLIDER_BG.x + (n % 14) * 15;
            let sy = EQ_SLIDER_BG.y + (n / 14) * 65;
            img.blit(&s.eqmain, r(sx, sy, 14, 63), x, EQ_SLIDER_Y);
            let ty = EQ_SLIDER_Y + ((1.0 - f) * (EQ_SLIDER_H - 12) as f32).round() as i32;
            let down = self.drag == Drag::Slider(Hit::ESlider(i as u8));
            img.blit(&s.eqmain, if down { EQ_THUMB_DOWN } else { EQ_THUMB }, x + 1, ty);
        }
    }

    // ------------------------------------------------------------------ playlist

    fn draw_playlist(&mut self, img: &mut Image) {
        let w = img.w;
        let h = img.h;
        let active = self.is_active(Win::Pl);
        if self.cfg.pl_shade {
            self.draw_playlist_shade(img, active);
            return;
        }
        let s = &self.skin;
        let sheet = &s.pledit;

        // top
        let (tl, title, tile, tr) = if active {
            (PL_TOP_LEFT_SEL, PL_TITLE_SEL, PL_TOP_TILE_SEL, PL_TOP_RIGHT_SEL)
        } else {
            (PL_TOP_LEFT, PL_TITLE, PL_TOP_TILE, PL_TOP_RIGHT)
        };
        img.tile(sheet, tile, r(25, 0, w - 50, 20));
        img.blit(sheet, tl, 0, 0);
        img.blit(sheet, title, (w - 100) / 2, 0);
        img.blit(sheet, tr, w - 25, 0);
        if self.down(Hit::PClose) {
            img.blit(sheet, PL_CLOSE_DOWN, w - 11, 3);
        }
        if self.down(Hit::PShade) {
            img.blit(sheet, PL_SHADE_DOWN, w - 20, 3);
        }

        // sides
        img.tile(sheet, PL_LEFT_TILE, r(0, 20, 12, h - 58));
        img.tile(sheet, PL_RIGHT_TILE, r(w - 20, 20, 20, h - 58));

        // bottom
        let by = h - 38;
        img.tile(sheet, PL_BOTTOM_TILE, r(125, by, w - 275, 38));
        img.blit(sheet, PL_BOTTOM_LEFT, 0, by);
        if w >= 350 {
            img.blit(sheet, PL_VIS_BG, w - 225, by);
        }
        img.blit(sheet, PL_BOTTOM_RIGHT, w - 150, by);

        // list
        let list = self.pl_list_rect(w, h);
        img.fill(list, s.pl.normal_bg);
        let rows = (list.h / ROW_H).max(1) as usize;
        let first = self.pl.scroll;
        let (normal, current, sel_bg) = (s.pl.normal, s.pl.current, s.pl.selected_bg);
        for row in 0..=rows {
            let i = first + row;
            if i >= self.pl.len() {
                break;
            }
            let y = list.y + row as i32 * ROW_H;
            let e = &self.pl.entries[i];
            let row_rc = r(list.x, y, list.w, ROW_H);
            let clip = r(list.x, y.max(list.y), list.w, (y + ROW_H).min(list.y + list.h) - y.max(list.y));
            if clip.h <= 0 {
                break;
            }
            if e.selected {
                img.fill(r(row_rc.x, clip.y, row_rc.w, clip.h), sel_bg);
            }
            let color = if self.pl.current == Some(i) { current } else { normal };
            let label = format!("{}. {}", i + 1, e.title);
            let dur = e.duration.map(fmt_time).unwrap_or_default();
            match self.ttf.as_mut() {
                Some(f) => {
                    let row = PlRowText { label, dur, left: list.x + 2, right: list.x + list.w - 3, baseline: y + 10, color, clip, hole: None };
                    if self.text_scale == 1.0 {
                        row.draw(f, img, 1.0);
                    } else {
                        // drawn later at screen resolution (see draw_zoomed_text)
                        self.pl_text.push(row);
                    }
                }
                None => {
                    // no TrueType font: fall back to the skin's bitmap font
                    let dw = dur.chars().count() as i32 * 5;
                    let dx = list.x + list.w - dw - 3;
                    draw_text_colored(img, s, list.x + 2, y + 4, &label.to_uppercase(), r(clip.x, clip.y, (dx - 4 - clip.x).max(0), clip.h), color);
                    draw_text_colored(img, s, dx, y + 4, &dur, clip, color);
                }
            }
        }

        // scrollbar
        let (hy, _) = self.pl_scroll_handle();
        let down = matches!(self.drag, Drag::PlScroll { .. });
        img.blit(sheet, if down { PL_SCROLL_HANDLE_DOWN } else { PL_SCROLL_HANDLE }, w - 15, hy);

        // running time: selected/total
        let (sel, sel_ok) = self.pl.selected_duration();
        let (tot, tot_ok) = self.pl.total_duration();
        let rt = format!(
            "{}{}/{}{}",
            fmt_time(sel),
            if sel_ok { "" } else { "+" },
            fmt_time(tot),
            if tot_ok { "" } else { "+" }
        );
        let bx = w - 150;
        draw_text(img, s, bx + 7, by + 10, &rt, r(bx + 7, by + 10, 90, 6));

        // mini time
        let mt = match self.time_parts() {
            Some((neg, m, sec)) if !self.blink_hidden() => format!("{}{}:{:02}", if neg { "-" } else { " " }, m, sec),
            Some(_) => String::new(),
            None => "  :  ".into(),
        };
        let mt = if mt.chars().count() < 5 && !mt.is_empty() { format!("{mt:>5}") } else { mt };
        draw_text(img, s, bx + 66, by + 23, &mt, r(bx + 66, by + 23, 30, 6));

        // popup menu
        if let Some(m) = self.menu_open {
            let menu = &PL_MENUS[m as usize];
            let bxm = if menu.from_right { w - menu.button_x } else { menu.button_x };
            let n = menu.items.len() as i32;
            let top = h - 30 - (n - 1) * PL_MENU_ITEM_H;
            img.blit(sheet, menu.bar, bxm - 3, top);
            let menu_rc = r(bxm - 3, top, 25, n * PL_MENU_ITEM_H);
            for t in &mut self.pl_text {
                t.hole = Some(menu_rc);
            }
            for (k, &(nx, sx, sy)) in menu.items.iter().enumerate() {
                let hover = self.menu_hover == Some(k as u8);
                img.blit(sheet, r(if hover { sx } else { nx }, sy, 22, 18), bxm, top + k as i32 * PL_MENU_ITEM_H);
            }
        }
    }

    fn draw_playlist_shade(&mut self, img: &mut Image, active: bool) {
        let w = img.w;
        let s = &self.skin;
        img.tile(&s.pledit, PL_SHADE_BG, r(25, 0, w - 75, 14));
        img.blit(&s.pledit, PL_SHADE_LEFT, 0, 0);
        img.blit(&s.pledit, if active { PL_SHADE_RIGHT_SEL } else { PL_SHADE_RIGHT }, w - 50, 0);
        if self.down(Hit::PShade) {
            img.blit(&s.pledit, PL_UNSHADE_DOWN, w - 20, 3);
        }
        if self.down(Hit::PClose) {
            img.blit(&s.pledit, PL_CLOSE_DOWN, w - 11, 3);
        }
        let (text, _) = match self.pl.current.and_then(|c| self.pl.entries.get(c).map(|e| (c, e))) {
            Some((c, e)) => (format!("{}. {}", c + 1, e.title), e.duration),
            None => (String::new(), None),
        };
        let dur = self.pl.current.and_then(|c| self.pl.entries.get(c)).and_then(|e| e.duration).map(fmt_time).unwrap_or_default();
        let dw = dur.chars().count() as i32 * 5;
        let dx = w - 30 - dw;
        draw_text(img, s, 5, 4, &text, r(5, 4, dx - 8, 6));
        draw_text(img, s, dx, 4, &dur, r(dx, 4, dw, 6));
    }
}

/// One playlist row of TrueType text, kept as a description so it can be drawn either into
/// the 1:1 frame or straight onto the zoomed frame at full screen resolution.
pub struct PlRowText {
    label: String,
    dur: String,
    /// left edge of the title, right edge of the duration, and the baseline
    left: i32,
    right: i32,
    baseline: i32,
    color: u32,
    clip: Rect,
    /// area covered by something drawn on top (an open playlist menu)
    hole: Option<Rect>,
}

impl PlRowText {
    fn offset(&mut self, dx: i32, dy: i32) {
        self.left += dx;
        self.right += dx;
        self.baseline += dy;
        self.clip = self.clip.offset(dx, dy);
        self.hole = self.hole.map(|h| h.offset(dx, dy));
    }

    /// Draw at `f` screen pixels per skin pixel; `font` must already be sized for `f`.
    fn draw(&self, font: &mut TtfFont, img: &mut Image, f: f64) {
        let sc = |v: i32| (v as f64 * f).round() as i32;
        let rect = |rc: Rect| r(sc(rc.x), sc(rc.y), sc(rc.x + rc.w) - sc(rc.x), sc(rc.y + rc.h) - sc(rc.y));
        let clip = rect(self.clip);
        let hole = self.hole.map(rect);
        let dx = sc(self.right) - font.width(&self.dur);
        let title_clip = r(clip.x, clip.y, (dx - sc(4) - clip.x).max(0), clip.h);
        font.draw(img, sc(self.left), sc(self.baseline), &self.label, self.color, title_clip, hole);
        font.draw(img, dx, sc(self.baseline), &self.dur, self.color, clip, hole);
    }
}

/// Bitmap text recolored to `color` (used when no TrueType font is available).
fn draw_text_colored(dst: &mut Image, skin: &crate::skin::Skin, x: i32, y: i32, s: &str, clip: Rect, color: u32) {
    let mut tmp = Image::new(clip.w.max(1), clip.h.max(1), 0);
    let local = r(0, 0, tmp.w, tmp.h);
    draw_text(&mut tmp, skin, x - clip.x, y - clip.y, s, local);
    let bg = skin.text.get(150, 4);
    for j in 0..tmp.h {
        for i in 0..tmp.w {
            let p = tmp.get(i, j);
            if p != bg && p != 0 {
                dst.set(clip.x + i, clip.y + j, color);
            }
        }
    }
}
