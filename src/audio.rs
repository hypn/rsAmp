//! Audio output: a decoder thread (symphonia) fills a sample queue that the cpal output
//! callback drains, applying the equalizer, balance and volume and feeding the visualizer.

use crate::eq::Equalizer;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

pub const VIS_LEN: usize = 2048;

struct Queue {
    samples: VecDeque<f32>, // interleaved stereo at output rate
    /// Output-rate frames of the current track already played.
    pos_frames: u64,
    gen: u64,
    eof: bool,
    active: bool,
}

#[derive(Clone, Copy, Default)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub duration: Option<f64>,
}

struct Shared {
    q: Mutex<Queue>,
    paused: AtomicBool,
    volume: AtomicU32,
    balance: AtomicU32,
    eq: Mutex<Equalizer>,
    eq_version: AtomicU64,
    vis: Mutex<VecDeque<f32>>,
    finished: AtomicBool,
    info: Mutex<Option<StreamInfo>>,
    error: Mutex<Option<String>>,
}

enum Cmd {
    Seek(f64),
    Stop,
}

pub struct Audio {
    shared: Arc<Shared>,
    _stream: Option<cpal::Stream>,
    out_rate: u32,
    decoder: Option<Sender<Cmd>>,
    gen: u64,
}

fn f2u(v: f32) -> u32 {
    v.to_bits()
}
fn u2f(v: u32) -> f32 {
    f32::from_bits(v)
}

impl Audio {
    pub fn new() -> Audio {
        let shared = Arc::new(Shared {
            q: Mutex::new(Queue { samples: VecDeque::new(), pos_frames: 0, gen: 0, eof: false, active: false }),
            paused: AtomicBool::new(false),
            volume: AtomicU32::new(f2u(0.75)),
            balance: AtomicU32::new(f2u(0.0)),
            eq: Mutex::new(Equalizer::default()),
            eq_version: AtomicU64::new(1),
            vis: Mutex::new(VecDeque::from(vec![0.0; VIS_LEN])),
            finished: AtomicBool::new(false),
            info: Mutex::new(None),
            error: Mutex::new(None),
        });
        let (stream, out_rate) = match open_stream(shared.clone()) {
            Ok(v) => (Some(v.0), v.1),
            Err(e) => {
                eprintln!("rsAmp: audio output unavailable ({e}); playing silently");
                spawn_null_output(shared.clone(), 44100);
                (None, 44100)
            }
        };
        Audio { shared, _stream: stream, out_rate, decoder: None, gen: 0 }
    }

    /// Start playing `path` from `start` seconds.
    pub fn play(&mut self, path: PathBuf, start: f64) {
        self.stop();
        self.gen += 1;
        let gen = self.gen;
        {
            let mut q = self.shared.q.lock().unwrap();
            q.samples.clear();
            q.pos_frames = (start * self.out_rate as f64) as u64;
            q.gen = gen;
            q.eof = false;
            q.active = true;
        }
        *self.shared.info.lock().unwrap() = None;
        *self.shared.error.lock().unwrap() = None;
        self.shared.finished.store(false, Ordering::SeqCst);
        self.shared.paused.store(false, Ordering::SeqCst);
        let (tx, rx) = channel();
        let shared = self.shared.clone();
        let out_rate = self.out_rate;
        std::thread::Builder::new()
            .name("decoder".into())
            .spawn(move || {
                if let Err(e) = decode_thread(&path, &shared, gen, out_rate, rx, start) {
                    eprintln!("rsAmp: {}: {e}", path.display());
                    *shared.error.lock().unwrap() = Some(e);
                    let mut q = shared.q.lock().unwrap();
                    if q.gen == gen {
                        q.eof = true;
                    }
                }
            })
            .ok();
        self.decoder = Some(tx);
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.decoder.take() {
            let _ = tx.send(Cmd::Stop);
        }
        let mut q = self.shared.q.lock().unwrap();
        q.samples.clear();
        q.active = false;
        q.eof = false;
        q.pos_frames = 0;
        q.gen = q.gen.wrapping_add(1_000_000);
        drop(q);
        *self.shared.info.lock().unwrap() = None;
        self.shared.finished.store(false, Ordering::SeqCst);
    }

    pub fn seek(&mut self, secs: f64) {
        if let Some(tx) = &self.decoder {
            let _ = tx.send(Cmd::Seek(secs.max(0.0)));
            // Update the reported position immediately so the UI does not jump back.
            let mut q = self.shared.q.lock().unwrap();
            q.samples.clear();
            q.pos_frames = (secs.max(0.0) * self.out_rate as f64) as u64;
        }
    }

    pub fn set_paused(&self, p: bool) {
        self.shared.paused.store(p, Ordering::SeqCst);
    }

    pub fn is_active(&self) -> bool {
        self.shared.q.lock().unwrap().active
    }

    pub fn position(&self) -> f64 {
        self.shared.q.lock().unwrap().pos_frames as f64 / self.out_rate as f64
    }

    pub fn info(&self) -> Option<StreamInfo> {
        *self.shared.info.lock().unwrap()
    }

    pub fn take_error(&self) -> Option<String> {
        self.shared.error.lock().unwrap().take()
    }

    /// True once after the current track has played out completely.
    pub fn take_finished(&self) -> bool {
        self.shared.finished.swap(false, Ordering::SeqCst)
    }

    /// 0..=1
    pub fn set_volume(&self, v: f32) {
        self.shared.volume.store(f2u(v.clamp(0.0, 1.0)), Ordering::Relaxed);
    }

    /// -1 (left) ..= 1 (right)
    pub fn set_balance(&self, b: f32) {
        self.shared.balance.store(f2u(b.clamp(-1.0, 1.0)), Ordering::Relaxed);
    }

    pub fn set_eq(&self, eq: &Equalizer) {
        *self.shared.eq.lock().unwrap() = eq.clone();
        self.shared.eq_version.fetch_add(1, Ordering::SeqCst);
    }

    /// Most recent mono samples that reached the output.
    pub fn vis_samples(&self, out: &mut Vec<f32>) {
        let v = self.shared.vis.lock().unwrap();
        out.clear();
        out.extend(v.iter().copied());
    }

    pub fn output_rate(&self) -> u32 {
        self.out_rate
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop();
    }
}

fn open_stream(shared: Arc<Shared>) -> Result<(cpal::Stream, u32), String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no output device")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate.0;
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, shared),
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, shared),
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, shared),
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, shared),
        cpal::SampleFormat::U8 => build::<u8>(&device, &config, shared),
        cpal::SampleFormat::F64 => build::<f64>(&device, &config, shared),
        other => return Err(format!("unsupported sample format {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, rate))
}

/// Pulls samples from the queue and applies EQ, balance and volume. Shared by the
/// cpal callback and the null-output fallback.
struct Mixer {
    shared: Arc<Shared>,
    eq: crate::eq::EqProcessor,
    eq_version: u64,
    vis_tmp: Vec<f32>,
}

impl Mixer {
    fn new(shared: Arc<Shared>, rate: u32) -> Self {
        Mixer { shared, eq: crate::eq::EqProcessor::new(rate as f32), eq_version: 0, vis_tmp: Vec::with_capacity(4096) }
    }

    /// Fill `frames` stereo frames into `out` (interleaved L/R).
    fn fill(&mut self, out: &mut [[f32; 2]]) {
        let shared = &*self.shared;
        let ver = shared.eq_version.load(Ordering::Relaxed);
        if ver != self.eq_version {
            if let Ok(eq) = shared.eq.try_lock() {
                self.eq.configure(&eq);
                self.eq_version = ver;
            }
        }
        let vol = u2f(shared.volume.load(Ordering::Relaxed));
        let gain = vol * vol; // perceptual-ish curve
        let bal = u2f(shared.balance.load(Ordering::Relaxed));
        let (gl, gr) = (gain * (1.0 - bal).min(1.0), gain * (1.0 + bal).min(1.0));
        let paused = shared.paused.load(Ordering::Relaxed);
        self.vis_tmp.clear();
        let mut q = shared.q.lock().unwrap();
        let active = q.active;
        for frame in out.iter_mut() {
            let (mut l, mut r) = (0.0f32, 0.0f32);
            if q.active && !paused {
                match (q.samples.pop_front(), q.samples.pop_front()) {
                    (Some(a), Some(b)) => {
                        (l, r) = self.eq.process(a, b);
                        q.pos_frames += 1;
                        self.vis_tmp.push((l + r) * 0.5);
                    }
                    _ => {
                        if q.eof {
                            q.active = false;
                            shared.finished.store(true, Ordering::SeqCst);
                        }
                    }
                }
            }
            *frame = [(l * gl).clamp(-1.0, 1.0), (r * gr).clamp(-1.0, 1.0)];
        }
        drop(q);
        if let Ok(mut v) = shared.vis.try_lock() {
            if !self.vis_tmp.is_empty() {
                v.extend(self.vis_tmp.iter().copied());
                while v.len() > VIS_LEN {
                    v.pop_front();
                }
            } else if paused || !active {
                // decay towards silence so the analyzer falls when stopped
                for s in v.iter_mut() {
                    *s *= 0.5;
                }
            }
        }
    }
}

fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig, shared: Arc<Shared>) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let mut mixer = Mixer::new(shared, config.sample_rate.0);
    let mut tmp: Vec<[f32; 2]> = Vec::new();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let n = data.len() / channels.max(1);
                tmp.resize(n, [0.0; 2]);
                mixer.fill(&mut tmp);
                for (frame, s) in data.chunks_mut(channels).zip(tmp.iter()) {
                    if channels == 1 {
                        frame[0] = T::from_sample((s[0] + s[1]) * 0.5);
                        continue;
                    }
                    for (i, out) in frame.iter_mut().enumerate() {
                        *out = T::from_sample(if i < 2 { s[i] } else { 0.0 });
                    }
                }
            },
            |e| eprintln!("rsAmp: audio stream error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

/// Without an output device, consume audio in real time so playback still progresses.
fn spawn_null_output(shared: Arc<Shared>, rate: u32) {
    std::thread::Builder::new()
        .name("null-audio".into())
        .spawn(move || {
            let mut mixer = Mixer::new(shared, rate);
            let chunk = (rate / 100) as usize;
            let mut buf = vec![[0.0f32; 2]; chunk];
            let start = std::time::Instant::now();
            let mut done: u64 = 0;
            loop {
                mixer.fill(&mut buf);
                done += chunk as u64;
                let due = Duration::from_secs_f64(done as f64 / rate as f64);
                if let Some(wait) = due.checked_sub(start.elapsed()) {
                    std::thread::sleep(wait);
                }
            }
        })
        .ok();
}

/// Linear-interpolating stereo resampler.
struct Resampler {
    step: f64,
    t: f64,
    buf: Vec<[f32; 2]>,
}

impl Resampler {
    fn new(in_rate: u32, out_rate: u32) -> Self {
        Resampler { step: in_rate as f64 / out_rate as f64, t: 0.0, buf: Vec::new() }
    }
    fn reset(&mut self) {
        self.t = 0.0;
        self.buf.clear();
    }
    fn process(&mut self, input: &[[f32; 2]], out: &mut Vec<f32>) {
        if (self.step - 1.0).abs() < 1e-9 {
            for f in input {
                out.extend_from_slice(f);
            }
            return;
        }
        self.buf.extend_from_slice(input);
        while self.t + 1.0 < self.buf.len() as f64 {
            let i = self.t as usize;
            let frac = (self.t - i as f64) as f32;
            let a = self.buf[i];
            let b = self.buf[i + 1];
            out.push(a[0] + (b[0] - a[0]) * frac);
            out.push(a[1] + (b[1] - a[1]) * frac);
            self.t += self.step;
        }
        let consumed = (self.t as usize).min(self.buf.len().saturating_sub(1));
        self.buf.drain(..consumed);
        self.t -= consumed as f64;
    }
}

fn decode_thread(
    path: &PathBuf,
    shared: &Shared,
    gen: u64,
    out_rate: u32,
    rx: Receiver<Cmd>,
    start: f64,
) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
        .map_err(|e| format!("unsupported format: {e}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("no audio track")?
        .clone();
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("unsupported codec: {e}"))?;

    let mut in_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count() as u32).unwrap_or(2);
    let duration = track.codec_params.n_frames.map(|n| n as f64 / in_rate as f64);
    *shared.info.lock().unwrap() = Some(StreamInfo { sample_rate: in_rate, channels, duration });

    let mut rs = Resampler::new(in_rate, out_rate);
    let mut sbuf: Option<SampleBuffer<f32>> = None;
    let mut frames: Vec<[f32; 2]> = Vec::new();
    let mut out: Vec<f32> = Vec::new();
    let target = (out_rate as usize) * 2 / 2; // ~0.5 s of stereo samples
    let mut eof = false;

    let do_seek = |format: &mut Box<dyn symphonia::core::formats::FormatReader>, secs: f64| -> Option<f64> {
        let t = Time::new(secs as u64, secs.fract());
        match format.seek(SeekMode::Coarse, SeekTo::Time { time: t, track_id: Some(track_id) }) {
            Ok(s) => Some(s.actual_ts as f64),
            Err(_) => None,
        }
    };

    if start > 0.0 {
        if let Some(ts) = do_seek(&mut format, start) {
            decoder.reset();
            let mut q = shared.q.lock().unwrap();
            if q.gen == gen {
                q.pos_frames = (ts / in_rate as f64 * out_rate as f64) as u64;
            }
        }
    }

    loop {
        // commands
        let cmd = if eof {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(c) => Some(c),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
        } else {
            rx.try_recv().ok()
        };
        match cmd {
            Some(Cmd::Stop) => return Ok(()),
            Some(Cmd::Seek(secs)) => {
                let ts = do_seek(&mut format, secs);
                decoder.reset();
                rs.reset();
                eof = false;
                let mut q = shared.q.lock().unwrap();
                if q.gen != gen {
                    return Ok(());
                }
                q.samples.clear();
                q.eof = false;
                q.active = true;
                if let Some(ts) = ts {
                    q.pos_frames = (ts / in_rate as f64 * out_rate as f64) as u64;
                }
                continue;
            }
            None => {}
        }
        if eof {
            continue;
        }
        {
            let q = shared.q.lock().unwrap();
            if q.gen != gen {
                return Ok(());
            }
            if q.samples.len() > target {
                drop(q);
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                eof = true;
                let mut q = shared.q.lock().unwrap();
                if q.gen == gen {
                    q.eof = true;
                }
                continue;
            }
            Err(SymError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(e) => {
                eof = true;
                eprintln!("rsAmp: read error: {e}");
                let mut q = shared.q.lock().unwrap();
                if q.gen == gen {
                    q.eof = true;
                }
                continue;
            }
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        };
        let spec = *decoded.spec();
        if spec.rate != in_rate {
            in_rate = spec.rate;
            rs = Resampler::new(in_rate, out_rate);
            if let Some(info) = shared.info.lock().unwrap().as_mut() {
                info.sample_rate = in_rate;
            }
        }
        let ch = spec.channels.count().max(1);
        let need = decoded.capacity() as u64;
        if sbuf.as_ref().map(|b| b.capacity() < need as usize * ch).unwrap_or(true) {
            sbuf = Some(SampleBuffer::<f32>::new(need, spec));
        }
        let sb = sbuf.as_mut().unwrap();
        sb.copy_interleaved_ref(decoded);
        frames.clear();
        for f in sb.samples().chunks_exact(ch) {
            if ch == 1 {
                frames.push([f[0], f[0]]);
            } else {
                frames.push([f[0], f[1]]);
            }
        }
        out.clear();
        rs.process(&frames, &mut out);
        let mut q = shared.q.lock().unwrap();
        if q.gen != gen {
            return Ok(());
        }
        q.samples.extend(out.iter().copied());
    }
}
