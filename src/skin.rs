//! Classic 2.x-format skin loading (.wsz zip archives or extracted directories).

use crate::default_skin;
use crate::gfx::{rgb, Image};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

pub struct PlaylistStyle {
    pub normal: u32,
    pub current: u32,
    pub normal_bg: u32,
    pub selected_bg: u32,
    pub font: String,
}

pub struct Skin {
    pub name: String,
    pub main: Image,
    pub titlebar: Image,
    pub cbuttons: Image,
    pub posbar: Image,
    pub volume: Image,
    pub balance: Image,
    pub shufrep: Image,
    pub monoster: Image,
    pub playpaus: Image,
    pub numbers: Image,
    /// true when `numbers` came from nums_ex.bmp (has its own minus glyph)
    pub nums_ex: bool,
    pub text: Image,
    pub pledit: Image,
    pub eqmain: Image,
    pub eq_ex: Option<Image>,
    pub vis_colors: [u32; 24],
    pub pl: PlaylistStyle,
    /// bitmaps present in the skin that could not be decoded (built-in ones are used instead)
    pub missing: Vec<String>,
}

/// True for .wsz/.zip archives and folders that look like an extracted classic skin.
pub fn is_skin_path(p: &Path) -> bool {
    if p.is_dir() {
        return std::fs::read_dir(p)
            .map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("main.bmp")))
            .unwrap_or(false);
    }
    p.extension().map(|e| e.eq_ignore_ascii_case("wsz") || e.eq_ignore_ascii_case("zip")).unwrap_or(false)
        && p.is_file()
}

/// Look for a skin the way classic players keep them: a `Skins` folder or .wsz files next to the
/// executable (or a few levels up, which covers `target/<profile>/`), or in the working dir.
/// Returns the first candidate that actually loads.
pub fn discover() -> Option<(std::path::PathBuf, Skin)> {
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        roots.extend(dir.ancestors().take(4).map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        if !roots.contains(&cwd) {
            roots.push(cwd);
        }
    }
    for root in roots {
        for dir in [root.join("Skins"), root.join("skins"), root.clone()] {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            let mut found: Vec<std::path::PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    // only unambiguous skins: .wsz archives or folders holding main.bmp
                    (p.is_file() && p.extension().map(|e| e.eq_ignore_ascii_case("wsz")).unwrap_or(false))
                        || (p.is_dir() && is_skin_path(p))
                })
                .collect();
            // prefer .wsz archives, then alphabetical for a stable choice
            found.sort_by_key(|p| (p.is_dir(), p.to_string_lossy().to_lowercase()));
            for p in found {
                if let Ok(skin) = Skin::load(&p) {
                    return Some((p, skin));
                }
            }
        }
    }
    None
}

/// Case-insensitive bag of files from the skin archive, keyed by lowercase basename.
struct Files(HashMap<String, Vec<u8>>);

impl Files {
    fn from_zip(path: &Path) -> Result<Files, String> {
        let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
        let mut map = HashMap::new();
        for i in 0..zip.len() {
            let Ok(mut entry) = zip.by_index(i) else { continue };
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().replace('\\', "/");
            let base = name.rsplit('/').next().unwrap_or(&name).to_ascii_lowercase();
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() {
                // Prefer files nearest the archive root if names collide.
                map.entry(base).or_insert(buf);
            }
        }
        Ok(Files(map))
    }

    fn from_dir(path: &Path) -> Result<Files, String> {
        let mut map = HashMap::new();
        fn walk(dir: &Path, depth: u32, map: &mut HashMap<String, Vec<u8>>) {
            let Ok(rd) = std::fs::read_dir(dir) else { return };
            let mut subdirs = Vec::new();
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    subdirs.push(p);
                } else if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                    if let Ok(data) = std::fs::read(&p) {
                        map.entry(n.to_ascii_lowercase()).or_insert(data);
                    }
                }
            }
            if depth < 2 {
                for d in subdirs {
                    walk(&d, depth + 1, map);
                }
            }
        }
        walk(path, 0, &mut map);
        if map.is_empty() {
            return Err("empty skin directory".into());
        }
        Ok(Files(map))
    }

    fn get(&self, name: &str) -> Option<&[u8]> {
        self.0.get(name).map(|v| v.as_slice())
    }

    fn bmp(&self, name: &str) -> Option<Image> {
        let data = self.get(&format!("{name}.bmp"))?;
        if let Ok(img) = image::load_from_memory_with_format(data, image::ImageFormat::Bmp) {
            let img = img.to_rgba8();
            return Some(Image::from_rgba(img.width(), img.height(), img.as_raw()));
        }
        // Many skins were saved by old tools with sloppy headers; Windows accepts them, so try
        // a lenient decoder, and finally any other format hiding behind a .bmp name.
        crate::bmp::decode(data).or_else(|| {
            let img = image::load_from_memory(data).ok()?.to_rgba8();
            Some(Image::from_rgba(img.width(), img.height(), img.as_raw()))
        })
    }

    fn text(&self, name: &str) -> Option<String> {
        self.get(name).map(|d| String::from_utf8_lossy(d).into_owned())
    }
}

impl Skin {
    pub fn default_skin() -> Skin {
        Skin::from_files(&Files(HashMap::new()), "rsAmp default".into())
    }

    pub fn load(path: &Path) -> Result<Skin, String> {
        let files = if path.is_dir() { Files::from_dir(path)? } else { Files::from_zip(path)? };
        if files.get("main.bmp").is_none() {
            return Err(format!("{}: not a classic skin (no main.bmp)", path.display()));
        }
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "skin".into());
        Ok(Skin::from_files(&files, name))
    }

    fn from_files(f: &Files, name: String) -> Skin {
        let missing = std::cell::RefCell::new(Vec::new());
        let sheet = |n: &str| {
            f.bmp(n).unwrap_or_else(|| {
                if f.get(&format!("{n}.bmp")).is_some() {
                    missing.borrow_mut().push(format!("{n}.bmp"));
                }
                default_skin::sheet(n)
            })
        };
        let (numbers, nums_ex) = match f.bmp("nums_ex") {
            Some(img) => (img, true),
            None => match f.bmp("numbers") {
                Some(img) => (img, false),
                None => (default_skin::sheet("numbers"), false),
            },
        };
        let volume = sheet("volume");
        // Classic skins without balance.bmp reuse the volume bitmap.
        let balance = f.bmp("balance").unwrap_or_else(|| {
            if f.get("volume.bmp").is_some() {
                volume.clone()
            } else {
                default_skin::sheet("balance")
            }
        });
        let vis_colors = f
            .text("viscolor.txt")
            .and_then(|t| parse_viscolor(&t))
            .unwrap_or_else(default_skin::vis_colors);
        let pl = parse_pledit(f.text("pledit.txt").as_deref().unwrap_or(""));
        Skin {
            name,
            main: sheet("main"),
            titlebar: sheet("titlebar"),
            cbuttons: sheet("cbuttons"),
            posbar: sheet("posbar"),
            volume,
            balance,
            shufrep: sheet("shufrep"),
            monoster: sheet("monoster"),
            playpaus: sheet("playpaus"),
            numbers,
            nums_ex,
            text: sheet("text"),
            pledit: sheet("pledit"),
            eqmain: sheet("eqmain"),
            eq_ex: f.bmp("eq_ex"),
            vis_colors,
            pl,
            missing: missing.take(),
        }
    }
}

fn parse_viscolor(text: &str) -> Option<[u32; 24]> {
    let mut colors = default_skin::vis_colors();
    let mut n = 0;
    for line in text.lines() {
        if n >= 24 {
            break;
        }
        let nums: Vec<u8> = line
            .split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .take(3)
            .filter_map(|s| s.parse::<u32>().ok().map(|v| v.min(255) as u8))
            .collect();
        if nums.len() == 3 {
            colors[n] = rgb(nums[0], nums[1], nums[2]);
            n += 1;
        }
    }
    (n > 0).then_some(colors)
}

fn parse_color(s: &str) -> Option<u32> {
    let s = s.trim().trim_start_matches('#');
    let hex: String = s.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() < 6 {
        return None;
    }
    u32::from_str_radix(&hex[..6], 16).ok()
}

fn parse_pledit(text: &str) -> PlaylistStyle {
    let mut st = PlaylistStyle {
        normal: rgb(0, 255, 0),
        current: rgb(255, 255, 255),
        normal_bg: rgb(0, 0, 0),
        selected_bg: rgb(0, 0, 198),
        font: "Arial".into(),
    };
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        let k = k.trim().to_ascii_lowercase();
        match k.as_str() {
            "normal" => st.normal = parse_color(v).unwrap_or(st.normal),
            "current" => st.current = parse_color(v).unwrap_or(st.current),
            "normalbg" => st.normal_bg = parse_color(v).unwrap_or(st.normal_bg),
            "selectedbg" => st.selected_bg = parse_color(v).unwrap_or(st.selected_bg),
            "font" => st.font = v.trim().to_string(),
            _ => {}
        }
    }
    st
}
