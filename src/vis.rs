//! Classic spectrum analyzer / oscilloscope.

use crate::gfx::{r, Image, Rect};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum VisMode {
    Spectrum,
    Oscilloscope,
    Off,
}

impl VisMode {
    pub fn next(self) -> VisMode {
        match self {
            VisMode::Spectrum => VisMode::Oscilloscope,
            VisMode::Oscilloscope => VisMode::Off,
            VisMode::Off => VisMode::Spectrum,
        }
    }
}

pub const NUM_BARS: usize = 19;
const FFT_N: usize = 512;

pub struct Analyzer {
    pub bars: [f32; NUM_BARS],  // 0..=16
    pub peaks: [f32; NUM_BARS], // 0..=16
    peak_hold: [u32; NUM_BARS],
    pub wave: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    window: Vec<f32>,
}

impl Analyzer {
    pub fn new() -> Self {
        let window = (0..FFT_N)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FFT_N - 1) as f32).cos())
            .collect();
        Analyzer {
            bars: [0.0; NUM_BARS],
            peaks: [0.0; NUM_BARS],
            peak_hold: [0; NUM_BARS],
            wave: vec![0.0; 76],
            re: vec![0.0; FFT_N],
            im: vec![0.0; FFT_N],
            window,
        }
    }

    pub fn update(&mut self, samples: &[f32], rate: u32, active: bool) {
        let n = samples.len();
        // oscilloscope: the latest 76 * 2 samples, decimated
        for (i, w) in self.wave.iter_mut().enumerate() {
            let idx = n.saturating_sub(152) + i * 2;
            *w = if active { samples.get(idx).copied().unwrap_or(0.0) } else { 0.0 };
        }
        let mut target = [0.0f32; NUM_BARS];
        if active && n >= FFT_N {
            let src = &samples[n - FFT_N..];
            for i in 0..FFT_N {
                self.re[i] = src[i] * self.window[i];
                self.im[i] = 0.0;
            }
            fft(&mut self.re, &mut self.im);
            let bin_hz = rate as f32 / FFT_N as f32;
            let (lo, hi) = (40.0f32, 16000.0f32.min(rate as f32 * 0.48));
            for (b, t) in target.iter_mut().enumerate() {
                let f0 = lo * (hi / lo).powf(b as f32 / NUM_BARS as f32);
                let f1 = lo * (hi / lo).powf((b + 1) as f32 / NUM_BARS as f32);
                let i0 = ((f0 / bin_hz) as usize).max(1);
                let i1 = ((f1 / bin_hz) as usize).max(i0 + 1).min(FFT_N / 2);
                let mut m = 0.0f32;
                for i in i0..i1 {
                    m = m.max((self.re[i] * self.re[i] + self.im[i] * self.im[i]).sqrt());
                }
                // tilt to compensate the natural high-frequency roll-off of music
                let tilt = 1.0 + b as f32 * 0.12;
                let db = 20.0 * (m * tilt / (FFT_N as f32 / 4.0) + 1e-9).log10();
                *t = ((db + 54.0) / 54.0 * 16.0).clamp(0.0, 16.0);
            }
        }
        for b in 0..NUM_BARS {
            // fast attack, gradual fall-off
            self.bars[b] = if target[b] > self.bars[b] { target[b] } else { (self.bars[b] - 1.2).max(target[b]) };
            if self.bars[b] >= self.peaks[b] {
                self.peaks[b] = self.bars[b];
                self.peak_hold[b] = 6;
            } else if self.peak_hold[b] > 0 {
                self.peak_hold[b] -= 1;
            } else {
                self.peaks[b] = (self.peaks[b] - 0.35).max(0.0);
            }
        }
    }

    /// Draw into the 76x16 main-window area.
    pub fn draw(&self, dst: &mut Image, area: Rect, colors: &[u32; 24], mode: VisMode) {
        // background with the dotted grid
        dst.fill(area, colors[0]);
        for y in (1..area.h).step_by(2) {
            for x in (0..area.w).step_by(2) {
                dst.set(area.x + x, area.y + y, colors[1]);
            }
        }
        match mode {
            VisMode::Off => {}
            VisMode::Spectrum => {
                for b in 0..NUM_BARS {
                    let x = area.x + b as i32 * 4;
                    let h = self.bars[b].round() as i32;
                    for j in 0..h.min(area.h) {
                        let y = area.y + area.h - 1 - j;
                        // classic coloring: row color by height (2 = top .. 17 = bottom)
                        let c = colors[(17 - j).clamp(2, 17) as usize];
                        dst.fill(r(x, y, 3, 1), c);
                    }
                    let p = self.peaks[b].round() as i32;
                    if p > 0 {
                        let y = area.y + area.h - p.min(area.h);
                        dst.fill(r(x, y, 3, 1), colors[23]);
                    }
                }
            }
            VisMode::Oscilloscope => {
                let mid = area.h / 2;
                let mut prev: Option<i32> = None;
                for (i, s) in self.wave.iter().enumerate().take(area.w as usize) {
                    let y = (mid as f32 - s * mid as f32 * 1.6).round().clamp(0.0, area.h as f32 - 1.0) as i32;
                    let (a, b) = match prev {
                        Some(p) => (p.min(y), p.max(y)),
                        None => (y, y),
                    };
                    for yy in a..=b {
                        let d = (yy - mid).abs();
                        let c = colors[18 + (d * 5 / (mid + 1)).clamp(0, 4) as usize];
                        dst.set(area.x + i as i32, area.y + yy, c);
                    }
                    prev = Some(y);
                }
            }
        }
    }

    /// Tiny 38x5 analyzer used in window-shade mode.
    pub fn draw_small(&self, dst: &mut Image, area: Rect, colors: &[u32; 24], mode: VisMode) {
        dst.fill(area, colors[0]);
        match mode {
            VisMode::Off => {}
            VisMode::Spectrum => {
                for b in 0..NUM_BARS {
                    let h = (self.bars[b] * area.h as f32 / 16.0).round() as i32;
                    for j in 0..h.min(area.h) {
                        let c = colors[(17 - j * 3).clamp(2, 17) as usize];
                        dst.fill(r(area.x + b as i32 * 2, area.y + area.h - 1 - j, 1, 1), c);
                    }
                }
            }
            VisMode::Oscilloscope => {
                for i in 0..area.w {
                    let s = self.wave.get(i as usize * 2).copied().unwrap_or(0.0);
                    let y = (2.0 - s * 4.0).round().clamp(0.0, area.h as f32 - 1.0) as i32;
                    dst.set(area.x + i, area.y + y, colors[18]);
                }
            }
        }
    }
}

/// In-place iterative radix-2 FFT.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = start + k;
                let b = a + len / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
        }
        len <<= 1;
    }
}
