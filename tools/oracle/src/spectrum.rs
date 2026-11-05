//! OpenDeepCool's spectrum (`docs/spec/music.md`, "OpenDeepCool's spectrum, precisely"), written
//! from the spec text alone (Phase 9 plan, Task 1): its own radix-2 FFT, the Blackman window, the
//! bands and loudness, the bars' fall and the peak dots. It writes the test signals,
//! `spectrum/signals.txt`, and the spectrum rows each one draws at every tick, `spectrum/expected.txt`.

use std::f64::consts::PI;
use std::fmt::Write as _;

/// Samples per second.
const RATE: f64 = 48_000.0;
/// Samples per 100 ms tick.
pub const TICK_SAMPLES: usize = 4800;
/// The analysis window: the latest 2048 samples.
pub const N: usize = 2048;
/// Columns.
pub const COLUMNS: usize = 14;

/// Winamp's five falloff stops (music.md point 4): the bar value *b* and the peak value *g*.
pub const STOPS: [(&str, f64, f64); 5] = [
    ("slower", 3.0, 1.05),
    ("slow", 6.0, 1.1),
    ("moderate", 12.0, 1.2),
    ("fast", 16.0, 1.4),
    ("faster", 32.0, 1.6),
];

/// Edge *i* = 40 × 400^(*i*/14) Hz, *i* = 0 … 14 (point 2).
fn edge(i: usize) -> f64 {
    40.0 * 400f64.powf(i as f64 / COLUMNS as f64)
}

/// The geometric centre of column *c*, 1 to 14.
pub fn centre(c: usize) -> f64 {
    40.0 * 400f64.powf((c as f64 - 0.5) / COLUMNS as f64)
}

/// An in-place iterative radix-2 FFT of `re` + i·`im`, length a power of two:
/// X[k] = Σ x[n] e^(−2πikn/N).
fn fft(re: &mut [f64], im: &mut [f64]) {
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
        let angle = -2.0 * PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((angle * k as f64).cos(), (angle * k as f64).sin());
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

/// Each column's level before clamping: (dB + 75) ÷ 50 of the band's loudest bin (points 1–3).
/// Silence is −∞.
pub fn unclamped_levels(window: &[f32]) -> [f64; COLUMNS] {
    assert_eq!(window.len(), N);
    // The Blackman window with α = 0.16 (W3C AnalyserNode).
    let alpha = 0.16;
    let (a0, a1, a2) = ((1.0 - alpha) / 2.0, 0.5, alpha / 2.0);
    let mut re: Vec<f64> = window
        .iter()
        .enumerate()
        .map(|(n, &x)| {
            let w = a0 - a1 * (2.0 * PI * n as f64 / N as f64).cos()
                + a2 * (4.0 * PI * n as f64 / N as f64).cos();
            f64::from(x) * w
        })
        .collect();
    let mut im = vec![0.0; N];
    fft(&mut re, &mut im);
    let bin_hz = RATE / N as f64;
    let db = |k: usize| 20.0 * ((re[k] * re[k] + im[k] * im[k]).sqrt() / N as f64).log10();
    let mut levels = [f64::NEG_INFINITY; COLUMNS];
    for (c, level) in levels.iter_mut().enumerate() {
        let (low, high) = (edge(c), edge(c + 1));
        let nearest = (low / bin_hz).round() as usize;
        let mut loudest = db(nearest);
        for k in 0..=N / 2 {
            let f = k as f64 * bin_hz;
            if f >= low && f < high {
                loudest = loudest.max(db(k));
            }
        }
        *level = (loudest + 75.0) / 50.0;
    }
    levels
}

/// The levels 0–1 (point 3).
#[cfg(test)]
fn levels(window: &[f32]) -> [f64; COLUMNS] {
    unclamped_levels(window).map(|l| l.clamp(0.0, 1.0))
}

/// The bars and peak dots of one column set (points 5, 6 and 8).
#[derive(Clone, Debug)]
pub struct Spectrum {
    bars: [f64; COLUMNS],
    /// The level each peak was last set to, the ticks since, and the peak now.
    p0: [f64; COLUMNS],
    t: [u32; COLUMNS],
    peaks: [f64; COLUMNS],
    /// The peak value *g* of the last tick, once there was one.
    g: Option<f64>,
}

impl Spectrum {
    /// Entering Music: every level 0.
    pub fn enter() -> Spectrum {
        Spectrum {
            bars: [0.0; COLUMNS],
            p0: [0.0; COLUMNS],
            t: [0; COLUMNS],
            peaks: [0.0; COLUMNS],
            g: None,
        }
    }

    /// One tick: each bar rises at once or falls by *b* ÷ 72, never below the level; then each
    /// peak is reset to its bar, or falls by Winamp's recurrence over the tick's 10/3 frames. On
    /// the first tick with a new *g*, each peak first falls on from where it is: *p₀* its level
    /// and *t* 0 (point 6; owner, 2026-11-05).
    pub fn update(&mut self, levels: &[f64; COLUMNS], b: f64, g: f64) {
        if self.g.is_some_and(|last| last != g) {
            self.p0 = self.peaks;
            self.t = [0; COLUMNS];
        }
        self.g = Some(g);
        for (c, &level) in levels.iter().enumerate() {
            self.bars[c] = level.max(self.bars[c] - b / 72.0).max(0.0);
            if self.bars[c] >= self.peaks[c] {
                self.p0[c] = self.bars[c];
                self.t[c] = 0;
                self.peaks[c] = self.bars[c];
            } else {
                self.t[c] += 1;
                let frames = f64::from(self.t[c]) * 10.0 / 3.0;
                let fallen = (3.0 / 3840.0) * (g.powf(frames) - 1.0) / (g - 1.0);
                self.peaks[c] = (self.p0[c] - fallen).max(0.0);
            }
        }
    }

    /// The values a picture rounds: each bar × *h*, and each peak × *h*.
    pub fn scaled(&self, h: u8) -> ([f64; COLUMNS], [f64; COLUMNS]) {
        let h = f64::from(h);
        (self.bars.map(|b| b * h), self.peaks.map(|p| p * h))
    }

    /// Each column's bar height, max(1, round(bar × *h*)) rows, and its dot's height when peaks
    /// are on and the dot is above the bar and within the area.
    pub fn draw(&self, h: u8, peaks: bool) -> [(u8, Option<u8>); COLUMNS] {
        let (bars, tops) = self.scaled(h);
        std::array::from_fn(|c| {
            let bar = (bars[c].round() as u8).max(1);
            let top = tops[c].round() as u8;
            (bar, (peaks && top > bar && top <= h).then_some(top))
        })
    }
}

/// One stretch of a test signal.
#[derive(Clone, Debug)]
pub enum Segment {
    Silence(usize),
    /// Tones summed, each a frequency in Hz and a peak amplitude in dBFS, for so many ticks.
    Tones(usize, Vec<(f64, f64)>),
    /// White noise from the generator below, at a peak amplitude in dBFS.
    Noise(usize, f64),
}

/// A test signal: its name and segments.
#[derive(Clone, Debug)]
pub struct Signal {
    pub name: String,
    pub segments: Vec<Segment>,
    /// The dB its amplitudes were moved by, so no tick lands near a rounding boundary.
    pub moved_db: f64,
}

/// The noise generator, as `signals.txt` states it: x ← (1103515245 x + 12345) mod 2³¹ from x =
/// 1, and the sample is x ÷ 2³⁰ − 1 times the amplitude.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = (1_103_515_245 * self.0 + 12_345) % (1 << 31);
        self.0 as f64 / f64::from(1u32 << 30) - 1.0
    }
}

/// The text a frequency or level is written with, which both sides read back: 6 decimals.
fn exact(x: f64) -> f64 {
    format!("{x:.6}").parse().expect("a number")
}

impl Signal {
    fn ticks(&self) -> usize {
        self.segments
            .iter()
            .map(|s| match s {
                Segment::Silence(t) | Segment::Tones(t, _) | Segment::Noise(t, _) => *t,
            })
            .sum()
    }

    /// The samples, mono (both channels alike), each computed in f64 then stored as f32. A tone's
    /// phase counts from the signal's first sample.
    pub fn samples(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.ticks() * TICK_SAMPLES);
        let mut noise = Lcg(1);
        for segment in &self.segments {
            match segment {
                Segment::Silence(t) => out.extend(std::iter::repeat_n(0.0f32, t * TICK_SAMPLES)),
                Segment::Tones(t, tones) => {
                    for _ in 0..t * TICK_SAMPLES {
                        let n = out.len() as f64;
                        let x: f64 = tones
                            .iter()
                            .map(|&(f, db)| {
                                let a = 10f64.powf((db + self.moved_db) / 20.0);
                                a * (2.0 * PI * f * n / RATE).sin()
                            })
                            .sum();
                        out.push(x as f32);
                    }
                }
                Segment::Noise(t, db) => {
                    let a = 10f64.powf((db + self.moved_db) / 20.0);
                    for _ in 0..t * TICK_SAMPLES {
                        out.push((a * noise.next()) as f32);
                    }
                }
            }
        }
        out
    }

    /// The column levels at every tick, unclamped: the window of tick *n* is the last 2048
    /// samples of its 100 ms.
    pub fn tick_levels(&self) -> Vec<[f64; COLUMNS]> {
        let samples = self.samples();
        (0..self.ticks())
            .map(|n| {
                let end = (n + 1) * TICK_SAMPLES;
                unclamped_levels(&samples[end - N..end])
            })
            .collect()
    }

    /// Whether a beat train, which only `dancer.txt` uses: `expected.txt` leaves them out.
    pub fn is_beat_train(&self) -> bool {
        self.name.starts_with("beat train")
    }

    /// Whether any value the pictures round, at any tick and setting, lies within 10⁻⁶ of a
    /// rounding boundary, a level of the −75/−25 dB clamps, or one of the Dancer's thresholds
    /// (Phase 9 plan, Task 1).
    fn near_a_boundary(&self) -> bool {
        let near = |x: f64| (x - x.floor() - 0.5).abs() < 1e-6;
        let per_tick = self.tick_levels();
        if per_tick
            .iter()
            .flatten()
            .any(|l| l.is_finite() && (l.abs() < 1e-6 || (l - 1.0).abs() < 1e-6))
        {
            return true;
        }
        let clamped: Vec<[f64; COLUMNS]> = per_tick
            .iter()
            .map(|l| l.map(|x| x.clamp(0.0, 1.0)))
            .collect();
        let mut previous = [0.0; COLUMNS];
        for levels in &clamped {
            if crate::dancer::near_a_threshold(&previous, levels) {
                return true;
            }
            previous = *levels;
        }
        for &(_, b, _) in &STOPS {
            for &(_, _, g) in &STOPS {
                let mut s = Spectrum::enter();
                for levels in &clamped {
                    s.update(levels, b, g);
                    for h in [12, 6] {
                        let (bars, peaks) = s.scaled(h);
                        if bars.iter().chain(&peaks).any(|&x| near(x)) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

/// The test signals (Phase 9 plan, Task 1), each moved by 0.5 dB steps until no value it draws
/// lies near a rounding boundary.
pub fn signals() -> Vec<Signal> {
    let tone = |f: f64, db: f64| (exact(f), db);
    let mut list: Vec<(String, Vec<Segment>)> =
        vec![("silence".into(), vec![Segment::Silence(10)])];
    for db in [-20.0, -45.0, -70.0] {
        for c in 1..=COLUMNS {
            list.push((
                format!("column {c} centre, {db} dBFS"),
                vec![
                    Segment::Tones(3, vec![tone(centre(c), db)]),
                    Segment::Silence(3),
                ],
            ));
        }
    }
    for i in 0..=COLUMNS {
        for (side, factor) in [("below", 0.98), ("above", 1.02)] {
            list.push((
                format!("edge {i} {side}, -20 dBFS"),
                vec![
                    Segment::Tones(3, vec![tone(edge(i) * factor, -20.0)]),
                    Segment::Silence(3),
                ],
            ));
        }
    }
    for (a, b) in [(1, 8), (3, 12), (5, 6), (2, 14), (7, 10)] {
        list.push((
            format!("columns {a} and {b}, -26 dBFS each"),
            vec![
                Segment::Tones(3, vec![tone(centre(a), -26.0), tone(centre(b), -26.0)]),
                Segment::Silence(3),
            ],
        ));
    }
    let mut bursts = Vec::new();
    for _ in 0..6 {
        bursts.push(Segment::Tones(1, vec![tone(centre(7), -20.0)]));
        bursts.push(Segment::Silence(4));
    }
    bursts.push(Segment::Silence(30));
    list.push((
        "burst train, column 7, -20 dBFS, 100 ms on, 400 ms off".into(),
        bursts,
    ));
    let mut sweep: Vec<Segment> = (0..28)
        .map(|j| {
            let f = 40.0 * 400f64.powf((f64::from(j) + 0.5) / 28.0);
            Segment::Tones(2, vec![tone(f, -20.0)])
        })
        .collect();
    sweep.push(Segment::Silence(10));
    list.push(("sweep, 40 Hz to 16 kHz, -20 dBFS".into(), sweep));
    list.push((
        "white noise, 0 dBFS".into(),
        vec![Segment::Noise(10, 0.0), Segment::Silence(10)],
    ));
    list.extend(crate::dancer::beat_trains());
    list.into_iter()
        .map(|(name, segments)| {
            let mut signal = Signal {
                name,
                segments,
                moved_db: 0.0,
            };
            while signal.near_a_boundary() {
                signal.moved_db -= 0.5;
            }
            signal
        })
        .collect()
}

/// `spectrum/signals.txt`.
pub fn signals_file(signals: &[Signal]) -> String {
    let mut out = String::from(
        "# Spectrum test signals\n\n\
         48 000 samples per second, one channel (both alike); a tick is 4800 samples, and the \
         window of tick n (from 0) is the last 2048 samples of its 100 ms: samples 4800(n + 1) − 2048 \
         to 4800(n + 1) − 1. A tone's sample n, counted from the signal's first sample, is \
         A · sin(2π f n / 48 000) with A = 10^((dBFS + moved) / 20), computed in f64 and stored as \
         f32; tones on one line add. Noise: x ← (1103515245 x + 12345) mod 2³¹ from x = 1 at the \
         signal's start, the sample (x ÷ 2³⁰ − 1) · A. `moved` is the dB a signal was moved by so \
         that no value it draws lies within 10⁻⁶ of a rounding boundary.\n\n",
    );
    for s in signals {
        let _ = writeln!(out, "## {}\n", s.name);
        let _ = writeln!(out, "moved {}", s.moved_db);
        for segment in &s.segments {
            match segment {
                Segment::Silence(t) => {
                    let _ = writeln!(out, "silence {t}");
                }
                Segment::Tones(t, tones) => {
                    let list: Vec<String> = tones
                        .iter()
                        .map(|(f, db)| format!("{f:.6} Hz {db} dBFS"))
                        .collect();
                    let _ = writeln!(out, "tones {t}: {}", list.join(", "));
                }
                Segment::Noise(t, db) => {
                    let _ = writeln!(out, "noise {t}: {db} dBFS");
                }
            }
        }
        out.push('\n');
    }
    let mut text = out.trim_end().to_owned();
    text.push('\n');
    text
}

/// A height as one character: `1` to `9`, then `a`, `b`, `c`.
fn hex(h: u8) -> char {
    char::from_digit(u32::from(h), 16).expect("a height up to 12")
}

/// `spectrum/expected.txt`: for each signal but the beat trains, *H* 12 and 6, peaks off with
/// each bar stop and on with each bar and peak stop, every tick's bar heights and dot heights
/// (`.` for none).
pub fn expected_file(signals: &[Signal]) -> String {
    let mut out = String::from(
        "# Spectrum: expected rows\n\n\
         For each signal of `signals.txt` but the Dancer's beat trains: each tick line gives the 14 \
         bar heights, columns 1 to 14, in rows from the floor, then, with peaks on, the 14 dot \
         heights, `.` where no dot is drawn. Heights are 1 to 9, then a, b, c for 10 to 12. The \
         spectrum enters at level 0 at tick 0.\n\n",
    );
    for s in signals.iter().filter(|s| !s.is_beat_train()) {
        let levels: Vec<[f64; COLUMNS]> = s
            .tick_levels()
            .iter()
            .map(|l| l.map(|x| x.clamp(0.0, 1.0)))
            .collect();
        for h in [12u8, 6] {
            for peaks in [false, true] {
                for &(bar_name, b, _) in &STOPS {
                    for &(peak_name, _, g) in &STOPS {
                        if !peaks && peak_name != "slow" {
                            continue;
                        }
                        let heading = if peaks {
                            format!(
                                "## {} · H {h} · peaks on · bars {bar_name} · peaks {peak_name}",
                                s.name
                            )
                        } else {
                            format!("## {} · H {h} · peaks off · bars {bar_name}", s.name)
                        };
                        let _ = writeln!(out, "{heading}\n");
                        let mut spectrum = Spectrum::enter();
                        for (n, tick) in levels.iter().enumerate() {
                            spectrum.update(tick, b, g);
                            let drawn = spectrum.draw(h, peaks);
                            let bars: String = drawn.iter().map(|&(bar, _)| hex(bar)).collect();
                            if peaks {
                                let dots: String =
                                    drawn.iter().map(|&(_, dot)| dot.map_or('.', hex)).collect();
                                let _ = writeln!(out, "{n}: {bars} {dots}");
                            } else {
                                let _ = writeln!(out, "{n}: {bars}");
                            }
                        }
                        out.push('\n');
                    }
                }
            }
        }
    }
    let mut text = out.trim_end().to_owned();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone_window(f: f64, db: f64) -> Vec<f32> {
        let a = 10f64.powf(db / 20.0);
        (0..N)
            .map(|n| (a * (2.0 * PI * f * n as f64 / RATE).sin()) as f32)
            .collect()
    }

    // The FFT against a plain discrete Fourier transform, on a mixed signal.
    #[test]
    fn the_fft_is_the_discrete_fourier_transform() {
        let n = 64;
        let x: Vec<f64> = (0..n).map(|i| ((i * 7 % 13) as f64 - 6.0) / 6.0).collect();
        let (mut re, mut im) = (x.clone(), vec![0.0; n]);
        fft(&mut re, &mut im);
        for k in 0..n {
            let (mut sr, mut si) = (0.0, 0.0);
            for (i, &v) in x.iter().enumerate() {
                let a = -2.0 * PI * (k * i) as f64 / n as f64;
                sr += v * a.cos();
                si += v * a.sin();
            }
            assert!(
                (re[k] - sr).abs() < 1e-9 && (im[k] - si).abs() < 1e-9,
                "bin {k}"
            );
        }
    }

    #[test]
    fn silence_is_level_0_everywhere() {
        assert_eq!(levels(&vec![0.0; N]), [0.0; COLUMNS]);
    }

    // Phase 9 plan, Task 1: a −20 dBFS tone at a column's centre lights that column and no
    // column two or more away, except column 1's, which reaches column 3: 2048 samples give bins
    // 23.4 Hz wide, wider than the lowest bands (owner, 2026-11-05). From column 5 up, a tone
    // lights its own column only.
    #[test]
    fn a_tone_lights_its_column_and_no_column_two_away() {
        for c in 1..=COLUMNS {
            let l = levels(&tone_window(centre(c), -20.0));
            assert!(l[c - 1] > 0.5, "column {c}: {l:?}");
            let reach = match c {
                1 => 2,
                2..=4 => 1,
                _ => 0,
            };
            for (i, &x) in l.iter().enumerate() {
                if (i + 1).abs_diff(c) > reach {
                    assert_eq!(x, 0.0, "a tone at column {c} lit column {}: {l:?}", i + 1);
                }
            }
        }
    }

    // Points 5 and 6: a bar falls by b/72 a tick; a peak holds, then falls, never below 0.
    #[test]
    fn bars_fall_by_their_stop_and_peaks_stay_at_or_above_the_bar_until_they_fall() {
        for &(_, b, g) in &STOPS {
            let mut s = Spectrum::enter();
            let mut full = [0.0; COLUMNS];
            full[0] = 1.0;
            s.update(&full, b, g);
            let silent = [0.0; COLUMNS];
            let mut previous = 1.0;
            for _ in 0..40 {
                s.update(&silent, b, g);
                let bar = s.bars[0];
                assert!((previous - bar - b / 72.0).abs() < 1e-12 || bar == 0.0);
                assert!(s.peaks[0] >= 0.0);
                previous = bar;
            }
            assert_eq!(s.bars[0], 0.0);
        }
    }

    // Point 6 (owner, 2026-11-05): a peak fall change keeps a falling peak where it is, and it
    // falls on from there, from rest, at the new speed.
    #[test]
    fn a_peak_fall_change_keeps_a_falling_peak_in_place() {
        let mut full = [0.0; COLUMNS];
        full[0] = 1.0;
        let silent = [0.0; COLUMNS];
        let (b, fast, slow) = (12.0, 1.6, 1.05);
        let mut s = Spectrum::enter();
        s.update(&full, b, fast);
        for _ in 0..3 {
            s.update(&silent, b, fast);
        }
        let before = s.peaks[0];
        s.update(&silent, b, slow);
        let fallen = (3.0 / 3840.0) * (slow.powf(10.0 / 3.0) - 1.0) / (slow - 1.0);
        assert!((s.peaks[0] - (before - fallen)).abs() < 1e-12);
        assert!(s.peaks[0] < before && s.peaks[0] > s.bars[0]);
    }

    // Phase 9 plan, Task 1: peaks off, every column is a solid run from the floor; peaks on, at
    // most one pixel above it.
    #[test]
    fn every_drawn_column_is_at_least_one_row_and_a_dot_sits_above_its_bar() {
        for s in signals() {
            let levels: Vec<[f64; COLUMNS]> = s
                .tick_levels()
                .iter()
                .map(|l| l.map(|x| x.clamp(0.0, 1.0)))
                .collect();
            let mut spectrum = Spectrum::enter();
            for tick in &levels {
                spectrum.update(tick, 12.0, 1.1);
                for h in [12, 6] {
                    for (bar, dot) in spectrum.draw(h, true) {
                        assert!((1..=h).contains(&bar), "{}", s.name);
                        if let Some(dot) = dot {
                            assert!(dot > bar && dot <= h, "{}", s.name);
                        }
                    }
                }
            }
        }
    }
}
