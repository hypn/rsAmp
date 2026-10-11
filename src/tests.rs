//! Behavioural tests driving the app through simulated mouse input (audio runs on the
//! null output when no device is present, so playback timing is real-time).

use crate::app::{App, Hit, Status};
use crate::config::Config;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn write_wav(path: &PathBuf, secs: f32, freq: f32) {
    let rate = 44100u32;
    let n = (rate as f32 * secs) as u32;
    let mut d = Vec::new();
    d.extend_from_slice(b"RIFF");
    d.extend_from_slice(&(36 + n * 4).to_le_bytes());
    d.extend_from_slice(b"WAVEfmt ");
    d.extend_from_slice(&16u32.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&2u16.to_le_bytes());
    d.extend_from_slice(&rate.to_le_bytes());
    d.extend_from_slice(&(rate * 4).to_le_bytes());
    d.extend_from_slice(&4u16.to_le_bytes());
    d.extend_from_slice(&16u16.to_le_bytes());
    d.extend_from_slice(b"data");
    d.extend_from_slice(&(n * 4).to_le_bytes());
    for i in 0..n {
        let s = ((i as f32 / rate as f32 * freq * std::f32::consts::TAU).sin() * 12000.0) as i16;
        d.extend_from_slice(&s.to_le_bytes());
        d.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, d).unwrap();
}

fn fixture(name: &str, lens: &[f32]) -> Vec<PathBuf> {
    let dir = std::env::temp_dir().join(format!("rsamp-test-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    lens.iter()
        .enumerate()
        .map(|(i, &l)| {
            let p = dir.join(format!("{:02} track.wav", i + 1));
            write_wav(&p, l, 220.0 * (i + 1) as f32);
            p
        })
        .collect()
}

fn app_with(files: Vec<PathBuf>) -> App {
    let mut app = App::new(Config::default(), None, Vec::new());
    app.add_paths(files);
    app
}

fn click(app: &mut App, x: i32, y: i32) {
    app.mouse_down(x, y, false, false);
    app.mouse_up(x, y);
}

fn run_for(app: &mut App, ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        app.tick();
        std::thread::sleep(Duration::from_millis(15));
    }
}

#[test]
fn hit_testing_matches_classic_layout() {
    let app = app_with(Vec::new());
    assert_eq!(app.hit_test(20, 95), Hit::CButton(0));
    assert_eq!(app.hit_test(45, 95), Hit::CButton(1));
    assert_eq!(app.hit_test(140, 95), Hit::CButton(5));
    assert_eq!(app.hit_test(257, 6), Hit::MShade);
    assert_eq!(app.hit_test(267, 6), Hit::MClose);
    assert_eq!(app.hit_test(120, 62), Hit::Volume);
    assert_eq!(app.hit_test(190, 62), Hit::Balance);
    assert_eq!(app.hit_test(170, 95), Hit::Shuffle);
    assert_eq!(app.hit_test(13, 22 + 12), Hit::Clutter(1));
    // equalizer is docked below the main window
    assert_eq!(app.hit_test(20, 116 + 22), Hit::EOn);
    assert_eq!(app.hit_test(80, 116 + 60), Hit::ESlider(1));
    // playlist below that
    assert_eq!(app.hit_test(50, 232 + 30), Hit::PList);
    assert_eq!(app.hit_test(20, 232 + 116 - 20), Hit::PMenuBtn(0));
}

#[test]
fn transport_and_track_advance() {
    let files = fixture("transport", &[0.6, 0.6, 3.0]);
    let mut app = app_with(files);
    click(&mut app, 45, 95); // play
    assert_eq!(app.status, Status::Playing);
    assert_eq!(app.pl.current, Some(0));
    run_for(&mut app, 1500);
    assert_eq!(app.pl.current, Some(2), "should have advanced through two short tracks");
    assert!(app.position() > 0.1);
    click(&mut app, 65, 95); // pause
    assert_eq!(app.status, Status::Paused);
    let p = app.position();
    run_for(&mut app, 300);
    assert!((app.position() - p).abs() < 0.02, "position frozen while paused");
    click(&mut app, 65, 95); // unpause
    assert_eq!(app.status, Status::Playing);
    click(&mut app, 65, 95); // pause again
    let p = app.position();
    click(&mut app, 45, 95); // play resumes rather than restarting
    assert_eq!(app.status, Status::Playing);
    assert_eq!(app.pl.current, Some(2));
    assert!((app.position() - p).abs() < 0.02, "play should resume, not restart");
    click(&mut app, 20, 95); // previous
    assert_eq!(app.pl.current, Some(1));
    click(&mut app, 90, 95); // stop
    assert_eq!(app.status, Status::Stopped);
}

#[test]
fn seek_with_position_bar() {
    let files = fixture("seek", &[10.0]);
    let mut app = app_with(files);
    app.play_index(0);
    run_for(&mut app, 200);
    // drag the thumb to the middle of the bar and release
    let mid = 16 + 14 + (248 - 29) / 2;
    app.mouse_down(30, 77, false, false);
    app.mouse_move(mid, 77);
    assert!(app.slider_text.as_deref().unwrap_or("").starts_with("SEEK TO:"));
    app.mouse_up(mid, 77);
    run_for(&mut app, 200);
    let pos = app.position();
    assert!((4.5..6.0).contains(&pos), "seeked position {pos}");
    assert!(app.slider_text.is_none());
}

#[test]
fn window_shade_and_toggles() {
    let mut app = app_with(Vec::new());
    let h = app.layout().h;
    click(&mut app, 257, 6);
    assert!(app.cfg.main_shade);
    assert_eq!(app.layout().main.h, 14);
    assert_eq!(app.layout().h, h - 102);
    click(&mut app, 257, 6);
    assert!(!app.cfg.main_shade);
    // EQ and PL buttons hide/show the docked windows
    click(&mut app, 225, 62);
    assert!(!app.cfg.eq_visible);
    click(&mut app, 248, 62);
    assert!(!app.cfg.pl_visible);
    assert_eq!(app.layout().h, 116);
    // clutter bar "A" = always on top, "D" = double size
    click(&mut app, 13, 22 + 12);
    assert!(app.cfg.always_on_top);
    click(&mut app, 13, 22 + 27);
    assert!(app.cfg.double_size);
}

#[test]
fn sliders_and_equalizer() {
    let mut app = app_with(Vec::new());
    // volume all the way left then right
    app.mouse_down(108, 62, false, false);
    app.mouse_up(108, 62);
    assert_eq!(app.cfg.volume, 0.0);
    app.mouse_down(174, 62, false, false);
    assert!(app.slider_text.as_deref() == Some("VOLUME: 100%"));
    app.mouse_up(174, 62);
    assert_eq!(app.cfg.volume, 1.0);
    // balance snaps to centre
    click(&mut app, 177 + 19, 62);
    assert_eq!(app.cfg.balance, 0.0);
    // EQ: turn on, drag the 60Hz slider to the top
    click(&mut app, 20, 116 + 22);
    assert!(app.eq.on);
    app.mouse_down(84, 116 + 70, false, false);
    app.mouse_move(84, 116 + 30);
    app.mouse_up(84, 116 + 30);
    assert_eq!(app.eq.bands[0], 12.0);
    // presets cycle
    click(&mut app, 230, 116 + 22);
    assert_ne!(app.eq.bands[0], 12.0);
}

#[test]
fn playlist_selection_menus_and_resize() {
    let files = fixture("playlist", &[0.2, 0.2, 0.2, 0.2]);
    let mut app = app_with(files);
    let top = app.layout().pl.unwrap().y;
    let row_y = |r: i32| top + 20 + r * 13 + 6;
    click(&mut app, 50, row_y(1));
    assert!(app.pl.entries[1].selected);
    app.mouse_down(50, row_y(3), false, true); // shift-click range
    app.mouse_up(50, row_y(3));
    assert_eq!(app.pl.entries.iter().filter(|e| e.selected).count(), 3);
    // REM button opens its menu; "remove selected" is the bottom item
    let ph = app.layout().pl.unwrap().h;
    let (bx, by) = (43 + 5, top + ph - 30 + 5);
    app.mouse_down(bx, by, false, false);
    assert_eq!(app.menu_open, Some(1));
    app.mouse_up(bx, by); // stays open after click
    assert_eq!(app.menu_open, Some(1));
    click(&mut app, bx, by); // bottom item == button position
    assert_eq!(app.menu_open, None);
    assert_eq!(app.pl.len(), 1);
    // double-click plays
    click(&mut app, 50, row_y(0));
    click(&mut app, 50, row_y(0));
    assert_eq!(app.status, Status::Playing);
    // resize from the bottom-right corner by two width steps and one height step
    let w = app.cfg.pl_w;
    let (cx, cy) = (w - 5, top + ph - 5);
    app.mouse_down(cx, cy, false, false);
    app.mouse_move(cx + 50, cy + 29);
    app.mouse_up(cx + 50, cy + 29);
    assert_eq!(app.cfg.pl_w, w + 50);
    assert_eq!(app.cfg.pl_h, ph + 29);
}

#[test]
fn mp3_header_parsing() {
    // A minimal CBR MPEG-1 Layer III stream: 128 kbps, 44.1 kHz, joint stereo.
    let dir = std::env::temp_dir().join(format!("rsamp-mp3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("cbr.mp3");
    let frame_len = 144 * 128_000 / 44_100; // 417
    let mut data = Vec::new();
    for _ in 0..100 {
        let mut f = vec![0u8; frame_len];
        f[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0x40]);
        data.extend(f);
    }
    std::fs::write(&p, &data).unwrap();
    let h = crate::media::mp3_header_info(&p).expect("header");
    assert_eq!(h.kbps, 128);
    assert_eq!(h.sample_rate, 44100);
    assert_eq!(h.channels, 2);
    assert!((h.duration - 100.0 * 1152.0 / 44100.0).abs() < 0.05, "{}", h.duration);
}

#[test]
fn skin_loads_from_directory_and_zip() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("base-2.91.wsz");
    if !dir.exists() {
        return;
    }
    let s = crate::skin::Skin::load(&dir).expect("dir skin");
    assert_eq!((s.main.w, s.main.h), (275, 116));
    assert_eq!(s.pl.normal, 0x00FF00);
    // zip it and load again
    let zp = std::env::temp_dir().join(format!("rsamp-skin-{}.wsz", std::process::id()));
    {
        let f = std::fs::File::create(&zp).unwrap();
        let mut z = zip::ZipWriter::new(f);
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            z.start_file(format!("Base/{name}"), zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut z, &std::fs::read(e.path()).unwrap()).unwrap();
        }
        z.finish().unwrap();
    }
    let s2 = crate::skin::Skin::load(&zp).expect("zip skin");
    assert_eq!(s2.main.px, s.main.px);
    assert!(s2.eq_ex.is_some());
}

#[test]
fn mp3_decode_and_seek() {
    // Set RSAMP_TEST_MP3 to a real .mp3 (>= 20 s) to exercise the decoder end to end.
    let Some(p) = std::env::var_os("RSAMP_TEST_MP3").map(PathBuf::from) else { return };
    let mut app = app_with(vec![p]);
    app.play_index(0);
    run_for(&mut app, 400);
    assert!(app.position() > 0.2, "mp3 should be playing");
    app.seek_to(12.0);
    run_for(&mut app, 300);
    let pos = app.position();
    assert!((11.5..13.0).contains(&pos), "seeked mp3 position {pos}");
    assert_eq!(app.status, Status::Playing);
}

#[test]
fn zoom_levels() {
    let mut app = app_with(Vec::new());
    assert_eq!(app.cfg.zoom, 100);
    app.zoom_by(1);
    assert_eq!(app.cfg.zoom, 110);
    app.zoom_by(-1);
    for _ in 0..10 {
        app.zoom_by(-1);
    }
    assert_eq!(app.cfg.zoom, 50, "never below 50%");
    for _ in 0..200 {
        app.zoom_by(1);
    }
    assert_eq!(app.cfg.zoom, 1000, "never above 10x");
    assert!(app.cfg.double_size);
    app.toggle_double();
    assert_eq!(app.cfg.zoom, 100);
    app.set_zoom(333);
    assert_eq!(app.cfg.zoom, 330, "snaps to whole steps");
}

#[test]
fn area_scaling() {
    use crate::gfx::{scale_into, scaled_len, Image};
    let mut img = Image::new(3, 1, 0);
    img.set(0, 0, 0xff0000);
    img.set(2, 0, 0x0000ff);
    let run = |f: f64| {
        let (w, h) = (scaled_len(3, f), scaled_len(1, f));
        let mut out = vec![0; (w * h) as usize];
        scale_into(&img, f, &mut out, w, h);
        out
    };
    // whole factors duplicate pixels exactly
    assert_eq!(run(2.0), [0xff0000, 0xff0000, 0, 0, 0x0000ff, 0x0000ff].repeat(2));
    // 50% averages pairs
    assert_eq!(run(0.5)[0], 0x800000);
    // 150%: every source pixel covers 1.5 output pixels; edges blend, flat areas stay exact
    let out = run(1.5);
    assert_eq!(out.len(), 5 * 2);
    assert_eq!(out[0], 0xff0000);
    assert_eq!(out[1], 0x800000);
    assert_eq!(out[4], 0x0000ff);
}

#[test]
fn xspf_playlists() {
    use crate::playlist::{file_uri_to_path, Playlist};
    let files = fixture("xspf", &[0.1, 0.1, 0.1]);
    let dir = files[0].parent().unwrap();
    // absolute file:/// URI with %-escapes, a relative location, and tracks VLC can't play here
    let abs = format!("file://{}", files[0].to_string_lossy().replace(' ', "%20"));
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<playlist xmlns="http://xspf.org/ns/0/" xmlns:vlc="http://www.videolan.org/vlc/playlist/ns/0/" version="1">
	<title>Playlist</title>
	<trackList>
		<track>
			<location>{abs}</location>
			<title>Hate/Love &amp; More</title>
			<creator>Electric Callboy</creator>
			<duration>188056</duration>
			<extension application="http://www.videolan.org/vlc/playlist/0"><vlc:id>0</vlc:id></extension>
		</track>
		<track><location>02%20track.wav</location></track>
		<track><location>http://radio.example/stream</location><title>radio</title></track>
		<track><title>no location</title></track>
	</trackList>
</playlist>"#
    );
    let pl_path = dir.join("list.xspf");
    std::fs::write(&pl_path, xml).unwrap();
    let mut pl = Playlist::default();
    assert_eq!(pl.add([pl_path]), 2, "streams and entries without a location are skipped");
    assert_eq!(pl.entries[0].path, files[0]);
    assert_eq!(pl.entries[0].title, "Electric Callboy - Hate/Love & More");
    assert_eq!(pl.entries[0].duration, Some(188.056));
    assert_eq!(pl.entries[1].path, files[1]);

    // file URI forms
    let p = |s: &str| file_uri_to_path(s).map(|p| p.to_string_lossy().into_owned());
    assert_eq!(p("http://x/y.mp3"), None);
    if cfg!(windows) {
        assert_eq!(p("file:///C:/Music/a%20b.mp3").as_deref(), Some(r"C:\Music\a b.mp3"));
        assert_eq!(p("file://192.168.42.52/d/MP3s/x.mp3").as_deref(), Some(r"\\192.168.42.52\d\MP3s\x.mp3"));
    } else {
        assert_eq!(p("file:///home/u/a%20b%C3%A9.mp3").as_deref(), Some("/home/u/a bé.mp3"));
        assert_eq!(p("file://localhost/home/u/x.mp3").as_deref(), Some("/home/u/x.mp3"));
        assert_eq!(p("file://192.168.42.52/d/MP3s/x.mp3").as_deref(), Some("//192.168.42.52/d/MP3s/x.mp3"));
    }
}
