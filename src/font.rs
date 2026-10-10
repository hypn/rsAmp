//! Text rendering: the skin's TEXT.BMP bitmap font, a tiny built-in font used to draw the
//! fallback skin, and a TrueType font for the playlist (classic skins use a system font there).

use crate::gfx::{blend, Image, Rect};
use crate::skin::Skin;

/// Character -> (column, row) in TEXT.BMP (5x6 cells).
pub const TEXT_MAP: &[(char, (i32, i32))] = &[
    ('A', (0, 0)), ('B', (1, 0)), ('C', (2, 0)), ('D', (3, 0)), ('E', (4, 0)), ('F', (5, 0)),
    ('G', (6, 0)), ('H', (7, 0)), ('I', (8, 0)), ('J', (9, 0)), ('K', (10, 0)), ('L', (11, 0)),
    ('M', (12, 0)), ('N', (13, 0)), ('O', (14, 0)), ('P', (15, 0)), ('Q', (16, 0)), ('R', (17, 0)),
    ('S', (18, 0)), ('T', (19, 0)), ('U', (20, 0)), ('V', (21, 0)), ('W', (22, 0)), ('X', (23, 0)),
    ('Y', (24, 0)), ('Z', (25, 0)), ('"', (26, 0)), ('@', (27, 0)), (' ', (30, 0)),
    ('0', (0, 1)), ('1', (1, 1)), ('2', (2, 1)), ('3', (3, 1)), ('4', (4, 1)), ('5', (5, 1)),
    ('6', (6, 1)), ('7', (7, 1)), ('8', (8, 1)), ('9', (9, 1)), ('…', (10, 1)), ('.', (11, 1)),
    (':', (12, 1)), ('(', (13, 1)), (')', (14, 1)), ('-', (15, 1)), ('\'', (16, 1)), ('!', (17, 1)),
    ('_', (18, 1)), ('+', (19, 1)), ('\\', (20, 1)), ('/', (21, 1)), ('[', (22, 1)), (']', (23, 1)),
    ('^', (24, 1)), ('&', (25, 1)), ('%', (26, 1)), (',', (27, 1)), ('=', (28, 1)), ('$', (29, 1)),
    ('#', (30, 1)),
    ('Å', (0, 2)), ('Ö', (1, 2)), ('Ä', (2, 2)), ('?', (3, 2)), ('*', (4, 2)),
];

fn text_cell(c: char) -> (i32, i32) {
    let c = match c {
        '<' | '{' => '[',
        '>' | '}' => ']',
        '`' | '´' | '’' | '‘' => '\'',
        '“' | '”' => '"',
        '|' => '!',
        '~' => '-',
        ';' => ':',
        _ => c,
    };
    let up = c.to_uppercase().next().unwrap_or(c);
    // strip common accents
    let up = match up {
        'À' | 'Á' | 'Â' | 'Ã' => 'A',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ø' => 'O',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'Ç' => 'C',
        'Ñ' => 'N',
        'Ý' => 'Y',
        'ß' => 'S',
        x => x,
    };
    TEXT_MAP.iter().find(|(ch, _)| *ch == up).map(|(_, p)| *p).unwrap_or((30, 0))
}

/// Draw `s` with the skin's TEXT.BMP font, clipped to `clip`.
pub fn draw_text(dst: &mut Image, skin: &Skin, x: i32, y: i32, s: &str, clip: Rect) {
    let mut cx = x;
    for ch in s.chars() {
        if cx >= clip.x + clip.w {
            break;
        }
        if cx + 5 > clip.x {
            let (col, row) = text_cell(ch);
            dst.blit_clipped(&skin.text, crate::gfx::r(col * 5, row * 6, 5, 6), cx, y, clip);
        }
        cx += 5;
    }
}

/// 3x5 glyphs for the built-in skin. Each row holds 3 bits, MSB = left.
pub fn tiny_glyph(c: char) -> Option<[u8; 5]> {
    let c = c.to_ascii_uppercase();
    Some(match c {
        'A' | 'Å' | 'Ä' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' | 'Ö' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b110, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b110],
        '.' => [0, 0, 0, 0, 0b010],
        ':' => [0, 0b010, 0, 0b010, 0],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        '-' => [0, 0, 0b111, 0, 0],
        '\'' => [0b010, 0b010, 0, 0, 0],
        '!' => [0b010, 0b010, 0b010, 0, 0b010],
        '_' => [0, 0, 0, 0, 0b111],
        '+' => [0, 0b010, 0b111, 0b010, 0],
        '\\' => [0b100, 0b100, 0b010, 0b001, 0b001],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '[' => [0b011, 0b010, 0b010, 0b010, 0b011],
        ']' => [0b110, 0b010, 0b010, 0b010, 0b110],
        '^' => [0b010, 0b101, 0, 0, 0],
        '&' => [0b010, 0b101, 0b010, 0b101, 0b011],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        ',' => [0, 0, 0, 0b010, 0b100],
        '=' => [0, 0b111, 0, 0b111, 0],
        '$' => [0b011, 0b110, 0b010, 0b011, 0b110],
        '#' => [0b101, 0b111, 0b101, 0b111, 0b101],
        '"' => [0b101, 0b101, 0, 0, 0],
        '@' => [0b010, 0b101, 0b111, 0b100, 0b011],
        '…' => [0, 0, 0, 0, 0b101],
        '?' => [0b110, 0b001, 0b010, 0, 0b010],
        '*' => [0, 0b101, 0b010, 0b101, 0],
        _ => return None,
    })
}

/// TrueType font loaded from the system for the playlist editor.
pub struct TtfFont {
    font: fontdue::Font,
    pub px: f32,
    cache: std::collections::HashMap<char, (fontdue::Metrics, Vec<u8>)>,
}

impl TtfFont {
    /// Try to find the skin's preferred font (usually Arial) or a metric-compatible fallback.
    pub fn load(preferred: &str, px: f32) -> Option<TtfFont> {
        let pref = preferred.to_ascii_lowercase().replace(' ', "");
        let mut names: Vec<String> = Vec::new();
        if !pref.is_empty() {
            names.push(pref.clone());
        }
        for n in ["arial", "liberationsans-regular", "arimo-regular", "dejavusans", "notosans-regular", "ubuntu-r", "helvetica", "verdana", "tahoma"] {
            names.push(n.to_string());
        }
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        if cfg!(windows) {
            let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
            dirs.push(std::path::Path::new(&windir).join("Fonts"));
        } else if cfg!(target_os = "macos") {
            dirs.extend(["/System/Library/Fonts/Supplemental", "/System/Library/Fonts", "/Library/Fonts"].map(Into::into));
            if let Some(h) = dirs::home_dir() {
                dirs.push(h.join("Library/Fonts"));
            }
        } else {
            dirs.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(Into::into));
            if let Some(h) = dirs::home_dir() {
                dirs.push(h.join(".fonts"));
                dirs.push(h.join(".local/share/fonts"));
            }
        }
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        fn collect(dir: &std::path::Path, depth: u32, out: &mut Vec<std::path::PathBuf>) {
            let Ok(rd) = std::fs::read_dir(dir) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if depth < 4 {
                        collect(&p, depth + 1, out);
                    }
                } else if p.extension().map(|e| e.eq_ignore_ascii_case("ttf")).unwrap_or(false) {
                    out.push(p);
                }
            }
        }
        for d in &dirs {
            collect(d, 0, &mut files);
        }
        for want in &names {
            for f in &files {
                let stem = f.file_stem().unwrap_or_default().to_string_lossy().to_ascii_lowercase().replace(' ', "");
                if stem == *want {
                    if let Ok(data) = std::fs::read(f) {
                        if let Ok(font) = fontdue::Font::from_bytes(data, fontdue::FontSettings::default()) {
                            return Some(TtfFont { font, px, cache: Default::default() });
                        }
                    }
                }
            }
        }
        None
    }

    /// Same typeface at another pixel size (for drawing at the zoomed screen resolution).
    pub fn resized(&self, px: f32) -> TtfFont {
        TtfFont { font: self.font.clone(), px, cache: Default::default() }
    }

    fn glyph(&mut self, c: char) -> &(fontdue::Metrics, Vec<u8>) {
        let px = self.px;
        let font = &self.font;
        self.cache.entry(c).or_insert_with(|| font.rasterize(c, px))
    }

    pub fn width(&mut self, s: &str) -> i32 {
        let mut w = 0.0;
        for c in s.chars() {
            w += self.glyph(c).0.advance_width;
        }
        w.round() as i32
    }

    /// Draw with baseline at `baseline`, clipped to `clip`. Returns the advance.
    /// Pixels inside `hole` (something drawn on top, like a menu) are left untouched.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, dst: &mut Image, x: i32, baseline: i32, s: &str, color: u32, clip: Rect, hole: Option<Rect>) -> i32 {
        let mut pen = x as f32;
        for c in s.chars() {
            let (m, bmp) = self.glyph(c).clone();
            let gx = pen.round() as i32 + m.xmin;
            let gy = baseline - m.height as i32 - m.ymin;
            for j in 0..m.height as i32 {
                for i in 0..m.width as i32 {
                    let a = bmp[(j * m.width as i32 + i) as usize];
                    let (px, py) = (gx + i, gy + j);
                    if a > 0 && clip.contains(px, py) && !hole.is_some_and(|h| h.contains(px, py)) {
                        // Slightly crisp up the coverage to mimic classic un-antialiased text.
                        let a = ((a as u32 * 3 / 2).min(255)) as u8;
                        let cur = dst.get(px, py);
                        dst.set(px, py, blend(cur, color, a));
                    }
                }
            }
            pen += m.advance_width;
        }
        (pen - x as f32).round() as i32
    }
}
