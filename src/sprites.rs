//! Sprite-sheet coordinates and on-screen positions for classic 2.x-format skins.
#![allow(dead_code)]

use crate::gfx::{r, Rect};

// ---------- titlebar.bmp ----------
pub const MAIN_TITLE_BAR: Rect = r(27, 15, 275, 14);
pub const MAIN_TITLE_BAR_SELECTED: Rect = r(27, 0, 275, 14);
pub const MAIN_OPTIONS: Rect = r(0, 0, 9, 9);
pub const MAIN_OPTIONS_DOWN: Rect = r(0, 9, 9, 9);
pub const MAIN_MINIMIZE: Rect = r(9, 0, 9, 9);
pub const MAIN_MINIMIZE_DOWN: Rect = r(9, 9, 9, 9);
pub const MAIN_SHADE: Rect = r(0, 18, 9, 9);
pub const MAIN_SHADE_DOWN: Rect = r(9, 18, 9, 9);
pub const MAIN_UNSHADE: Rect = r(0, 27, 9, 9);
pub const MAIN_UNSHADE_DOWN: Rect = r(9, 27, 9, 9);
pub const MAIN_CLOSE: Rect = r(18, 0, 9, 9);
pub const MAIN_CLOSE_DOWN: Rect = r(18, 9, 9, 9);
pub const MAIN_SHADE_BG: Rect = r(27, 42, 275, 14);
pub const MAIN_SHADE_BG_SELECTED: Rect = r(27, 29, 275, 14);
pub const MAIN_SHADE_POS_BG: Rect = r(0, 36, 17, 7);
pub const MAIN_SHADE_POS_THUMB: Rect = r(20, 36, 3, 7);
pub const MAIN_SHADE_POS_THUMB_LEFT: Rect = r(17, 36, 3, 7);
pub const MAIN_SHADE_POS_THUMB_RIGHT: Rect = r(23, 36, 3, 7);
pub const CLUTTER_BG: Rect = r(304, 0, 8, 43);
pub const CLUTTER_BG_DISABLED: Rect = r(312, 0, 8, 43);
/// (y offset inside the bar, height, selected sprite x) for O, A, I, D, V.
pub const CLUTTER_BUTTONS: [(i32, i32, i32); 5] =
    [(3, 8, 304), (11, 7, 312), (18, 7, 320), (25, 8, 328), (33, 7, 336)];

// Main window positions
pub const POS_OPTIONS: Rect = r(6, 3, 9, 9);
pub const POS_MINIMIZE: Rect = r(244, 3, 9, 9);
pub const POS_SHADE: Rect = r(254, 3, 9, 9);
pub const POS_CLOSE: Rect = r(264, 3, 9, 9);
pub const POS_CLUTTER: Rect = r(10, 22, 8, 43);

// ---------- cbuttons.bmp ----------
// (sheet x, width, height, screen x, screen y)
pub const CBUTTONS: [(i32, i32, i32, i32, i32); 6] = [
    (0, 23, 18, 16, 88),   // previous
    (23, 23, 18, 39, 88),  // play
    (46, 23, 18, 62, 88),  // pause
    (69, 23, 18, 85, 88),  // stop
    (92, 22, 18, 108, 88), // next
    (114, 22, 16, 136, 89),// eject
];

// ---------- shufrep.bmp ----------
pub const POS_REPEAT: Rect = r(210, 89, 28, 15);
pub const POS_SHUFFLE: Rect = r(164, 89, 47, 15);
pub const POS_EQ_BUTTON: Rect = r(219, 58, 23, 12);
pub const POS_PL_BUTTON: Rect = r(242, 58, 23, 12);

// ---------- posbar.bmp ----------
pub const POSBAR_BG: Rect = r(0, 0, 248, 10);
pub const POSBAR_THUMB: Rect = r(248, 0, 29, 10);
pub const POSBAR_THUMB_DOWN: Rect = r(278, 0, 29, 10);
pub const POS_POSBAR: Rect = r(16, 72, 248, 10);

// ---------- volume.bmp / balance.bmp ----------
pub const POS_VOLUME: Rect = r(107, 57, 68, 13);
pub const POS_BALANCE: Rect = r(177, 57, 38, 13);
pub const VOL_THUMB: Rect = r(15, 422, 14, 11);
pub const VOL_THUMB_DOWN: Rect = r(0, 422, 14, 11);

// ---------- monoster.bmp ----------
pub const STEREO_ON: Rect = r(0, 0, 29, 12);
pub const STEREO_OFF: Rect = r(0, 12, 29, 12);
pub const MONO_ON: Rect = r(29, 0, 27, 12);
pub const MONO_OFF: Rect = r(29, 12, 27, 12);
pub const POS_STEREO: (i32, i32) = (239, 41);
pub const POS_MONO: (i32, i32) = (212, 41);

// ---------- playpaus.bmp ----------
pub const IND_PLAYING: Rect = r(0, 0, 9, 9);
pub const IND_PAUSED: Rect = r(9, 0, 9, 9);
pub const IND_STOPPED: Rect = r(18, 0, 9, 9);
pub const IND_NOT_WORKING: Rect = r(39, 0, 3, 9);
pub const IND_WORKING: Rect = r(36, 0, 3, 9);
pub const POS_PLAYPAUS: (i32, i32) = (26, 28);

// ---------- main window LCD ----------
pub const POS_TIME: Rect = r(36, 26, 63, 13);
pub const POS_MARQUEE: Rect = r(111, 27, 154, 6);
pub const POS_KBPS: (i32, i32) = (111, 43);
pub const POS_KHZ: (i32, i32) = (156, 43);
pub const POS_VIS: Rect = r(24, 43, 76, 16);

// Shade-mode main window
pub const SHADE_POS: Rect = r(226, 4, 17, 7);
pub const SHADE_VIS: Rect = r(79, 5, 38, 5);
pub const SHADE_TIME: (i32, i32) = (127, 4);
/// Invisible click regions over the shade background: prev, play, pause, stop, next, eject.
pub const SHADE_BUTTONS: [Rect; 6] = [
    r(169, 2, 8, 10),
    r(177, 2, 10, 10),
    r(187, 2, 10, 10),
    r(197, 2, 9, 10),
    r(206, 2, 8, 10),
    r(216, 2, 9, 10),
];

// ---------- eqmain.bmp ----------
pub const EQ_TITLE: Rect = r(0, 149, 275, 14);
pub const EQ_TITLE_SELECTED: Rect = r(0, 134, 275, 14);
pub const EQ_SLIDER_BG: Rect = r(13, 164, 209, 129);
pub const EQ_THUMB: Rect = r(0, 164, 11, 11);
pub const EQ_THUMB_DOWN: Rect = r(0, 176, 11, 11);
pub const EQ_ON: Rect = r(10, 119, 26, 12);
pub const EQ_ON_DOWN: Rect = r(128, 119, 26, 12);
pub const EQ_ON_SEL: Rect = r(69, 119, 26, 12);
pub const EQ_ON_SEL_DOWN: Rect = r(187, 119, 26, 12);
pub const EQ_AUTO: Rect = r(36, 119, 32, 12);
pub const EQ_AUTO_DOWN: Rect = r(154, 119, 32, 12);
pub const EQ_AUTO_SEL: Rect = r(95, 119, 32, 12);
pub const EQ_AUTO_SEL_DOWN: Rect = r(213, 119, 32, 12);
pub const EQ_GRAPH_BG: Rect = r(0, 294, 113, 19);
pub const EQ_GRAPH_COLORS: Rect = r(115, 294, 1, 19);
pub const EQ_PREAMP_LINE: Rect = r(0, 314, 113, 1);
pub const EQ_PRESETS: Rect = r(224, 164, 44, 12);
pub const EQ_PRESETS_DOWN: Rect = r(224, 176, 44, 12);
pub const EQ_CLOSE_DOWN: Rect = r(0, 116, 9, 9);

pub const POS_EQ_ON: Rect = r(14, 18, 26, 12);
pub const POS_EQ_AUTO: Rect = r(40, 18, 32, 12);
pub const POS_EQ_GRAPH: Rect = r(86, 17, 113, 19);
pub const POS_EQ_PRESETS: Rect = r(217, 18, 44, 12);
pub const POS_EQ_CLOSE: Rect = r(264, 3, 9, 9);
pub const POS_EQ_SHADE: Rect = r(254, 3, 9, 9);
/// x positions of preamp + 10 band sliders
pub const EQ_SLIDER_X: [i32; 11] = [21, 78, 96, 114, 132, 150, 168, 186, 204, 222, 240];
pub const EQ_SLIDER_Y: i32 = 38;
pub const EQ_SLIDER_W: i32 = 14;
pub const EQ_SLIDER_H: i32 = 63;

// ---------- eq_ex.bmp (equalizer window-shade) ----------
pub const EQS_BG_SELECTED: Rect = r(0, 0, 275, 14);
pub const EQS_BG: Rect = r(0, 15, 275, 14);
// The shade background already contains the normal buttons; these are the pressed states.
pub const EQS_UNSHADE_DOWN: Rect = r(1, 38, 9, 9);
pub const EQS_CLOSE_DOWN: Rect = r(11, 47, 9, 9);
/// pressed shade button of the full-size equalizer (also in eq_ex.bmp)
pub const EQS_SHADE_DOWN: Rect = r(1, 47, 9, 9);
pub const EQS_VOL_THUMB_LEFT: Rect = r(1, 30, 3, 7);
pub const EQS_VOL_THUMB_CENTER: Rect = r(4, 30, 3, 7);
pub const EQS_VOL_THUMB_RIGHT: Rect = r(7, 30, 3, 7);
pub const EQS_BAL_THUMB_LEFT: Rect = r(11, 30, 3, 7);
pub const EQS_BAL_THUMB_CENTER: Rect = r(14, 30, 3, 7);
pub const EQS_BAL_THUMB_RIGHT: Rect = r(17, 30, 3, 7);
pub const POS_EQS_VOL: Rect = r(61, 4, 97, 7);
pub const POS_EQS_BAL: Rect = r(164, 4, 43, 7);

// ---------- pledit.bmp ----------
pub const PL_TOP_LEFT: Rect = r(0, 21, 25, 20);
pub const PL_TITLE: Rect = r(26, 21, 100, 20);
pub const PL_TOP_TILE: Rect = r(127, 21, 25, 20);
pub const PL_TOP_RIGHT: Rect = r(153, 21, 25, 20);
pub const PL_TOP_LEFT_SEL: Rect = r(0, 0, 25, 20);
pub const PL_TITLE_SEL: Rect = r(26, 0, 100, 20);
pub const PL_TOP_TILE_SEL: Rect = r(127, 0, 25, 20);
pub const PL_TOP_RIGHT_SEL: Rect = r(153, 0, 25, 20);
pub const PL_LEFT_TILE: Rect = r(0, 42, 12, 29);
pub const PL_RIGHT_TILE: Rect = r(31, 42, 20, 29);
pub const PL_BOTTOM_TILE: Rect = r(179, 0, 25, 38);
pub const PL_BOTTOM_LEFT: Rect = r(0, 72, 125, 38);
pub const PL_BOTTOM_RIGHT: Rect = r(126, 72, 150, 38);
pub const PL_VIS_BG: Rect = r(205, 0, 75, 38);
pub const PL_SHADE_BG: Rect = r(72, 57, 25, 14);
pub const PL_SHADE_LEFT: Rect = r(72, 42, 25, 14);
pub const PL_SHADE_RIGHT: Rect = r(99, 57, 50, 14);
pub const PL_SHADE_RIGHT_SEL: Rect = r(99, 42, 50, 14);
pub const PL_SCROLL_HANDLE: Rect = r(52, 53, 8, 18);
pub const PL_SCROLL_HANDLE_DOWN: Rect = r(61, 53, 8, 18);
pub const PL_CLOSE_DOWN: Rect = r(52, 42, 9, 9);
pub const PL_SHADE_DOWN: Rect = r(62, 42, 9, 9);
pub const PL_UNSHADE_DOWN: Rect = r(150, 42, 9, 9);
pub const PL_SHADE_CLOSE_DOWN: Rect = r(52, 42, 9, 9);

/// Popup menus from the playlist's bottom bar: (button x from left or negative from right,
/// bar sprite, items as (normal, selected) sprite x/y, listed top to bottom).
/// Items rsAmp doesn't implement are commented out (see AGENTS.md); the bar is cropped to
/// the remaining items' height.
pub struct PlMenu {
    pub button_x: i32,
    pub from_right: bool,
    pub bar: Rect,
    pub items: &'static [(i32, i32, i32)], // (normal x, selected x, y)
}

pub const PL_MENUS: [PlMenu; 5] = [
    // ADD: url, dir, file
    PlMenu { button_x: 14, from_right: false, bar: r(48, 129, 3, 36),
        items: &[/* (0, 23, 111) add url, */ (0, 23, 130), (0, 23, 149)] },
    // REM: misc, all, crop, selected
    PlMenu { button_x: 43, from_right: false, bar: r(100, 129, 3, 54),
        items: &[/* (54, 77, 168) rem misc, */ (54, 77, 111), (54, 77, 130), (54, 77, 149)] },
    // SEL: invert, none, all
    PlMenu { button_x: 72, from_right: false, bar: r(150, 111, 3, 54),
        items: &[(104, 127, 111), (104, 127, 130), (104, 127, 149)] },
    // MISC: sort, file info, misc options
    PlMenu { button_x: 101, from_right: false, bar: r(200, 147, 3, 18),
        items: &[(154, 177, 111) /* , (154, 177, 130) file info, (154, 177, 149) misc opts */] },
    // LIST: new, save, load
    PlMenu { button_x: 44, from_right: true, bar: r(250, 111, 3, 54),
        items: &[(204, 227, 111), (204, 227, 130), (204, 227, 149)] },
];
pub const PL_MENU_ITEM_W: i32 = 22;
pub const PL_MENU_ITEM_H: i32 = 18;
