//! Player state and input handling for the main, equalizer and playlist windows.
//!
//! All three windows are stacked in one OS window (the classic docked layout), so the
//! app works the same on X11, Wayland, Windows and macOS.

use crate::audio::Audio;
use crate::config::Config;
use crate::eq::{self, Equalizer};
use crate::font::TtfFont;
use crate::gfx::{r, Rect};
use crate::media::{self, TrackInfo};
use crate::playlist::Playlist;
use crate::skin::Skin;
use crate::sprites::*;
use crate::vis::Analyzer;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

pub const ROW_H: i32 = 13;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Win {
    Main,
    Eq,
    Pl,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Hit {
    MTitle,
    MOptions,
    MMinimize,
    MShade,
    MClose,
    Clutter(u8),
    CButton(u8),
    Shuffle,
    Repeat,
    Logo,
    EqBtn,
    PlBtn,
    Pos,
    Volume,
    Balance,
    Time,
    Vis,
    ETitle,
    EClose,
    EShade,
    EOn,
    EAuto,
    EPresets,
    ESlider(u8),
    ESVol,
    ESBal,
    PTitle,
    PClose,
    PShade,
    PList,
    PScroll,
    PResize,
    PMenuBtn(u8),
    PMenuItem(u8, u8),
    PMini(u8),
    PTime,
    Nothing,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Drag {
    None,
    Slider(Hit),
    PlScroll { grab: i32 },
    PlResize { mx: i32, my: i32, w: i32, h: i32 },
    PlReorder { row: isize },
    PlMenu,
}

pub enum WinReq {
    DragWindow,
    Minimize,
    OnTop(bool),
    Relayout,
    Quit,
}

pub enum DialogKind {
    OpenFiles,
    AddFiles,
    AddDir,
    LoadSkin,
    LoadPlaylist,
    SavePlaylist,
}

pub struct DialogResult {
    kind: DialogKind,
    paths: Vec<PathBuf>,
}

pub struct Layout {
    pub main: Rect,
    pub eq: Option<Rect>,
    pub pl: Option<Rect>,
    pub w: i32,
    pub h: i32,
}

pub struct App {
    pub skin: Skin,
    pub cfg: Config,
    pub audio: Audio,
    pub pl: Playlist,
    pub analyzer: Analyzer,
    pub eq: Equalizer,
    pub status: Status,
    pub current_info: Option<TrackInfo>,
    pub focused: bool,
    pub active: Win,
    pub mouse: (i32, i32),
    pub pressed: Option<Hit>,
    pub drag: Drag,
    pub slider_text: Option<String>,
    flash: Option<(String, Instant)>,
    pub marquee_offset: i32,
    marquee_tick: Instant,
    /// position-slider preview while dragging (0..1)
    pub seek_preview: Option<f64>,
    pub start: Instant,
    pub ttf: Option<TtfFont>,
    /// `ttf` re-rasterized at the current zoom, for crisp playlist text
    ttf_zoomed: Option<TtfFont>,
    /// screen pixels per skin pixel; when not 1, playlist text is drawn after scaling
    pub text_scale: f64,
    /// playlist rows waiting to be drawn at `text_scale` (filled by `render`)
    pub pl_text: Vec<crate::render::PlRowText>,
    pub menu_open: Option<u8>,
    pub menu_hover: Option<u8>,
    menu_sticky: bool,
    /// pointer left the opening button's spot since the menu was opened
    menu_moved: bool,
    last_click: Option<(Instant, Hit, i32)>,
    preset_idx: usize,
    vis_buf: Vec<f32>,
    pub requests: Vec<WinReq>,
    probe_tx: Sender<PathBuf>,
    probe_rx: Receiver<(PathBuf, TrackInfo)>,
    dialog_tx: Sender<DialogResult>,
    dialog_rx: Receiver<DialogResult>,
    pub dirty_config: bool,
}

impl App {
    pub fn new(cfg: Config, skin_override: Option<PathBuf>, files: Vec<PathBuf>) -> App {
        // explicit --skin, then the remembered skin, then any skin found next to the exe
        let mut cfg = cfg;
        let mut skin_err = None;
        let fallback = Skin::default_skin;
        let explicit = skin_override.or_else(|| cfg.skin.clone().filter(|p| p.exists()));
        let skin = match explicit {
            Some(p) => match Skin::load(&p) {
                Ok(s) => {
                    cfg.skin = Some(std::path::absolute(&p).unwrap_or(p));
                    s
                }
                Err(e) => {
                    eprintln!("rsAmp: could not load skin: {e}");
                    skin_err = Some(e);
                    fallback()
                }
            },
            None => match crate::skin::discover() {
                Some((p, s)) => {
                    cfg.skin = Some(std::path::absolute(&p).unwrap_or(p));
                    s
                }
                None => fallback(),
            },
        };
        let audio = Audio::new();
        let eq = Equalizer { on: cfg.eq_on, preamp: cfg.eq_preamp, bands: cfg.eq_bands };
        audio.set_volume(cfg.volume);
        audio.set_balance(cfg.balance);
        audio.set_eq(&eq);
        let ttf = TtfFont::load(&skin.pl.font, 10.0);

        // background metadata prober
        let (probe_tx, job_rx) = channel::<PathBuf>();
        let (res_tx, probe_rx) = channel();
        std::thread::Builder::new()
            .name("probe".into())
            .spawn(move || {
                while let Ok(p) = job_rx.recv() {
                    let info = media::probe(&p);
                    if res_tx.send((p, info)).is_err() {
                        break;
                    }
                }
            })
            .ok();
        let (dialog_tx, dialog_rx) = channel();

        let mut app = App {
            skin,
            cfg,
            audio,
            pl: Playlist::default(),
            analyzer: Analyzer::new(),
            eq,
            status: Status::Stopped,
            current_info: None,
            focused: true,
            active: Win::Main,
            mouse: (0, 0),
            pressed: None,
            drag: Drag::None,
            slider_text: None,
            flash: None,
            marquee_offset: 0,
            marquee_tick: Instant::now(),
            seek_preview: None,
            start: Instant::now(),
            ttf,
            ttf_zoomed: None,
            text_scale: 1.0,
            pl_text: Vec::new(),
            menu_open: None,
            menu_hover: None,
            menu_sticky: false,
            menu_moved: false,
            last_click: None,
            preset_idx: 0,
            vis_buf: Vec::new(),
            requests: Vec::new(),
            probe_tx,
            probe_rx,
            dialog_tx,
            dialog_rx,
            dirty_config: false,
        };
        // persist a discovered skin choice
        app.dirty_config = true;
        if let Some(e) = skin_err {
            app.flash(format!("SKIN ERROR: {}", e.to_uppercase()));
        } else if !app.skin.missing.is_empty() {
            app.flash(format!("SKIN: COULD NOT READ {}", app.skin.missing.join(", ").to_uppercase()));
        }
        if files.is_empty() {
            let saved = app.cfg.playlist.clone();
            app.add_paths(saved);
            if let Some(c) = app.cfg.current.filter(|&c| c < app.pl.len()) {
                app.pl.current = Some(c);
                app.pl.select_only(c);
                app.current_info = Some(media::probe(&app.pl.entries[c].path));
            }
        } else {
            app.add_paths(files);
            if app.pl.len() > 0 {
                app.play_index(0);
            }
        }
        app
    }

    // ---------------------------------------------------------------- layout

    pub fn layout(&self) -> Layout {
        let main_h = if self.cfg.main_shade { 14 } else { 116 };
        let mut y = main_h;
        let eq = if self.cfg.eq_visible {
            let h = if self.cfg.eq_shade && self.skin.eq_ex.is_some() { 14 } else { 116 };
            let rc = r(0, y, 275, h);
            y += h;
            Some(rc)
        } else {
            None
        };
        let pl = if self.cfg.pl_visible {
            let h = if self.cfg.pl_shade { 14 } else { self.cfg.pl_h };
            let rc = r(0, y, self.cfg.pl_w, h);
            y += h;
            Some(rc)
        } else {
            None
        };
        let w = pl.map(|p| p.w).unwrap_or(275).max(275);
        Layout { main: r(0, 0, 275, main_h), eq, pl, w, h: y }
    }

    pub fn eq_shaded(&self) -> bool {
        self.cfg.eq_shade && self.skin.eq_ex.is_some()
    }

    fn which(&self, x: i32, y: i32) -> Option<(Win, i32, i32, Rect)> {
        let l = self.layout();
        if l.main.contains(x, y) {
            return Some((Win::Main, x, y - l.main.y, l.main));
        }
        if let Some(e) = l.eq {
            if e.contains(x, y) {
                return Some((Win::Eq, x, y - e.y, e));
            }
        }
        if let Some(p) = l.pl {
            if p.contains(x, y) {
                return Some((Win::Pl, x, y - p.y, p));
            }
        }
        None
    }

    pub fn pl_list_rect(&self, w: i32, h: i32) -> Rect {
        r(12, 20, w - 32, h - 58)
    }

    pub fn pl_visible_rows(&self) -> usize {
        let l = self.layout();
        let Some(p) = l.pl else { return 0 };
        (self.pl_list_rect(p.w, p.h).h / ROW_H).max(1) as usize
    }

    pub fn pl_max_scroll(&self) -> usize {
        self.pl.len().saturating_sub(self.pl_visible_rows())
    }

    fn menu_rect(&self, idx: usize, pw: i32, ph: i32) -> (Rect, i32, i32) {
        let m = &PL_MENUS[idx];
        let bx = if m.from_right { pw - m.button_x } else { m.button_x };
        let n = m.items.len() as i32;
        let top = ph - 30 - (n - 1) * PL_MENU_ITEM_H;
        (r(bx, top, PL_MENU_ITEM_W, n * PL_MENU_ITEM_H), bx, top)
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Hit {
        let Some((win, lx, ly, rc)) = self.which(x, y) else { return Hit::Nothing };
        match win {
            Win::Main => self.hit_main(lx, ly),
            Win::Eq => self.hit_eq(lx, ly),
            Win::Pl => self.hit_pl(lx, ly, rc.w, rc.h),
        }
    }

    fn hit_main(&self, x: i32, y: i32) -> Hit {
        if POS_OPTIONS.contains(x, y) {
            return Hit::MOptions;
        }
        if POS_MINIMIZE.contains(x, y) {
            return Hit::MMinimize;
        }
        if POS_SHADE.contains(x, y) {
            return Hit::MShade;
        }
        if POS_CLOSE.contains(x, y) {
            return Hit::MClose;
        }
        if self.cfg.main_shade {
            for (i, b) in SHADE_BUTTONS.iter().enumerate() {
                if b.contains(x, y) {
                    return Hit::CButton(i as u8);
                }
            }
            if SHADE_POS.contains(x, y) {
                return Hit::Pos;
            }
            if r(SHADE_TIME.0, 3, 32, 8).contains(x, y) {
                return Hit::Time;
            }
            if SHADE_VIS.contains(x, y) {
                return Hit::Vis;
            }
            return Hit::MTitle;
        }
        if y < 14 {
            return Hit::MTitle;
        }
        if POS_CLUTTER.contains(x, y) {
            let oy = y - POS_CLUTTER.y;
            for (i, (by, h, _)) in CLUTTER_BUTTONS.iter().enumerate() {
                if oy >= *by && oy < by + h {
                    return Hit::Clutter(i as u8);
                }
            }
            return Hit::Nothing;
        }
        for (i, &(_, w, h, bx, by)) in CBUTTONS.iter().enumerate() {
            if r(bx, by, w, h).contains(x, y) {
                return Hit::CButton(i as u8);
            }
        }
        if POS_SHUFFLE.contains(x, y) {
            return Hit::Shuffle;
        }
        if POS_LOGO.contains(x, y) {
            return Hit::Logo;
        }
        if POS_REPEAT.contains(x, y) {
            return Hit::Repeat;
        }
        if POS_EQ_BUTTON.contains(x, y) {
            return Hit::EqBtn;
        }
        if POS_PL_BUTTON.contains(x, y) {
            return Hit::PlBtn;
        }
        if POS_POSBAR.contains(x, y) && self.status != Status::Stopped {
            return Hit::Pos;
        }
        if POS_VOLUME.contains(x, y) {
            return Hit::Volume;
        }
        if POS_BALANCE.contains(x, y) {
            return Hit::Balance;
        }
        if POS_TIME.contains(x, y) {
            return Hit::Time;
        }
        if POS_VIS.contains(x, y) {
            return Hit::Vis;
        }
        // The main window can be dragged from anywhere that isn't a control.
        Hit::MTitle
    }

    fn hit_eq(&self, x: i32, y: i32) -> Hit {
        if self.eq_shaded() {
            if r(254, 3, 9, 9).contains(x, y) {
                return Hit::EShade;
            }
            if r(264, 3, 9, 9).contains(x, y) {
                return Hit::EClose;
            }
            if POS_EQS_VOL.contains(x, y) {
                return Hit::ESVol;
            }
            if POS_EQS_BAL.contains(x, y) {
                return Hit::ESBal;
            }
            return Hit::ETitle;
        }
        if POS_EQ_CLOSE.contains(x, y) {
            return Hit::EClose;
        }
        if POS_EQ_SHADE.contains(x, y) && self.skin.eq_ex.is_some() {
            return Hit::EShade;
        }
        if y < 14 {
            return Hit::ETitle;
        }
        if POS_EQ_ON.contains(x, y) {
            return Hit::EOn;
        }
        if POS_EQ_AUTO.contains(x, y) {
            return Hit::EAuto;
        }
        if POS_EQ_PRESETS.contains(x, y) {
            return Hit::EPresets;
        }
        for (i, &sx) in EQ_SLIDER_X.iter().enumerate() {
            if r(sx, EQ_SLIDER_Y, EQ_SLIDER_W, EQ_SLIDER_H).contains(x, y) {
                return Hit::ESlider(i as u8);
            }
        }
        Hit::ETitle
    }

    fn hit_pl(&self, x: i32, y: i32, w: i32, h: i32) -> Hit {
        if self.cfg.pl_shade {
            if r(w - 20, 3, 9, 9).contains(x, y) {
                return Hit::PShade;
            }
            if r(w - 11, 3, 9, 9).contains(x, y) {
                return Hit::PClose;
            }
            if x >= w - 20 && x < w - 2 && y >= 0 {
                // the bottom-right of the shaded bar can still resize horizontally
                return Hit::PResize;
            }
            return Hit::PTitle;
        }
        if let Some(m) = self.menu_open {
            let (rc, _, top) = self.menu_rect(m as usize, w, h);
            if rc.contains(x, y) {
                return Hit::PMenuItem(m, ((y - top) / PL_MENU_ITEM_H) as u8);
            }
        }
        if y < 20 {
            if r(w - 11, 3, 9, 9).contains(x, y) {
                return Hit::PClose;
            }
            if r(w - 20, 3, 9, 9).contains(x, y) {
                return Hit::PShade;
            }
            return Hit::PTitle;
        }
        let list = self.pl_list_rect(w, h);
        if list.contains(x, y) {
            return Hit::PList;
        }
        if x >= w - 15 && x < w - 7 && y >= 20 && y < h - 38 {
            return Hit::PScroll;
        }
        if y >= h - 38 {
            if r(w - 20, h - 20, 20, 20).contains(x, y) {
                return Hit::PResize;
            }
            for (i, m) in PL_MENUS.iter().enumerate() {
                let bx = if m.from_right { w - m.button_x } else { m.button_x };
                if r(bx, h - 30, 22, 18).contains(x, y) {
                    return Hit::PMenuBtn(i as u8);
                }
            }
            let bx = w - 150;
            for (i, &(ox, bw)) in crate::default_skin::PL_MINI_BUTTONS.iter().enumerate() {
                if r(bx + ox, h - 16, bw, 8).contains(x, y) {
                    return Hit::PMini(i as u8);
                }
            }
            if r(bx + 62, h - 17, 36, 9).contains(x, y) {
                return Hit::PTime;
            }
        }
        Hit::PTitle
    }

    // ---------------------------------------------------------------- playback

    pub fn duration(&self) -> Option<f64> {
        self.current_info
            .as_ref()
            .and_then(|i| i.duration)
            .or_else(|| self.audio.info().and_then(|i| i.duration))
            .or_else(|| self.pl.current.and_then(|c| self.pl.entries.get(c)).and_then(|e| e.duration))
    }

    pub fn position(&self) -> f64 {
        if self.status == Status::Stopped {
            0.0
        } else {
            self.audio.position()
        }
    }

    pub fn play_index(&mut self, i: usize) {
        if i >= self.pl.len() {
            return;
        }
        let record = self.pl.current != Some(i);
        self.pl.set_current(i, record);
        let path = self.pl.entries[i].path.clone();
        let info = media::probe(&path);
        let e = &mut self.pl.entries[i];
        if info.duration.is_some() {
            e.duration = info.duration;
        }
        if !e.probed {
            if info.title != media::file_title(&path) {
                e.title = info.title.clone();
            }
            e.probed = true;
        }
        self.current_info = Some(info);
        self.audio.play(path, 0.0);
        self.status = Status::Playing;
        self.marquee_offset = 0;
        self.ensure_visible(i);
        self.dirty_config = true;
    }

    pub fn play(&mut self) {
        match self.status {
            Status::Paused => {
                // the classic play button restarts the track when paused
                if let Some(c) = self.pl.current {
                    self.play_index(c);
                }
            }
            _ => {
                if let Some(c) = self.pl.current.or(if self.pl.len() > 0 { Some(0) } else { None }) {
                    self.play_index(c);
                } else {
                    self.open_dialog(DialogKind::OpenFiles);
                }
            }
        }
    }

    pub fn pause(&mut self) {
        match self.status {
            Status::Playing => {
                self.audio.set_paused(true);
                self.status = Status::Paused;
            }
            Status::Paused => {
                self.audio.set_paused(false);
                self.status = Status::Playing;
            }
            Status::Stopped => {}
        }
    }

    pub fn stop(&mut self) {
        self.audio.stop();
        self.status = Status::Stopped;
    }

    pub fn next(&mut self) {
        if let Some(i) = self.pl.next_index(self.cfg.shuffle, true) {
            if self.status == Status::Stopped {
                self.pl.set_current(i, true);
                self.current_info = Some(media::probe(&self.pl.entries[i].path));
                self.marquee_offset = 0;
                self.ensure_visible(i);
            } else {
                self.play_index(i);
            }
        }
    }

    pub fn prev(&mut self) {
        if let Some(i) = self.pl.prev_index(self.cfg.shuffle) {
            if self.status == Status::Stopped {
                self.pl.current = Some(i);
                self.current_info = Some(media::probe(&self.pl.entries[i].path));
                self.marquee_offset = 0;
                self.ensure_visible(i);
            } else {
                self.play_index(i);
            }
        }
    }

    pub fn seek_to(&mut self, secs: f64) {
        if self.status == Status::Stopped {
            return;
        }
        let d = self.duration().unwrap_or(f64::MAX);
        self.audio.seek(secs.clamp(0.0, (d - 0.5).max(0.0)));
    }

    pub fn set_volume(&mut self, v: f32) {
        self.cfg.volume = v.clamp(0.0, 1.0);
        self.audio.set_volume(self.cfg.volume);
        self.dirty_config = true;
    }

    pub fn set_balance(&mut self, b: f32) {
        let b = if b.abs() < 0.08 { 0.0 } else { b.clamp(-1.0, 1.0) };
        self.cfg.balance = b;
        self.audio.set_balance(b);
        self.dirty_config = true;
    }

    fn apply_eq(&mut self) {
        self.audio.set_eq(&self.eq);
        self.cfg.eq_on = self.eq.on;
        self.cfg.eq_preamp = self.eq.preamp;
        self.cfg.eq_bands = self.eq.bands;
        self.dirty_config = true;
    }

    pub fn ensure_visible(&mut self, i: usize) {
        let rows = self.pl_visible_rows();
        if i < self.pl.scroll {
            self.pl.scroll = i;
        } else if i >= self.pl.scroll + rows {
            self.pl.scroll = i + 1 - rows;
        }
        self.pl.scroll = self.pl.scroll.min(self.pl_max_scroll());
    }

    pub fn add_paths(&mut self, paths: Vec<PathBuf>) -> usize {
        let before = self.pl.len();
        let n = self.pl.add(paths);
        for e in &self.pl.entries[before..] {
            let _ = self.probe_tx.send(e.path.clone());
        }
        self.dirty_config = true;
        n
    }

    pub fn flash(&mut self, s: impl Into<String>) {
        self.flash = Some((s.into(), Instant::now()));
    }

    pub fn marquee_text(&self) -> (String, bool) {
        if let Some(t) = &self.slider_text {
            return (t.clone(), false);
        }
        if let Some((t, at)) = &self.flash {
            if at.elapsed() < Duration::from_millis(1800) {
                return (t.clone(), false);
            }
        }
        match self.pl.current.and_then(|c| self.pl.entries.get(c).map(|e| (c, e))) {
            Some((c, e)) => {
                let dur = e.duration.or(self.duration());
                let t = match dur {
                    Some(d) => format!("{}. {} ({})", c + 1, e.title, media::fmt_time(d)),
                    None => format!("{}. {}", c + 1, e.title),
                };
                (t, true)
            }
            None => ("RSAMP 2.91".to_string(), false),
        }
    }

    // ---------------------------------------------------------------- per-frame

    pub fn tick(&mut self) {
        while let Ok((p, info)) = self.probe_rx.try_recv() {
            // a title from the file's tags wins; otherwise keep what the playlist file said
            let tagged = info.title != media::file_title(&p);
            for e in self.pl.entries.iter_mut().filter(|e| e.path == p && !e.probed) {
                if tagged {
                    e.title = info.title.clone();
                }
                if info.duration.is_some() {
                    e.duration = info.duration;
                }
                e.probed = true;
            }
        }
        while let Ok(res) = self.dialog_rx.try_recv() {
            self.handle_dialog(res);
        }
        if let Some(err) = self.audio.take_error() {
            self.flash(format!("ERROR: {}", err.to_uppercase()));
        }
        if self.audio.take_finished() && self.status == Status::Playing {
            match self.pl.next_index(self.cfg.shuffle, self.cfg.repeat) {
                Some(i) => self.play_index(i),
                None => self.stop(),
            }
        }
        // a track that never produced audio (e.g. undecodable) also ends up inactive
        if self.status != Status::Stopped && !self.audio.is_active() && self.pressed.is_none() {
            self.status = Status::Stopped;
        }
        self.audio.vis_samples(&mut self.vis_buf);
        let active = self.status == Status::Playing;
        self.analyzer.update(&self.vis_buf, self.audio.output_rate(), active);

        if self.marquee_tick.elapsed() >= Duration::from_millis(200) {
            self.marquee_tick = Instant::now();
            self.marquee_offset += 5;
        }
    }

    // ---------------------------------------------------------------- input

    pub fn mouse_down(&mut self, x: i32, y: i32, ctrl: bool, shift: bool) {
        self.mouse = (x, y);
        if let Some((w, ..)) = self.which(x, y) {
            self.active = w;
        }
        let hit = self.hit_test(x, y);
        let now = Instant::now();
        let double = matches!(self.last_click, Some((t, h, row)) if h == hit
            && now.duration_since(t) < Duration::from_millis(400)
            && (hit != Hit::PList || row == self.pl_row_at(y)));
        self.last_click = Some((now, hit, if hit == Hit::PList { self.pl_row_at(y) } else { -1 }));

        if self.menu_open.is_some() && !matches!(hit, Hit::PMenuItem(..)) {
            self.menu_open = None;
            self.menu_hover = None;
            if matches!(hit, Hit::PMenuBtn(_)) {
                return;
            }
        }

        match hit {
            Hit::MTitle | Hit::ETitle | Hit::PTitle => {
                if double {
                    match hit {
                        Hit::MTitle => self.toggle_main_shade(),
                        Hit::ETitle => self.toggle_eq_shade(),
                        _ => self.toggle_pl_shade(),
                    }
                    self.last_click = None;
                } else {
                    self.requests.push(WinReq::DragWindow);
                }
            }
            Hit::Time | Hit::PTime => {
                self.cfg.time_remaining = !self.cfg.time_remaining;
                self.dirty_config = true;
            }
            Hit::Vis => {
                self.cfg.vis_mode = self.cfg.vis_mode.next();
                self.dirty_config = true;
            }
            Hit::Pos | Hit::Volume | Hit::Balance | Hit::ESlider(_) | Hit::ESVol | Hit::ESBal => {
                self.pressed = Some(hit);
                self.drag = Drag::Slider(hit);
                self.slider_move(hit, x, y);
            }
            Hit::PList => self.pl_click(x, y, ctrl, shift, double),
            Hit::PScroll => {
                let (handle_y, _) = self.pl_scroll_handle();
                let ly = y - self.layout().pl.map(|p| p.y).unwrap_or(0);
                let grab = if ly >= handle_y && ly < handle_y + 18 { ly - handle_y } else { 9 };
                self.pressed = Some(hit);
                self.drag = Drag::PlScroll { grab };
                self.mouse_move(x, y);
            }
            Hit::PResize => {
                self.pressed = Some(hit);
                self.drag = Drag::PlResize { mx: x, my: y, w: self.cfg.pl_w, h: self.cfg.pl_h };
            }
            Hit::PMenuBtn(m) => {
                self.menu_open = Some(m);
                self.menu_hover = None;
                self.menu_sticky = false;
                self.menu_moved = false;
                self.drag = Drag::PlMenu;
                self.pressed = Some(hit);
            }
            Hit::PMenuItem(m, i) => {
                self.menu_hover = Some(i);
                self.drag = Drag::PlMenu;
                self.pressed = Some(Hit::PMenuItem(m, i));
            }
            Hit::Nothing => {}
            _ => self.pressed = Some(hit),
        }
    }

    pub fn mouse_move(&mut self, x: i32, y: i32) {
        self.mouse = (x, y);
        match self.drag {
            Drag::Slider(h) => self.slider_move(h, x, y),
            Drag::PlScroll { grab } => {
                let Some(p) = self.layout().pl else { return };
                let ly = y - p.y;
                let track = (p.h - 58 - 18).max(1);
                let frac = ((ly - grab - 20) as f32 / track as f32).clamp(0.0, 1.0);
                self.pl.scroll = (frac * self.pl_max_scroll() as f32).round() as usize;
            }
            Drag::PlResize { mx, my, w, h } => {
                let nw = (w + ((x - mx) as f32 / 25.0).round() as i32 * 25).max(275);
                let nh = if self.cfg.pl_shade { h } else { (h + ((y - my) as f32 / 29.0).round() as i32 * 29).max(116) };
                if nw != self.cfg.pl_w || nh != self.cfg.pl_h {
                    self.cfg.pl_w = nw;
                    self.cfg.pl_h = nh;
                    self.pl.scroll = self.pl.scroll.min(self.pl_max_scroll());
                    self.requests.push(WinReq::Relayout);
                    self.dirty_config = true;
                }
            }
            Drag::PlReorder { row } => {
                let r = self.pl_row_at(y) as isize;
                if r >= 0 && r != row && self.pl.move_selected(r - row) {
                    self.drag = Drag::PlReorder { row: r };
                } else if r < 0 {
                    // auto-scroll while dragging above/below the list
                    let Some(p) = self.layout().pl else { return };
                    let ly = y - p.y;
                    if ly < 20 && self.pl.scroll > 0 {
                        self.pl.scroll -= 1;
                    } else if ly >= p.h - 38 && self.pl.scroll < self.pl_max_scroll() {
                        self.pl.scroll += 1;
                    }
                }
            }
            Drag::PlMenu => {
                self.menu_hover = match self.hit_test(x, y) {
                    Hit::PMenuItem(_, i) => Some(i),
                    _ => None,
                };
                if let Some(m) = self.menu_open {
                    let last = PL_MENUS[m as usize].items.len() as u8 - 1;
                    if self.menu_hover != Some(last) {
                        self.menu_moved = true;
                    }
                }
            }
            Drag::None => {}
        }
    }

    pub fn mouse_up(&mut self, x: i32, y: i32) {
        self.mouse = (x, y);
        let hit = self.hit_test(x, y);
        let pressed = self.pressed.take();
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Slider(Hit::Pos) => {
                if let (Some(f), Some(d)) = (self.seek_preview.take(), self.duration()) {
                    self.seek_to(f * d);
                }
                self.slider_text = None;
                return;
            }
            Drag::Slider(_) => {
                self.slider_text = None;
                return;
            }
            Drag::PlMenu => {
                if matches!(pressed, Some(Hit::PMenuBtn(_))) && !self.menu_moved {
                    // click-and-release on the button keeps the menu open
                    self.menu_sticky = true;
                    self.menu_hover = None;
                    return;
                }
                match hit {
                    Hit::PMenuItem(m, i) => {
                        self.menu_open = None;
                        self.menu_hover = None;
                        self.menu_action(m, i);
                    }
                    _ => {
                        self.menu_open = None;
                        self.menu_hover = None;
                    }
                }
                return;
            }
            Drag::None => {}
            _ => return,
        }
        let Some(p) = pressed else { return };
        if p != hit {
            return;
        }
        self.click(p);
    }

    fn click(&mut self, h: Hit) {
        match h {
            Hit::MOptions | Hit::Logo => self.open_dialog(DialogKind::LoadSkin),
            Hit::MMinimize => self.requests.push(WinReq::Minimize),
            Hit::MShade => self.toggle_main_shade(),
            Hit::MClose => self.requests.push(WinReq::Quit),
            Hit::Clutter(i) => match i {
                0 => self.open_dialog(DialogKind::LoadSkin),
                1 => self.toggle_on_top(),
                2 => {
                    if let Some(info) = &self.current_info {
                        let msg = format!(
                            "{} {}KBPS {}KHZ {}",
                            info.title,
                            info.kbps.unwrap_or(0),
                            info.khz.unwrap_or(0),
                            if info.channels == Some(1) { "MONO" } else { "STEREO" }
                        );
                        self.flash(msg);
                    }
                }
                3 => self.toggle_double(),
                _ => {
                    self.cfg.vis_mode = self.cfg.vis_mode.next();
                    self.dirty_config = true;
                }
            },
            Hit::CButton(i) | Hit::PMini(i) => match i {
                0 => self.prev(),
                1 => self.play(),
                2 => self.pause(),
                3 => self.stop(),
                4 => self.next(),
                _ => self.open_dialog(DialogKind::OpenFiles),
            },
            Hit::Shuffle => {
                self.cfg.shuffle = !self.cfg.shuffle;
                self.dirty_config = true;
            }
            Hit::Repeat => {
                self.cfg.repeat = !self.cfg.repeat;
                self.dirty_config = true;
            }
            Hit::EqBtn | Hit::EClose => self.toggle_eq(),
            Hit::PlBtn | Hit::PClose => self.toggle_pl(),
            Hit::EShade => self.toggle_eq_shade(),
            Hit::PShade => self.toggle_pl_shade(),
            Hit::EOn => {
                self.eq.on = !self.eq.on;
                self.apply_eq();
            }
            Hit::EAuto => {
                self.cfg.eq_auto = !self.cfg.eq_auto;
                self.dirty_config = true;
            }
            Hit::EPresets => {
                self.preset_idx = (self.preset_idx + 1) % eq::PRESETS.len();
                let (name, bands) = eq::PRESETS[self.preset_idx];
                self.eq.bands = bands;
                self.apply_eq();
                self.flash(format!("EQ PRESET: {name}"));
            }
            _ => {}
        }
    }

    fn slider_move(&mut self, h: Hit, x: i32, y: i32) {
        let l = self.layout();
        match h {
            Hit::Pos => {
                let f = if self.cfg.main_shade {
                    (x - SHADE_POS.x - 1) as f64 / (SHADE_POS.w - 3) as f64
                } else {
                    (x - POS_POSBAR.x - 14) as f64 / (POS_POSBAR.w - 29) as f64
                }
                .clamp(0.0, 1.0);
                self.seek_preview = Some(f);
                if let Some(d) = self.duration() {
                    self.slider_text = Some(format!(
                        "SEEK TO: {}/{} ({}%)",
                        media::fmt_time(f * d),
                        media::fmt_time(d),
                        (f * 100.0).round()
                    ));
                }
            }
            Hit::Volume | Hit::ESVol => {
                let v = if h == Hit::Volume {
                    (x - POS_VOLUME.x - 7) as f32 / (POS_VOLUME.w - 14) as f32
                } else {
                    (x - POS_EQS_VOL.x - 1) as f32 / (POS_EQS_VOL.w - 3) as f32
                };
                self.set_volume(v);
                self.slider_text = Some(format!("VOLUME: {}%", (self.cfg.volume * 100.0).round()));
            }
            Hit::Balance | Hit::ESBal => {
                let f = if h == Hit::Balance {
                    (x - POS_BALANCE.x - 7) as f32 / (POS_BALANCE.w - 14) as f32
                } else {
                    (x - POS_EQS_BAL.x - 1) as f32 / (POS_EQS_BAL.w - 3) as f32
                };
                self.set_balance(f * 2.0 - 1.0);
                let b = self.cfg.balance;
                self.slider_text = Some(if b == 0.0 {
                    "BALANCE: CENTER".into()
                } else {
                    format!("BALANCE: {}% {}", (b.abs() * 100.0).round(), if b < 0.0 { "LEFT" } else { "RIGHT" })
                });
            }
            Hit::ESlider(i) => {
                let ly = y - l.eq.map(|e| e.y).unwrap_or(0);
                let f = 1.0 - (ly - EQ_SLIDER_Y - 5) as f32 / (EQ_SLIDER_H - 12) as f32;
                let mut db = (f.clamp(0.0, 1.0) * 2.0 - 1.0) * eq::MAX_DB;
                if db.abs() < 0.6 {
                    db = 0.0;
                }
                let txt = if i == 0 {
                    self.eq.preamp = db;
                    format!("PREAMP: {:+.1} DB", db)
                } else {
                    self.eq.bands[i as usize - 1] = db;
                    format!("EQ: {}: {:+.1} DB", eq::BAND_NAMES[i as usize - 1], db)
                };
                self.slider_text = Some(txt);
                self.apply_eq();
            }
            _ => {}
        }
    }

    pub fn wheel(&mut self, x: i32, y: i32, lines: f32) {
        match self.which(x, y).map(|w| w.0) {
            Some(Win::Pl) => {
                let s = self.pl.scroll as i64 - (lines * 3.0).round() as i64;
                self.pl.scroll = s.clamp(0, self.pl_max_scroll() as i64) as usize;
            }
            Some(_) => {
                let v = self.cfg.volume + lines * 0.03;
                self.set_volume(v);
                self.flash(format!("VOLUME: {}%", (self.cfg.volume * 100.0).round()));
            }
            None => {}
        }
    }

    fn pl_row_at(&self, y: i32) -> i32 {
        let Some(p) = self.layout().pl else { return -1 };
        let list = self.pl_list_rect(p.w, p.h);
        let ly = y - p.y;
        if ly < list.y || ly >= list.y + list.h {
            return -1;
        }
        let row = (ly - list.y) / ROW_H + self.pl.scroll as i32;
        if row >= 0 && (row as usize) < self.pl.len() {
            row
        } else {
            -1
        }
    }

    fn pl_click(&mut self, _x: i32, y: i32, ctrl: bool, shift: bool, double: bool) {
        let row = self.pl_row_at(y);
        if row < 0 {
            if !ctrl && !shift {
                self.pl.select_all(false);
            }
            return;
        }
        let i = row as usize;
        if double {
            self.pl.select_only(i);
            self.play_index(i);
            return;
        }
        if shift {
            let a = self.pl.anchor.unwrap_or(i);
            self.pl.select_range(a, i);
        } else if ctrl {
            self.pl.entries[i].selected = !self.pl.entries[i].selected;
            self.pl.anchor = Some(i);
        } else {
            if !self.pl.entries[i].selected {
                self.pl.select_only(i);
            }
            self.pl.anchor = Some(i);
            self.drag = Drag::PlReorder { row: row as isize };
            self.pressed = Some(Hit::PList);
        }
    }

    pub fn pl_scroll_handle(&self) -> (i32, i32) {
        let Some(p) = self.layout().pl else { return (20, 0) };
        let track = (p.h - 58 - 18).max(0);
        let max = self.pl_max_scroll();
        let y = if max == 0 { 0 } else { (self.pl.scroll as f32 / max as f32 * track as f32).round() as i32 };
        (20 + y, track)
    }

    fn menu_action(&mut self, m: u8, i: u8) {
        // keyed by the item's sprite row, so hiding menu items doesn't shift the others
        let Some(&(_, _, y)) = PL_MENUS[m as usize].items.get(i as usize) else { return };
        match (m, y) {
            // ADD: url (not implemented, hidden), dir, file
            (0, 130) => self.open_dialog(DialogKind::AddDir),
            (0, 149) => self.open_dialog(DialogKind::AddFiles),
            // REM: misc (hidden), all, crop, selected
            (1, 111) => {
                self.pl.clear();
                self.dirty_config = true;
            }
            (1, 130) => self.pl.remove_where(|e| !e.selected),
            (1, 149) => self.pl.remove_where(|e| e.selected),
            // SEL: invert, none, all
            (2, 111) => self.pl.invert_selection(),
            (2, 130) => self.pl.select_all(false),
            (2, 149) => self.pl.select_all(true),
            // MISC: sort (file info and misc options are hidden)
            (3, 111) => {
                self.pl.sort_by_title();
                self.flash("PLAYLIST SORTED BY TITLE");
            }
            // LIST: new, save, load
            (4, 111) => {
                self.stop();
                self.pl.clear();
            }
            (4, 130) => self.open_dialog(DialogKind::SavePlaylist),
            (4, 149) => self.open_dialog(DialogKind::LoadPlaylist),
            _ => {}
        }
        self.pl.scroll = self.pl.scroll.min(self.pl_max_scroll());
        self.dirty_config = true;
    }

    pub fn key(&mut self, key: &winit::keyboard::Key, ctrl: bool, alt: bool, shift: bool) {
        use winit::keyboard::{Key, NamedKey};
        match key {
            Key::Character(c) => {
                let c = c.to_lowercase();
                match (c.as_str(), ctrl, alt) {
                    ("z", false, false) => self.prev(),
                    ("x", false, false) => self.play(),
                    ("c", false, false) => self.pause(),
                    ("v", false, false) => self.stop(),
                    ("b", false, false) => self.next(),
                    ("l", false, false) => self.open_dialog(DialogKind::OpenFiles),
                    ("l", true, false) => self.open_dialog(DialogKind::AddDir),
                    ("r", false, false) => self.click(Hit::Repeat),
                    ("s", false, false) => self.click(Hit::Shuffle),
                    ("a", true, false) if self.active == Win::Pl => self.pl.select_all(true),
                    ("a", true, false) => self.toggle_on_top(),
                    ("d", true, false) => self.toggle_double(),
                    ("=" | "+", true, false) => self.zoom_by(1),
                    ("-", true, false) => self.zoom_by(-1),
                    ("0", true, false) => self.set_zoom(100),
                    ("w", true, false) if shift || self.active == Win::Pl => self.toggle_pl_shade(),
                    ("w", true, false) if self.active == Win::Eq => self.toggle_eq_shade(),
                    ("w", true, false) => self.toggle_main_shade(),
                    ("g", false, true) => self.toggle_eq(),
                    ("e", false, true) => self.toggle_pl(),
                    ("s", false, true) => self.open_dialog(DialogKind::LoadSkin),
                    ("i", true, false) => self.pl.invert_selection(),
                    _ => {}
                }
            }
            Key::Named(n) => match n {
                NamedKey::ArrowLeft => self.seek_to(self.position() - 5.0),
                NamedKey::ArrowRight => self.seek_to(self.position() + 5.0),
                NamedKey::ArrowUp | NamedKey::ArrowDown if self.active == Win::Pl && self.pl.len() > 0 => {
                    let cur = self.pl.anchor.unwrap_or(0) as i64;
                    let d = if *n == NamedKey::ArrowUp { -1 } else { 1 };
                    let i = (cur + d).clamp(0, self.pl.len() as i64 - 1) as usize;
                    if shift {
                        let a = self.pl.anchor.unwrap_or(i);
                        self.pl.select_range(a, i);
                    } else {
                        self.pl.select_only(i);
                    }
                    self.ensure_visible(i);
                }
                NamedKey::ArrowUp => self.wheel(1, 1, 1.0),
                NamedKey::ArrowDown => self.wheel(1, 1, -1.0),
                NamedKey::PageUp => self.pl.scroll = self.pl.scroll.saturating_sub(self.pl_visible_rows()),
                NamedKey::PageDown => {
                    self.pl.scroll = (self.pl.scroll + self.pl_visible_rows()).min(self.pl_max_scroll())
                }
                NamedKey::Enter => {
                    if let Some(a) = self.pl.anchor {
                        self.play_index(a);
                    }
                }
                NamedKey::Delete => {
                    self.pl.remove_where(|e| e.selected);
                    self.dirty_config = true;
                }
                NamedKey::Insert => self.open_dialog(DialogKind::AddFiles),
                NamedKey::Space => self.pause(),
                _ => {}
            },
            _ => {}
        }
    }

    /// Files dropped onto the window: skins change the skin, anything else is queued.
    pub fn drop_files(&mut self, files: Vec<PathBuf>, x: i32, y: i32) {
        let (skins, media): (Vec<PathBuf>, Vec<PathBuf>) = files.into_iter().partition(|p| {
            p.extension().map(|e| e.eq_ignore_ascii_case("wsz") || e.eq_ignore_ascii_case("zip")).unwrap_or(false)
                || (p.is_dir() && (p.join("main.bmp").exists() || p.join("MAIN.BMP").exists()))
        });
        if let Some(s) = skins.into_iter().next() {
            self.load_skin(s);
        }
        if media.is_empty() {
            return;
        }
        let on_playlist = matches!(self.which(x, y), Some((Win::Pl, ..)));
        if on_playlist {
            self.add_paths(media);
        } else {
            self.stop();
            self.pl.clear();
            self.add_paths(media);
            if self.pl.len() > 0 {
                self.play_index(0);
            }
        }
    }

    /// The playlist font at `f` times its normal size.
    pub fn zoomed_font(&mut self, f: f64) -> Option<&mut TtfFont> {
        let px = (10.0 * f) as f32;
        if self.ttf_zoomed.as_ref().map(|t| t.px) != Some(px) {
            self.ttf_zoomed = self.ttf.as_ref().map(|t| t.resized(px));
        }
        self.ttf_zoomed.as_mut()
    }

    pub fn load_skin(&mut self, path: PathBuf) {
        match Skin::load(&path) {
            Ok(s) => {
                if s.pl.font != self.skin.pl.font || self.ttf.is_none() {
                    self.ttf = TtfFont::load(&s.pl.font, 10.0);
                    self.ttf_zoomed = None;
                }
                if s.missing.is_empty() {
                    self.flash(format!("SKIN: {}", s.name.to_uppercase()));
                } else {
                    self.flash(format!("SKIN: COULD NOT READ {}", s.missing.join(", ").to_uppercase()));
                }
                self.skin = s;
                self.cfg.skin = Some(std::path::absolute(&path).unwrap_or(path));
                self.dirty_config = true;
                self.requests.push(WinReq::Relayout);
            }
            Err(e) => self.flash(format!("SKIN ERROR: {}", e.to_uppercase())),
        }
    }

    // ---------------------------------------------------------------- toggles

    pub fn toggle_main_shade(&mut self) {
        self.cfg.main_shade = !self.cfg.main_shade;
        self.relayout();
    }
    pub fn toggle_eq_shade(&mut self) {
        if self.skin.eq_ex.is_some() {
            self.cfg.eq_shade = !self.cfg.eq_shade;
            self.relayout();
        }
    }
    pub fn toggle_pl_shade(&mut self) {
        self.cfg.pl_shade = !self.cfg.pl_shade;
        self.relayout();
    }
    pub fn toggle_eq(&mut self) {
        self.cfg.eq_visible = !self.cfg.eq_visible;
        self.relayout();
    }
    pub fn toggle_pl(&mut self) {
        self.cfg.pl_visible = !self.cfg.pl_visible;
        self.relayout();
    }
    /// Classic "double size": switches between 100% and 200%, or back to 100% from any
    /// larger zoom.
    pub fn toggle_double(&mut self) {
        self.set_zoom(if self.cfg.zoom > 100 { 100 } else { 200 });
    }
    pub fn set_zoom(&mut self, z: u32) {
        let z = crate::config::snap_zoom(z);
        self.cfg.double_size = z > 100;
        self.flash(format!("ZOOM: {z}%"));
        if z != self.cfg.zoom {
            self.cfg.zoom = z;
            self.relayout();
        }
    }
    /// Zoom in (positive) or out by `steps` notches, stopping at 50% and 1000%.
    pub fn zoom_by(&mut self, steps: i32) {
        let z = self.cfg.zoom as i32 + steps * crate::config::ZOOM_STEP as i32;
        self.set_zoom(z.max(0) as u32);
    }
    pub fn toggle_on_top(&mut self) {
        self.cfg.always_on_top = !self.cfg.always_on_top;
        self.requests.push(WinReq::OnTop(self.cfg.always_on_top));
        self.flash(if self.cfg.always_on_top { "ALWAYS ON TOP: ON" } else { "ALWAYS ON TOP: OFF" });
        self.dirty_config = true;
    }
    fn relayout(&mut self) {
        self.requests.push(WinReq::Relayout);
        self.dirty_config = true;
    }

    // ---------------------------------------------------------------- dialogs

    pub fn open_dialog(&mut self, kind: DialogKind) {
        let tx = self.dialog_tx.clone();
        // skins: the current skin's folder; everything else: the playing track's folder
        let start_dir = match kind {
            DialogKind::LoadSkin => self.cfg.skin.as_ref().and_then(|p| p.parent().map(|p| p.to_path_buf())),
            _ => self.pl.current.and_then(|c| self.pl.entries.get(c)).and_then(|e| e.path.parent().map(|p| p.to_path_buf())),
        };
        let run = move || {
            let mut d = rfd::FileDialog::new();
            if let Some(dir) = &start_dir {
                d = d.set_directory(dir);
            }
            let audio = media::AUDIO_EXTS;
            let paths: Vec<PathBuf> = match kind {
                DialogKind::OpenFiles | DialogKind::AddFiles => d
                    .set_title("Open file(s)")
                    .add_filter("Audio files", audio)
                    .add_filter("Playlists", &["m3u", "m3u8", "pls", "xspf"])
                    .add_filter("All files", &["*"])
                    .pick_files()
                    .unwrap_or_default(),
                DialogKind::AddDir => d.set_title("Add directory").pick_folder().into_iter().collect(),
                DialogKind::LoadSkin => d
                    .set_title("Load skin")
                    .add_filter("Skins", &["wsz", "zip"])
                    .pick_file()
                    .into_iter()
                    .collect(),
                DialogKind::LoadPlaylist => d
                    .set_title("Load playlist")
                    .add_filter("Playlists", &["m3u", "m3u8", "pls", "xspf"])
                    .pick_file()
                    .into_iter()
                    .collect(),
                DialogKind::SavePlaylist => d
                    .set_title("Save playlist")
                    .set_file_name("playlist.m3u")
                    .add_filter("M3U playlist", &["m3u"])
                    .save_file()
                    .into_iter()
                    .collect(),
            };
            let _ = tx.send(DialogResult { kind, paths });
        };
        // macOS requires dialogs on the main thread; elsewhere keep the UI animating.
        if cfg!(target_os = "macos") {
            run();
        } else {
            std::thread::spawn(run);
        }
    }

    fn handle_dialog(&mut self, res: DialogResult) {
        if res.paths.is_empty() {
            return;
        }
        match res.kind {
            DialogKind::OpenFiles => {
                self.stop();
                self.pl.clear();
                self.add_paths(res.paths);
                if self.pl.len() > 0 {
                    self.play_index(0);
                }
            }
            DialogKind::AddFiles | DialogKind::AddDir => {
                self.add_paths(res.paths);
            }
            DialogKind::LoadSkin => {
                if let Some(p) = res.paths.into_iter().next() {
                    self.load_skin(p);
                }
            }
            DialogKind::LoadPlaylist => {
                self.stop();
                self.pl.clear();
                self.add_paths(res.paths);
            }
            DialogKind::SavePlaylist => {
                if let Some(p) = res.paths.into_iter().next() {
                    match self.pl.save_m3u(&p) {
                        Ok(()) => self.flash("PLAYLIST SAVED"),
                        Err(e) => self.flash(format!("SAVE FAILED: {e}")),
                    }
                }
            }
        }
    }

    pub fn save_config(&mut self) {
        self.cfg.playlist = self.pl.entries.iter().map(|e| e.path.clone()).collect();
        self.cfg.current = self.pl.current;
        self.cfg.save();
        self.dirty_config = false;
    }
}
