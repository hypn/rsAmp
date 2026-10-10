//! Track metadata: tags, duration, bitrate and sample rate.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const AUDIO_EXTS: &[&str] = &["mp3", "mp2", "mp1", "flac", "ogg", "oga", "wav", "m4a", "aac", "mp4"];

pub fn is_audio(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
        .unwrap_or(false)
}

#[derive(Clone, Debug, Default)]
pub struct TrackInfo {
    pub title: String,
    pub duration: Option<f64>,
    pub kbps: Option<u32>,
    pub khz: Option<u32>,
    pub channels: Option<u32>,
}

pub fn file_title(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

pub fn probe(path: &Path) -> TrackInfo {
    let mut info = TrackInfo { title: file_title(path), ..Default::default() };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let is_mpeg = matches!(ext.as_str(), "mp3" | "mp2" | "mp1");
    if is_mpeg {
        if let Ok(tag) = id3::v1v2::read_from_path(path) {
            use id3::TagLike;
            if let Some(t) = format_title(tag.artist(), tag.title()) {
                info.title = t;
            }
        }
        if let Some(h) = mp3_header_info(path) {
            info.duration = Some(h.duration);
            info.kbps = Some(h.kbps);
            info.khz = Some(h.sample_rate / 1000);
            info.channels = Some(h.channels);
        }
    } else {
        symphonia_info(path, &mut info);
    }
    info
}

fn format_title(artist: Option<&str>, title: Option<&str>) -> Option<String> {
    let clean = |s: Option<&str>| s.map(|s| s.trim().trim_matches('\0').to_string()).filter(|s| !s.is_empty());
    match (clean(artist), clean(title)) {
        (Some(a), Some(t)) => Some(format!("{a} - {t}")),
        (None, Some(t)) => Some(t),
        _ => None,
    }
}

fn symphonia_info(path: &Path, info: &mut TrackInfo) {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::{MetadataOptions, StandardTagKey};
    use symphonia::core::probe::Hint;
    let Ok(file) = std::fs::File::open(path) else { return };
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(e) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(e);
    }
    let Ok(mut probed) =
        symphonia::default::get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
    else {
        return;
    };
    let mut artist = None;
    let mut title = None;
    let mut scan = |rev: &symphonia::core::meta::MetadataRevision| {
        for t in rev.tags() {
            match t.std_key {
                Some(StandardTagKey::Artist) => artist = Some(t.value.to_string()),
                Some(StandardTagKey::TrackTitle) => title = Some(t.value.to_string()),
                _ => {}
            }
        }
    };
    if let Some(m) = probed.metadata.get() {
        if let Some(rev) = m.current() {
            scan(rev);
        }
    }
    if let Some(rev) = probed.format.metadata().current() {
        scan(rev);
    }
    if let Some(t) = format_title(artist.as_deref(), title.as_deref()) {
        info.title = t;
    }
    if let Some(track) = probed.format.default_track() {
        let p = &track.codec_params;
        info.khz = p.sample_rate.map(|r| r / 1000);
        info.channels = p.channels.map(|c| c.count() as u32);
        if let (Some(n), Some(r)) = (p.n_frames, p.sample_rate) {
            let d = n as f64 / r as f64;
            info.duration = Some(d);
            if d > 0.0 {
                info.kbps = Some((size as f64 * 8.0 / d / 1000.0).round() as u32);
            }
        }
    }
}

pub struct Mp3Header {
    pub duration: f64,
    pub kbps: u32,
    pub sample_rate: u32,
    pub channels: u32,
}

const BITRATES: [[[u32; 16]; 3]; 2] = [
    // MPEG-1: layer 1, 2, 3
    [
        [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0],
        [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 0],
        [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0],
    ],
    // MPEG-2/2.5: layer 1, 2, 3
    [
        [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 0],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
    ],
];

struct Frame {
    mpeg1: bool,
    layer: usize, // 1..=3
    kbps: u32,
    rate: u32,
    channels: u32,
    len: usize,
}

fn parse_frame(h: &[u8]) -> Option<Frame> {
    if h.len() < 4 || h[0] != 0xFF || h[1] & 0xE0 != 0xE0 {
        return None;
    }
    let ver = (h[1] >> 3) & 3;
    let layer_bits = (h[1] >> 1) & 3;
    if ver == 1 || layer_bits == 0 {
        return None;
    }
    let layer = 4 - layer_bits as usize;
    let br_idx = (h[2] >> 4) as usize;
    let sr_idx = ((h[2] >> 2) & 3) as usize;
    if br_idx == 0 || br_idx == 15 || sr_idx == 3 {
        return None;
    }
    let pad = ((h[2] >> 1) & 1) as usize;
    let mpeg1 = ver == 3;
    let base = [44100, 48000, 32000][sr_idx];
    let rate = match ver {
        3 => base,
        2 => base / 2,
        _ => base / 4,
    };
    let kbps = BITRATES[if mpeg1 { 0 } else { 1 }][layer - 1][br_idx];
    let channels = if h[3] >> 6 == 3 { 1 } else { 2 };
    let len = match layer {
        1 => (12 * kbps as usize * 1000 / rate as usize + pad) * 4,
        3 if !mpeg1 => 72 * kbps as usize * 1000 / rate as usize + pad,
        _ => 144 * kbps as usize * 1000 / rate as usize + pad,
    };
    Some(Frame { mpeg1, layer, kbps, rate, channels, len })
}

pub fn mp3_header_info(path: &Path) -> Option<Mp3Header> {
    let mut f = std::fs::File::open(path).ok()?;
    let file_len = f.metadata().ok()?.len();
    let mut head = [0u8; 10];
    f.read_exact(&mut head).ok()?;
    let mut start = 0u64;
    if &head[0..3] == b"ID3" {
        let size = ((head[6] as u64 & 0x7f) << 21)
            | ((head[7] as u64 & 0x7f) << 14)
            | ((head[8] as u64 & 0x7f) << 7)
            | (head[9] as u64 & 0x7f);
        start = 10 + size + if head[5] & 0x10 != 0 { 10 } else { 0 };
    }
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    (&mut f).take(256 * 1024).read_to_end(&mut buf).ok()?;
    let mut tail_tag = 0u64;
    if file_len >= 128 {
        let mut t = [0u8; 3];
        if f.seek(SeekFrom::Start(file_len - 128)).is_ok() && f.read_exact(&mut t).is_ok() && &t == b"TAG" {
            tail_tag = 128;
        }
    }

    // first frame whose successor is also a valid frame header
    let mut i = 0;
    let (pos, fr) = loop {
        if i + 4 > buf.len() {
            return None;
        }
        if let Some(fr) = parse_frame(&buf[i..]) {
            let next = i + fr.len;
            if fr.len > 0 && (next + 4 > buf.len() || parse_frame(&buf[next..]).is_some()) {
                break (i, fr);
            }
        }
        i += 1;
    };
    let spf: u64 = match (fr.layer, fr.mpeg1) {
        (1, _) => 384,
        (3, false) => 576,
        _ => 1152,
    };
    let audio_bytes = file_len.saturating_sub(start + pos as u64 + tail_tag);

    // Xing / Info / VBRI headers give the exact frame count for VBR files.
    let side = match (fr.mpeg1, fr.channels) {
        (true, 1) => 17,
        (true, _) => 32,
        (false, 1) => 9,
        (false, _) => 17,
    };
    let mut frames: Option<u64> = None;
    let x = pos + 4 + side;
    if x + 12 <= buf.len() && (&buf[x..x + 4] == b"Xing" || &buf[x..x + 4] == b"Info") {
        let flags = u32::from_be_bytes([buf[x + 4], buf[x + 5], buf[x + 6], buf[x + 7]]);
        if flags & 1 != 0 {
            frames = Some(u32::from_be_bytes([buf[x + 8], buf[x + 9], buf[x + 10], buf[x + 11]]) as u64);
        }
    }
    let v = pos + 4 + 32;
    if frames.is_none() && v + 18 <= buf.len() && &buf[v..v + 4] == b"VBRI" {
        frames = Some(u32::from_be_bytes([buf[v + 14], buf[v + 15], buf[v + 16], buf[v + 17]]) as u64);
    }
    let (duration, kbps) = match frames {
        Some(n) if n > 0 => {
            let d = n as f64 * spf as f64 / fr.rate as f64;
            (d, ((audio_bytes as f64 * 8.0 / d) / 1000.0).round() as u32)
        }
        _ => (audio_bytes as f64 * 8.0 / (fr.kbps as f64 * 1000.0), fr.kbps),
    };
    Some(Mp3Header { duration, kbps, sample_rate: fr.rate, channels: fr.channels })
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}
