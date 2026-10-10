#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod audio;
mod bmp;
mod config;
mod default_skin;
mod eq;
mod font;
mod gfx;
mod media;
mod playlist;
mod render;
mod skin;
mod sprites;
mod vis;
#[cfg(test)]
mod tests;

use app::{App, WinReq};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId, WindowLevel};

const FRAME: Duration = Duration::from_millis(33);

struct Gui {
    app: App,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    /// physical pixels per skin pixel (zoom × display scaling)
    factor: f64,
    /// partial Ctrl+wheel movement not yet turned into a zoom step (touchpads)
    zoom_acc: f32,
    /// the scaled frame, reused between redraws (playlist text is drawn onto it)
    scaled: gfx::Image,
    cursor: (f64, f64),
    mods: ModifiersState,
    dropped: Vec<PathBuf>,
    next_frame: Instant,
    last_save: Instant,
}

impl Gui {
    fn factor_for(&self, window: &Window) -> f64 {
        window.scale_factor().max(0.5) * self.app.cfg.zoom as f64 / 100.0
    }

    fn logical_cursor(&self) -> (i32, i32) {
        let s = self.factor;
        ((self.cursor.0 / s).floor() as i32, (self.cursor.1 / s).floor() as i32)
    }

    fn apply_size(&mut self) {
        let Some(w) = self.window.clone() else { return };
        self.factor = self.factor_for(&w);
        self.app.text_scale = self.factor;
        let l = self.app.layout();
        let size = PhysicalSize::new(gfx::scaled_len(l.w, self.factor) as u32, gfx::scaled_len(l.h, self.factor) as u32);
        let _ = w.request_inner_size(size);
        w.request_redraw();
    }

    /// Windows sends no mouse moves while files are dragged in from Explorer, so the last known
    /// cursor position can be stale (e.g. over the playlist) when they are dropped. Ask the OS
    /// where the cursor really is so the drop goes to the right window.
    fn update_cursor_for_drop(&mut self) {
        #[cfg(windows)]
        if let Some(w) = &self.window {
            let mut pt = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
            // SAFETY: GetCursorPos only writes to the POINT we pass.
            let ok = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt) } != 0;
            if let (true, Ok(origin)) = (ok, w.inner_position()) {
                self.cursor = ((pt.x - origin.x) as f64, (pt.y - origin.y) as f64);
            }
        }
    }

    fn handle_requests(&mut self, el: &ActiveEventLoop) {
        let reqs: Vec<WinReq> = self.app.requests.drain(..).collect();
        for r in reqs {
            match r {
                WinReq::DragWindow => {
                    if let Some(w) = &self.window {
                        let _ = w.drag_window();
                    }
                    // the button release is usually swallowed by the OS move loop
                    self.app.pressed = None;
                }
                WinReq::Minimize => {
                    if let Some(w) = &self.window {
                        w.set_minimized(true);
                    }
                }
                WinReq::OnTop(on) => {
                    if let Some(w) = &self.window {
                        w.set_window_level(if on { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal });
                    }
                }
                WinReq::Relayout => self.apply_size(),
                WinReq::Quit => self.quit(el),
            }
        }
    }

    fn quit(&mut self, el: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            if let Ok(p) = w.outer_position() {
                self.app.cfg.window_pos = Some((p.x, p.y));
            }
        }
        self.app.save_config();
        self.app.stop();
        el.exit();
    }

    fn redraw(&mut self) {
        let (Some(window), Some(surface)) = (self.window.clone(), self.surface.as_mut()) else { return };
        let frame = self.app.render();
        let size = window.inner_size();
        let (bw, bh) = (size.width.max(1), size.height.max(1));
        if surface.resize(NonZeroU32::new(bw).unwrap(), NonZeroU32::new(bh).unwrap()).is_err() {
            return;
        }
        let Ok(mut buf) = surface.buffer_mut() else { return };
        let (bw, bh) = (bw as i32, bh as i32);
        if self.app.text_scale == 1.0 {
            gfx::scale_into(&frame, self.factor, &mut buf, bw, bh);
        } else {
            if (self.scaled.w, self.scaled.h) != (bw, bh) {
                self.scaled = gfx::Image::new(bw, bh, 0);
            }
            gfx::scale_into(&frame, self.factor, &mut self.scaled.px, bw, bh);
            self.app.draw_zoomed_text(&mut self.scaled);
            buf.copy_from_slice(&self.scaled.px);
        }
        window.pre_present_notify();
        let _ = buf.present();
    }
}

impl ApplicationHandler for Gui {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let l = self.app.layout();
        let mut attrs = Window::default_attributes()
            .with_title("rsAmp")
            .with_decorations(false)
            .with_resizable(false)
            .with_inner_size(PhysicalSize::new(l.w as u32, l.h as u32));
        if let Some((x, y)) = self.app.cfg.window_pos {
            attrs = attrs.with_position(PhysicalPosition::new(x, y));
        }
        if self.app.cfg.always_on_top {
            attrs = attrs.with_window_level(WindowLevel::AlwaysOnTop);
        }
        attrs = attrs.with_window_icon(window_icon());
        let window = match el.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                eprintln!("rsAmp: cannot create window: {e}");
                el.exit();
                return;
            }
        };
        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("rsAmp: no framebuffer: {e}");
                el.exit();
                return;
            }
        };
        self.surface = softbuffer::Surface::new(&context, window.clone()).ok();
        self.window = Some(window);
        self.apply_size();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.quit(el),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(_) => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => self.apply_size(),
            WindowEvent::Focused(f) => self.app.focused = f,
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                let (x, y) = self.logical_cursor();
                self.app.mouse_move(x, y);
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let (x, y) = self.logical_cursor();
                match state {
                    ElementState::Pressed => {
                        self.app.mouse_down(x, y, self.mods.control_key() || self.mods.super_key(), self.mods.shift_key())
                    }
                    ElementState::Released => self.app.mouse_up(x, y),
                }
                self.handle_requests(el);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 20.0) as f32,
                };
                if self.mods.control_key() || self.mods.super_key() {
                    // Ctrl+wheel zooms one step per notch
                    self.zoom_acc += lines;
                    let steps = self.zoom_acc.trunc() as i32;
                    if steps != 0 {
                        self.zoom_acc = 0.0;
                        self.app.zoom_by(steps.signum());
                        self.handle_requests(el);
                    }
                    return;
                }
                self.zoom_acc = 0.0;
                let (x, y) = self.logical_cursor();
                self.app.wheel(x, y, lines);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let m = self.mods;
                self.app.key(&event.logical_key, m.control_key() || m.super_key(), m.alt_key(), m.shift_key());
                self.handle_requests(el);
            }
            WindowEvent::DroppedFile(p) => {
                if self.dropped.is_empty() {
                    self.update_cursor_for_drop();
                }
                self.dropped.push(p);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if !self.dropped.is_empty() {
            let files = std::mem::take(&mut self.dropped);
            let (x, y) = self.logical_cursor();
            self.app.drop_files(files, x, y);
        }
        let now = Instant::now();
        if now >= self.next_frame {
            self.app.tick();
            self.handle_requests(el);
            if let Some(w) = &self.window {
                w.request_redraw();
            }
            self.next_frame = now + FRAME;
        }
        if self.app.dirty_config && self.last_save.elapsed() > Duration::from_secs(5) {
            self.app.save_config();
            self.last_save = Instant::now();
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

/// Taskbar / title bar icon (the .exe's own icon is embedded by build.rs).
fn window_icon() -> Option<winit::window::Icon> {
    let img = image::load_from_memory(include_bytes!("../assets/rsamp-icon.png")).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    winit::window::Icon::from_rgba(img.into_raw(), w, h).ok()
}

fn usage() {
    println!(
        "rsAmp - a classic skinnable music player\n\n\
         usage: rsAmp [--skin <file.wsz|dir>] [files/dirs/playlists...]\n\
         \x20      rsAmp --screenshot <out.png> [--skin ...] [files...]   render one frame and exit"
    );
}

fn main() {
    let mut skin = None;
    let mut files = Vec::new();
    let mut screenshot: Option<PathBuf> = None;
    let mut zoom: Option<u32> = None;
    let mut wait_ms = 600;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--skin") => skin = args.next().map(PathBuf::from),
            Some("--screenshot") => screenshot = args.next().map(PathBuf::from),
            Some("--zoom") => zoom = args.next().and_then(|z| z.to_str()?.trim_end_matches('%').parse().ok()),
            Some("--wait") => wait_ms = args.next().and_then(|z| z.to_str()?.parse().ok()).unwrap_or(600),
            Some("--probe") => {
                for f in args.by_ref() {
                    println!("{}: {:?}", PathBuf::from(&f).display(), media::probe(&PathBuf::from(&f)));
                }
                return;
            }
            Some("-h") | Some("--help") => return usage(),
            _ => {
                // a skin given as a plain argument ("Open with", drag onto the exe) selects it
                let p = PathBuf::from(a);
                if skin::is_skin_path(&p) {
                    skin = Some(p);
                } else {
                    files.push(p);
                }
            }
        }
    }
    let cfg = config::Config::load();
    let mut app = App::new(cfg, skin, files);

    if let Some(out) = screenshot {
        // headless render for testing skins
        std::thread::sleep(Duration::from_millis(wait_ms));
        for _ in 0..10 {
            app.tick();
            std::thread::sleep(Duration::from_millis(30));
        }
        let f = config::snap_zoom(zoom.unwrap_or(app.cfg.zoom)) as f64 / 100.0;
        app.text_scale = f;
        let img = app.render();
        let mut big = gfx::Image::new(gfx::scaled_len(img.w, f), gfx::scaled_len(img.h, f), 0);
        let (bw, bh) = (big.w, big.h);
        gfx::scale_into(&img, f, &mut big.px, bw, bh);
        if f != 1.0 {
            app.draw_zoomed_text(&mut big);
        }
        if let Err(e) = image::save_buffer(&out, &big.to_rgba(), big.w as u32, big.h as u32, image::ColorType::Rgba8) {
            eprintln!("rsAmp: {e}");
        }
        return;
    }

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("rsAmp: cannot open display: {e}");
            std::process::exit(1);
        }
    };
    let mut gui = Gui {
        app,
        window: None,
        surface: None,
        factor: 1.0,
        zoom_acc: 0.0,
        scaled: gfx::Image::new(1, 1, 0),
        cursor: (0.0, 0.0),
        mods: ModifiersState::empty(),
        dropped: Vec::new(),
        next_frame: Instant::now(),
        last_save: Instant::now(),
    };
    if let Err(e) = event_loop.run_app(&mut gui) {
        eprintln!("rsAmp: {e}");
    }
}
