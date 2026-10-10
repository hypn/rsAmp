//! Playlist model plus .m3u/.pls/.xspf reading and .m3u writing.

use crate::media;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub title: String,
    pub duration: Option<f64>,
    pub selected: bool,
    pub probed: bool,
}

impl Entry {
    pub fn new(path: PathBuf) -> Entry {
        let title = media::file_title(&path);
        Entry { path, title, duration: None, selected: false, probed: false }
    }
}

#[derive(Default)]
pub struct Playlist {
    pub entries: Vec<Entry>,
    pub current: Option<usize>,
    /// first visible row
    pub scroll: usize,
    /// anchor for shift-click range selection
    pub anchor: Option<usize>,
    history: Vec<usize>,
    rng: u64,
}

impl Playlist {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn add(&mut self, paths: impl IntoIterator<Item = PathBuf>) -> usize {
        let before = self.entries.len();
        for p in paths {
            if p.is_dir() {
                let mut found = Vec::new();
                scan_dir(&p, &mut found, 0);
                found.sort_by_key(|p| p.to_string_lossy().to_lowercase());
                self.entries.extend(found.into_iter().map(Entry::new));
            } else if is_playlist_file(&p) {
                let items = load_playlist(&p);
                self.entries.extend(items);
            } else if media::is_audio(&p) {
                self.entries.push(Entry::new(p));
            }
        }
        self.entries.len() - before
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current = None;
        self.scroll = 0;
        self.anchor = None;
        self.history.clear();
    }

    pub fn total_duration(&self) -> (f64, bool) {
        let mut sum = 0.0;
        let mut complete = true;
        for e in &self.entries {
            match e.duration {
                Some(d) => sum += d,
                None => complete = false,
            }
        }
        (sum, complete)
    }

    pub fn selected_duration(&self) -> (f64, bool) {
        let mut sum = 0.0;
        let mut complete = true;
        for e in self.entries.iter().filter(|e| e.selected) {
            match e.duration {
                Some(d) => sum += d,
                None => complete = false,
            }
        }
        (sum, complete)
    }

    fn rand(&mut self) -> u64 {
        if self.rng == 0 {
            self.rng = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x9E3779B97F4A7C15)
                | 1;
        }
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// Index of the next track; `None` when the end is reached without repeat.
    pub fn next_index(&mut self, shuffle: bool, repeat: bool) -> Option<usize> {
        let n = self.entries.len();
        if n == 0 {
            return None;
        }
        if shuffle {
            if n == 1 {
                return if repeat || self.current.is_none() { Some(0) } else { None };
            }
            let cur = self.current.unwrap_or(usize::MAX);
            loop {
                let i = (self.rand() % n as u64) as usize;
                if i != cur {
                    return Some(i);
                }
            }
        }
        match self.current {
            None => Some(0),
            Some(i) if i + 1 < n => Some(i + 1),
            Some(_) => repeat.then_some(0),
        }
    }

    pub fn prev_index(&mut self, shuffle: bool) -> Option<usize> {
        let n = self.entries.len();
        if n == 0 {
            return None;
        }
        if shuffle {
            if let Some(i) = self.history.pop() {
                if i < n {
                    return Some(i);
                }
            }
        }
        match self.current {
            None => Some(0),
            Some(0) => Some(n - 1),
            Some(i) => Some(i - 1),
        }
    }

    pub fn set_current(&mut self, i: usize, record: bool) {
        if record {
            if let Some(c) = self.current {
                self.history.push(c);
                if self.history.len() > 200 {
                    self.history.remove(0);
                }
            }
        }
        self.current = Some(i);
    }

    pub fn select_only(&mut self, i: usize) {
        for (k, e) in self.entries.iter_mut().enumerate() {
            e.selected = k == i;
        }
        self.anchor = Some(i);
    }

    pub fn select_range(&mut self, a: usize, b: usize) {
        let (lo, hi) = (a.min(b), a.max(b));
        for (k, e) in self.entries.iter_mut().enumerate() {
            e.selected = k >= lo && k <= hi;
        }
    }

    pub fn select_all(&mut self, on: bool) {
        for e in &mut self.entries {
            e.selected = on;
        }
    }

    pub fn invert_selection(&mut self) {
        for e in &mut self.entries {
            e.selected = !e.selected;
        }
    }

    /// Remove entries matching `pred`, keeping `current` pointing at the same track.
    pub fn remove_where(&mut self, pred: impl Fn(&Entry) -> bool) {
        let cur = self.current;
        let mut new_cur = None;
        let mut kept = Vec::with_capacity(self.entries.len());
        for (i, e) in self.entries.drain(..).enumerate() {
            if pred(&e) {
                continue;
            }
            if Some(i) == cur {
                new_cur = Some(kept.len());
            }
            kept.push(e);
        }
        self.entries = kept;
        self.current = new_cur;
        self.history.clear();
        self.anchor = None;
        self.scroll = self.scroll.min(self.entries.len().saturating_sub(1));
    }

    /// Move the selected entries by `delta` rows (drag-reordering).
    pub fn move_selected(&mut self, delta: isize) -> bool {
        let n = self.entries.len() as isize;
        let sel: Vec<usize> = (0..self.entries.len()).filter(|&i| self.entries[i].selected).collect();
        if sel.is_empty() || delta == 0 {
            return false;
        }
        if (delta < 0 && sel[0] as isize + delta < 0) || (delta > 0 && *sel.last().unwrap() as isize + delta >= n) {
            return false;
        }
        let cur_path = self.current.map(|c| c as isize);
        let mut order: Vec<usize> = (0..self.entries.len()).collect();
        let steps = delta.unsigned_abs();
        for _ in 0..steps {
            if delta < 0 {
                for i in 1..order.len() {
                    if self.entries[order[i]].selected && !self.entries[order[i - 1]].selected {
                        order.swap(i, i - 1);
                    }
                }
            } else {
                for i in (0..order.len() - 1).rev() {
                    if self.entries[order[i]].selected && !self.entries[order[i + 1]].selected {
                        order.swap(i, i + 1);
                    }
                }
            }
        }
        let old = std::mem::take(&mut self.entries);
        let mut slots: Vec<Option<Entry>> = old.into_iter().map(Some).collect();
        self.entries = order.iter().map(|&i| slots[i].take().unwrap()).collect();
        if let Some(c) = cur_path {
            self.current = order.iter().position(|&i| i as isize == c);
        }
        self.history.clear();
        true
    }

    pub fn sort_by_title(&mut self) {
        self.sort_with(|e| e.title.to_lowercase());
    }

    fn sort_with(&mut self, key: impl Fn(&Entry) -> String) {
        let cur = self.current.map(|c| self.entries[c].path.clone());
        self.entries.sort_by_key(|e| key(e));
        self.current = cur.and_then(|p| self.entries.iter().position(|e| e.path == p));
        self.history.clear();
    }

    #[allow(dead_code)] // was behind the hidden MISC > "misc opts" item
    pub fn randomize(&mut self) {
        let cur = self.current.map(|c| self.entries[c].path.clone());
        for i in (1..self.entries.len()).rev() {
            let j = (self.rand() % (i as u64 + 1)) as usize;
            self.entries.swap(i, j);
        }
        self.current = cur.and_then(|p| self.entries.iter().position(|e| e.path == p));
        self.history.clear();
    }

    pub fn save_m3u(&self, path: &Path) -> std::io::Result<()> {
        let mut s = String::from("#EXTM3U\n");
        for e in &self.entries {
            let d = e.duration.map(|d| d.round() as i64).unwrap_or(-1);
            s.push_str(&format!("#EXTINF:{},{}\n{}\n", d, e.title, e.path.display()));
        }
        std::fs::write(path, s)
    }
}

fn scan_dir(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 16 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            scan_dir(&p, out, depth + 1);
        } else if media::is_audio(&p) {
            out.push(p);
        }
    }
}

pub fn is_playlist_file(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_ascii_lowercase().as_str(), "m3u" | "m3u8" | "pls" | "xspf"))
        .unwrap_or(false)
}

pub fn load_playlist(path: &Path) -> Vec<Entry> {
    let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
    let text = String::from_utf8_lossy(&bytes);
    let base = path.parent().unwrap_or(Path::new("."));
    let resolve = |s: &str| -> Option<PathBuf> {
        let s = s.trim().trim_start_matches('\u{feff}');
        if s.is_empty() {
            return None;
        }
        let p = if s.contains("://") {
            file_uri_to_path(s)?
        } else {
            PathBuf::from(s.replace('\\', std::path::MAIN_SEPARATOR_STR))
        };
        Some(if p.is_absolute() { p } else { base.join(p) })
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext == "xspf" {
        return load_xspf(&text, |loc| {
            // XSPF locations are URIs; relative ones are relative to the playlist
            if loc.contains("://") {
                resolve(loc)
            } else {
                resolve(&percent_decode(loc))
            }
        });
    }
    let mut out = Vec::new();
    let is_pls = path.extension().map(|e| e.eq_ignore_ascii_case("pls")).unwrap_or(false);
    if is_pls {
        for line in text.lines() {
            let line = line.trim();
            if line.len() > 4 && line[..4].eq_ignore_ascii_case("file") {
                if let Some((_, v)) = line.split_once('=') {
                    if let Some(p) = resolve(v) {
                        out.push(Entry::new(p));
                    }
                }
            }
        }
    } else {
        let mut pending: Option<(Option<f64>, String)> = None;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("#EXTINF:") {
                if let Some((d, t)) = rest.split_once(',') {
                    let d = d.trim().parse::<f64>().ok().filter(|d| *d >= 0.0);
                    pending = Some((d, t.trim().to_string()));
                }
            } else if line.starts_with('#') || line.is_empty() {
                continue;
            } else if let Some(p) = resolve(line) {
                let mut e = Entry::new(p);
                if let Some((d, t)) = pending.take() {
                    e.duration = d;
                    if !t.is_empty() {
                        e.title = t;
                    }
                }
                out.push(e);
            }
        }
    }
    out
}

/// XSPF (XML Shareable Playlist Format, used by VLC): one `<track>` per entry with a
/// `<location>` URI and optional `<title>`, `<creator>` and `<duration>` (milliseconds).
fn load_xspf(text: &str, resolve: impl Fn(&str) -> Option<PathBuf>) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<track>").or_else(|| rest.find("<track ")) {
        let after = &rest[start..];
        let Some(end) = after.find("</track>") else { break };
        let track = &after[..end];
        rest = &after[end + "</track>".len()..];
        let Some(path) = xml_tag(track, "location").and_then(|l| resolve(&l)) else { continue };
        let mut e = Entry::new(path);
        match (xml_tag(track, "creator"), xml_tag(track, "title")) {
            (Some(a), Some(t)) if !a.is_empty() && !t.is_empty() => e.title = format!("{a} - {t}"),
            (_, Some(t)) if !t.is_empty() => e.title = t,
            _ => {}
        }
        e.duration = xml_tag(track, "duration").and_then(|d| d.parse::<f64>().ok()).map(|ms| ms / 1000.0);
        out.push(e);
    }
    out
}

/// Text content of the first `<name>` element in `xml`, with entities decoded.
fn xml_tag(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}");
    let mut from = 0;
    loop {
        let i = from + xml[from..].find(&open)?;
        let after = &xml[i + open.len()..];
        // make sure this is <name> or <name attr=...>, not <nameOther>
        if after.starts_with('>') || after.starts_with(char::is_whitespace) {
            let body = &after[after.find('>')? + 1..];
            let end = body.find(&format!("</{name}>"))?;
            return Some(xml_unescape(body[..end].trim()));
        }
        from = i + open.len();
    }
}

fn xml_unescape(s: &str) -> String {
    if let Some(cdata) = s.strip_prefix("<![CDATA[").and_then(|s| s.strip_suffix("]]>")) {
        return cdata.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.find(';').filter(|&n| n <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent
                .strip_prefix("#x")
                .or_else(|| ent.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Decode `%XX` escapes (as UTF-8) in a URI.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `file://` URI to a local path. Handles `file:///C:/x` and `file:///home/x`,
/// `file://localhost/x`, and network shares (`file://server/share/x` becomes
/// `\\server\share\x` on Windows). Other schemes (http, ...) aren't supported.
pub fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.get(..7).filter(|p| p.eq_ignore_ascii_case("file://")).map(|_| &uri[7..])?;
    let rest = percent_decode(rest);
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest.as_str(), "/"),
    };
    let host = if host.eq_ignore_ascii_case("localhost") { "" } else { host };
    // "/C:/Music" -> "C:/Music"
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':';
    let p = if !host.is_empty() {
        if cfg!(windows) {
            format!(r"\\{host}{}", path.replace('/', "\\"))
        } else {
            format!("//{host}{path}")
        }
    } else if drive {
        path[1..].to_string()
    } else {
        path.to_string()
    };
    Some(PathBuf::from(if cfg!(windows) { p.replace('/', "\\") } else { p }))
}
