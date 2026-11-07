//! OpenDeepCool's spectrum (`docs/spec/music.md`, "OpenDeepCool's spectrum, precisely"): the
//! latest 2048 samples analysed into 14 column levels, the bars' fall, the peak dots, and the
//! heights they're drawn at. No clock and no I/O: the service's audio reader gives the samples,
//! and its engine keeps one `Spectrum` while a Music mode with a spectrum is shown.

use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

use realfft::{RealFftPlanner, RealToComplex};

/// The analysis window: the latest 2048 samples (point 1).
pub const WINDOW: usize = 2048;
/// One column per band, column 1 at the left (point 2).
pub const COLUMNS: usize = 14;
/// Samples per second (Spectrum: analysis, Source).
const RATE: f64 = 48_000.0;

/// The 14 column levels of the latest 2048 samples, each 0 to 1 (points 1–3): a Blackman window
/// with α = 0.16, X[k] = (1/N) Σ x̂[n] e^(−2πikn/N), 20 log₁₀ |X[k]| dB per bin, the highest
/// bin of each band, and −75 to −25 dB mapped to 0 to 1.
pub fn analyse(window: &[f32; WINDOW]) -> [f64; COLUMNS] {
    static FFT: OnceLock<Arc<dyn RealToComplex<f64>>> = OnceLock::new();
    let fft = FFT.get_or_init(|| RealFftPlanner::new().plan_fft_forward(WINDOW));
    let mut input: Vec<f64> = window
        .iter()
        .zip(blackman())
        .map(|(&x, w)| f64::from(x) * w)
        .collect();
    let mut output = fft.make_output_vec();
    fft.process(&mut input, &mut output)
        .expect("buffers made for the planned length");
    let db = |k: usize| 20.0 * (output[k].norm() / WINDOW as f64).log10();
    std::array::from_fn(|c| {
        let (low, high) = bins(c);
        let loudest = (low..high).map(db).fold(f64::NEG_INFINITY, f64::max);
        ((loudest + 75.0) / 50.0).clamp(0.0, 1.0)
    })
}

/// The Blackman window with α = 0.16, as the W3C `AnalyserNode` defines it.
fn blackman() -> &'static [f64; WINDOW] {
    static WEIGHTS: OnceLock<[f64; WINDOW]> = OnceLock::new();
    WEIGHTS.get_or_init(|| {
        let alpha = 0.16;
        let (a0, a1, a2) = ((1.0 - alpha) / 2.0, 0.5, alpha / 2.0);
        std::array::from_fn(|n| {
            let x = n as f64 / WINDOW as f64;
            a0 - a1 * (2.0 * PI * x).cos() + a2 * (4.0 * PI * x).cos()
        })
    })
}

/// The bins column `c` (from 0) takes, `low..high`: those from edge *c* up to below edge *c* + 1,
/// with edge *i* = 40 × 400^(*i*/14) Hz, and at least the bin nearest edge *c* (point 2).
fn bins(c: usize) -> (usize, usize) {
    let edge = |i: usize| 40.0 * 400f64.powf(i as f64 / COLUMNS as f64);
    let bin = RATE / WINDOW as f64;
    // Bin k is at k × bin Hz: the first at or above an edge is ceil(edge ÷ bin).
    let (from, below) = ((edge(c) / bin).ceil(), (edge(c + 1) / bin).ceil());
    let nearest = (edge(c) / bin).round();
    let low = nearest.min(from) as usize;
    let high = (below as usize).max(nearest as usize + 1);
    (low, high)
}

/// Winamp's five falloff stops, slowest first (point 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stop {
    Slower,
    Slow,
    Moderate,
    Fast,
    Faster,
}

impl Stop {
    /// Every stop, slowest first.
    pub const ALL: [Stop; 5] = [
        Stop::Slower,
        Stop::Slow,
        Stop::Moderate,
        Stop::Fast,
        Stop::Faster,
    ];

    /// The stop's name, as the settings and the expected files write it.
    pub fn name(self) -> &'static str {
        match self {
            Stop::Slower => "slower",
            Stop::Slow => "slow",
            Stop::Moderate => "moderate",
            Stop::Fast => "fast",
            Stop::Faster => "faster",
        }
    }

    /// The stop named `name`, or `None` when it isn't one of the five.
    pub fn from_name(name: &str) -> Option<Stop> {
        Stop::ALL.into_iter().find(|stop| stop.name() == name)
    }

    /// *b*, Winamp's bar value: a bar falls *b* ÷ 72 of the height per tick (point 5).
    fn bar_value(self) -> f64 {
        match self {
            Stop::Slower => 3.0,
            Stop::Slow => 6.0,
            Stop::Moderate => 12.0,
            Stop::Fast => 16.0,
            Stop::Faster => 32.0,
        }
    }

    /// *g*, Winamp's peak value: a peak's speed grows by *g* every Winamp frame (point 6).
    fn peak_value(self) -> f64 {
        match self {
            Stop::Slower => 1.05,
            Stop::Slow => 1.1,
            Stop::Moderate => 1.2,
            Stop::Fast => 1.4,
            Stop::Faster => 1.6,
        }
    }
}

/// The Music settings (point 7). A change shows on the next frame and resets nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MusicSettings {
    /// Whether the peak dots are drawn; they follow the sound either way.
    pub peaks: bool,
    pub bar_fall: Stop,
    pub peak_fall: Stop,
}

impl Default for MusicSettings {
    /// Peak dots on, bars moderate, peaks slow: Winamp's defaults.
    fn default() -> MusicSettings {
        MusicSettings {
            peaks: true,
            bar_fall: Stop::Moderate,
            peak_fall: Stop::Slow,
        }
    }
}

/// One drawn column: its bar's height in rows from the floor, 1 at least, and its peak dot's
/// height when one is drawn, always above the bar and within the area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    pub bar: u8,
    pub dot: Option<u8>,
}

/// The 14 drawn columns, column 1 first.
pub type Columns = [Column; COLUMNS];

/// The bars and peaks, as levels 0 to 1, so a switch between heights keeps them (point 8).
#[derive(Clone, Debug, PartialEq)]
pub struct Spectrum {
    bars: [f64; COLUMNS],
    /// The peak now, the level *p₀* it was last set to, and the ticks *t* since.
    peaks: [f64; COLUMNS],
    set_to: [f64; COLUMNS],
    since: [u32; COLUMNS],
    /// The peak fall of the last tick, once there was one.
    peak_fall: Option<Stop>,
}

impl Spectrum {
    /// Entering a Music mode from any other mode: every level 0 (point 8).
    pub fn enter() -> Spectrum {
        Spectrum {
            bars: [0.0; COLUMNS],
            peaks: [0.0; COLUMNS],
            set_to: [0.0; COLUMNS],
            since: [0; COLUMNS],
            peak_fall: None,
        }
    }

    /// A spectrum standing at `bars` and `peaks`, levels 0 to 1, each peak at its bar or above
    /// it, as if it had come there: the settings app's Music tiles draw a fixed one (Phase 10
    /// plan, Task 5). A level outside 0–1 is taken as the nearest end, and a peak below its bar
    /// as the bar.
    pub fn at(bars: [f64; COLUMNS], peaks: [f64; COLUMNS]) -> Spectrum {
        let bars = bars.map(|level| level.clamp(0.0, 1.0));
        let peaks: [f64; COLUMNS] = std::array::from_fn(|c| peaks[c].clamp(0.0, 1.0).max(bars[c]));
        Spectrum {
            bars,
            peaks,
            set_to: peaks,
            since: [0; COLUMNS],
            peak_fall: None,
        }
    }

    /// One tick with this tick's `levels` (points 5 and 6): each bar rises to a higher level at
    /// once, or falls by *b* ÷ 72 but never below the level; then each peak is set to its bar
    /// when the bar is at it or above, or falls by Winamp's recurrence summed over the tick's
    /// 10/3 frames. On the first tick with a new peak fall, each peak first falls on from where it
    /// is, from rest (owner, 2026-11-05). The peaks run with the dots off too.
    pub fn update(&mut self, levels: &[f64; COLUMNS], settings: &MusicSettings) {
        if self
            .peak_fall
            .is_some_and(|last| last != settings.peak_fall)
        {
            self.set_to = self.peaks;
            self.since = [0; COLUMNS];
        }
        self.peak_fall = Some(settings.peak_fall);
        let fall = settings.bar_fall.bar_value() / 72.0;
        let g = settings.peak_fall.peak_value();
        for (c, &level) in levels.iter().enumerate() {
            self.bars[c] = level.max(self.bars[c] - fall).max(0.0);
            if self.bars[c] >= self.peaks[c] {
                self.peaks[c] = self.bars[c];
                self.set_to[c] = self.bars[c];
                self.since[c] = 0;
            } else {
                self.since[c] = self.since[c].saturating_add(1);
                let frames = f64::from(self.since[c]) * 10.0 / 3.0;
                let fallen = 3.0 / 3840.0 * (g.powf(frames) - 1.0) / (g - 1.0);
                self.peaks[c] = (self.set_to[c] - fallen).max(0.0);
            }
        }
    }

    /// Each bar's level, 0 to 1, column 1 first.
    pub fn bars(&self) -> [f64; COLUMNS] {
        self.bars
    }

    /// Each peak's level, 0 to 1, column 1 first, drawn or not.
    pub fn peaks(&self) -> [f64; COLUMNS] {
        self.peaks
    }

    /// The columns drawn in an area `h` rows high: each bar max(1, round(level × *h*)) rows, and,
    /// with `peaks` on, its dot at round(peak × *h*) when that is above the bar and within the
    /// area (points 5 and 6).
    pub fn draw(&self, h: u8, peaks: bool) -> Columns {
        let rows = |level: f64| (level * f64::from(h)).round() as u8;
        std::array::from_fn(|c| {
            let bar = rows(self.bars[c]).max(1);
            let top = rows(self.peaks[c]);
            Column {
                bar,
                dot: (peaks && top > bar && top <= h).then_some(top),
            }
        })
    }
}
