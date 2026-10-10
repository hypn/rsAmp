//! Persistent settings (JSON in the per-user config directory).

use crate::vis::VisMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub skin: Option<PathBuf>,
    pub volume: f32,
    pub balance: f32,
    pub eq_on: bool,
    pub eq_auto: bool,
    pub eq_preamp: f32,
    pub eq_bands: [f32; 10],
    pub shuffle: bool,
    pub repeat: bool,
    pub always_on_top: bool,
    pub double_size: bool,
    /// window zoom in percent (50–1000, in `ZOOM_STEP`s)
    pub zoom: u32,
    pub main_shade: bool,
    pub eq_visible: bool,
    pub eq_shade: bool,
    pub pl_visible: bool,
    pub pl_shade: bool,
    pub pl_w: i32,
    pub pl_h: i32,
    pub time_remaining: bool,
    pub vis_mode: VisMode,
    pub window_pos: Option<(i32, i32)>,
    pub playlist: Vec<PathBuf>,
    pub current: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            skin: None,
            volume: 0.75,
            balance: 0.0,
            eq_on: false,
            eq_auto: false,
            eq_preamp: 0.0,
            eq_bands: [0.0; 10],
            shuffle: false,
            repeat: false,
            always_on_top: false,
            double_size: false,
            zoom: 100,
            main_shade: false,
            eq_visible: true,
            eq_shade: false,
            pl_visible: true,
            pl_shade: false,
            pl_w: 275,
            pl_h: 116,
            time_remaining: false,
            vis_mode: VisMode::Spectrum,
            window_pos: None,
            playlist: Vec::new(),
            current: None,
        }
    }
}

/// Settings live in the per-user config directory
/// (`%APPDATA%\rsAmp\config.json`, `~/.config/rsAmp/config.json`,
/// `~/Library/Application Support/rsAmp/config.json`).
fn path() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    let dir = base.join("rsAmp");
    // early builds used a lowercase `rsamp` folder; rename it so nothing is lost
    if let Ok(rd) = std::fs::read_dir(&base) {
        for e in rd.flatten() {
            let name = e.file_name();
            if name != "rsAmp" && name.to_string_lossy().eq_ignore_ascii_case("rsamp") {
                let _ = std::fs::rename(e.path(), &dir);
            }
        }
    }
    Some(dir.join("config.json"))
}

pub const ZOOM_MIN: u32 = 50;
pub const ZOOM_MAX: u32 = 1000;
/// One Ctrl+wheel notch.
pub const ZOOM_STEP: u32 = 10;

/// Clamp to the allowed range and round to a whole step.
pub fn snap_zoom(z: u32) -> u32 {
    ((z + ZOOM_STEP / 2) / ZOOM_STEP * ZOOM_STEP).clamp(ZOOM_MIN, ZOOM_MAX)
}

impl Config {
    pub fn load() -> Config {
        let mut cfg: Config = path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|d| serde_json::from_slice(&d).ok())
            .unwrap_or_default();
        // configs from before zoom existed only know "double size"
        if cfg.double_size && cfg.zoom == 100 {
            cfg.zoom = 200;
        }
        cfg.zoom = snap_zoom(cfg.zoom);
        cfg.double_size = cfg.zoom > 100;
        cfg
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
}
