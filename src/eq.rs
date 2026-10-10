//! 10-band graphic equalizer (peaking biquads) at the classic 60 Hz–16 kHz frequencies.

pub const BANDS: [f32; 10] = [60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0];
pub const BAND_NAMES: [&str; 10] = ["60HZ", "170HZ", "310HZ", "600HZ", "1KHZ", "3KHZ", "6KHZ", "12KHZ", "14KHZ", "16KHZ"];
pub const MAX_DB: f32 = 12.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Equalizer {
    pub on: bool,
    pub preamp: f32,     // dB
    pub bands: [f32; 10], // dB
}

impl Default for Equalizer {
    fn default() -> Self {
        Equalizer { on: false, preamp: 0.0, bands: [0.0; 10] }
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: [f32; 2],
    z2: [f32; 2],
}

impl Biquad {
    fn peaking(rate: f32, freq: f32, q: f32, db: f32) -> Biquad {
        let freq = freq.min(rate * 0.45);
        let a = 10f32.powf(db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * freq / rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos = w0.cos();
        let a0 = 1.0 + alpha / a;
        Biquad {
            b0: (1.0 + alpha * a) / a0,
            b1: (-2.0 * cos) / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha / a) / a0,
            z1: [0.0; 2],
            z2: [0.0; 2],
        }
    }

    #[inline]
    fn run(&mut self, ch: usize, x: f32) -> f32 {
        // transposed direct form II
        let y = self.b0 * x + self.z1[ch];
        self.z1[ch] = self.b1 * x - self.a1 * y + self.z2[ch];
        self.z2[ch] = self.b2 * x - self.a2 * y;
        y
    }
}

pub struct EqProcessor {
    rate: f32,
    on: bool,
    pre: f32,
    filters: Vec<Biquad>,
}

impl EqProcessor {
    pub fn new(rate: f32) -> Self {
        EqProcessor { rate, on: false, pre: 1.0, filters: Vec::new() }
    }

    pub fn configure(&mut self, eq: &Equalizer) {
        self.on = eq.on;
        self.pre = 10f32.powf(eq.preamp / 20.0);
        let old = std::mem::take(&mut self.filters);
        self.filters = BANDS
            .iter()
            .zip(eq.bands.iter())
            .map(|(&f, &db)| Biquad::peaking(self.rate, f, 1.2, db))
            .collect();
        // keep filter memory where possible to avoid clicks while dragging sliders
        for (n, o) in self.filters.iter_mut().zip(old.iter()) {
            n.z1 = o.z1;
            n.z2 = o.z2;
        }
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.on {
            return (l, r);
        }
        let (mut l, mut r) = (l * self.pre, r * self.pre);
        for f in &mut self.filters {
            l = f.run(0, l);
            r = f.run(1, r);
        }
        (l, r)
    }
}

/// Built-in presets (dB values).
pub const PRESETS: &[(&str, [f32; 10])] = &[
    ("FLAT", [0.0; 10]),
    ("CLASSICAL", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -4.4, -4.4, -4.4, -5.8]),
    ("CLUB", [0.0, 0.0, 4.8, 3.3, 3.3, 3.3, 1.9, 0.0, 0.0, 0.0]),
    ("DANCE", [5.8, 4.3, 1.4, 0.0, 0.0, -3.4, -4.4, -4.4, 0.0, 0.0]),
    ("FULL BASS", [5.8, 5.8, 5.8, 3.4, 1.0, -2.9, -5.8, -6.8, -7.3, -7.3]),
    ("FULL BASS & TREBLE", [4.3, 3.4, 0.0, -4.4, -2.9, 1.0, 4.8, 6.3, 6.8, 6.8]),
    ("FULL TREBLE", [-5.8, -5.8, -5.8, -2.4, 1.4, 6.8, 9.6, 9.6, 9.6, 10.1]),
    ("LARGE HALL", [6.3, 6.3, 3.4, 3.4, 0.0, -2.9, -2.9, -2.9, 0.0, 0.0]),
    ("LIVE", [-2.9, 0.0, 2.4, 3.3, 3.4, 3.4, 2.4, 1.4, 1.4, 1.4]),
    ("PARTY", [4.3, 4.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 4.3, 4.3]),
    ("POP", [-1.0, 2.9, 4.3, 4.8, 3.3, 0.0, -1.4, -1.4, -1.0, -1.0]),
    ("REGGAE", [0.0, 0.0, 0.0, -3.4, 0.0, 3.8, 3.8, 0.0, 0.0, 0.0]),
    ("ROCK", [4.8, 2.9, -3.4, -4.8, -1.9, 2.4, 5.3, 6.8, 6.8, 6.8]),
    ("SKA", [-1.4, -2.9, -2.4, 0.0, 2.4, 3.3, 5.3, 5.8, 6.8, 5.8]),
    ("SOFT", [2.9, 1.0, 0.0, -1.4, 0.0, 2.4, 4.8, 5.8, 6.8, 7.3]),
    ("SOFT ROCK", [2.4, 2.4, 1.4, 0.0, -2.4, -3.4, -1.9, 0.0, 1.4, 5.3]),
    ("TECHNO", [4.8, 3.3, 0.0, -3.4, -2.9, 0.0, 4.8, 5.8, 5.8, 5.3]),
];
