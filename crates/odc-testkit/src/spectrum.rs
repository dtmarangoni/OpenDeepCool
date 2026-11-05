//! The spectrum's test files, `docs/spec/frames/spectrum/` (README, Spectrum): the test signals
//! of `signals.txt` made into samples as the file states it, the rows each one draws
//! (`expected.txt`), and the Dancer's pose at every tick (`dancer.txt`). A file that doesn't parse
//! panics, naming its file and line, as the expected frames do.

use std::f64::consts::PI;
use std::fs;
use std::path::Path;

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/spec/frames/spectrum"
);

/// Samples per second.
pub const RATE: f64 = 48_000.0;
/// Samples per 100 ms tick.
pub const TICK: usize = 4800;
/// The analysis window: the last 2048 samples of a tick.
pub const WINDOW: usize = 2048;
/// Columns.
pub const COLUMNS: usize = 14;

/// One stretch of a signal, so many ticks long.
#[derive(Clone, Debug, PartialEq)]
enum Segment {
    Silence(usize),
    /// Tones added, each a frequency in Hz and a peak amplitude in dBFS.
    Tones(usize, Vec<(f64, f64)>),
    /// The file's white noise, at a peak amplitude in dBFS.
    Noise(usize, f64),
}

/// A test signal of `signals.txt`.
#[derive(Clone, Debug, PartialEq)]
pub struct Signal {
    pub name: String,
    /// The dB its amplitudes were moved by, so no value it draws lies near a rounding boundary.
    moved: f64,
    segments: Vec<Segment>,
}

impl Signal {
    /// Its length in ticks.
    pub fn ticks(&self) -> usize {
        self.segments
            .iter()
            .map(|s| match s {
                Segment::Silence(t) | Segment::Tones(t, _) | Segment::Noise(t, _) => *t,
            })
            .sum()
    }

    /// Its samples, one channel (both alike), as `signals.txt` states them: a tone's sample *n*,
    /// counted from the signal's first sample, is A · sin(2π f n / 48 000), A = 10^((dBFS +
    /// moved) / 20), computed in f64 and stored as f32, tones on one line added; the noise is
    /// x ← (1103515245 x + 12345) mod 2³¹ from x = 1 at the signal's start, the sample
    /// (x ÷ 2³⁰ − 1) · A.
    pub fn samples(&self) -> Vec<f32> {
        let amplitude = |db: f64| 10f64.powf((db + self.moved) / 20.0);
        let mut out: Vec<f32> = Vec::with_capacity(self.ticks() * TICK);
        let mut x: u64 = 1;
        for segment in &self.segments {
            match segment {
                Segment::Silence(t) => out.resize(out.len() + t * TICK, 0.0),
                Segment::Tones(t, tones) => {
                    for _ in 0..t * TICK {
                        let n = out.len() as f64;
                        let sample: f64 = tones
                            .iter()
                            .map(|&(f, db)| amplitude(db) * (2.0 * PI * f * n / RATE).sin())
                            .sum();
                        out.push(sample as f32);
                    }
                }
                Segment::Noise(t, db) => {
                    for _ in 0..t * TICK {
                        x = (1_103_515_245 * x + 12_345) % (1 << 31);
                        out.push(
                            ((x as f64 / f64::from(1u32 << 30) - 1.0) * amplitude(*db)) as f32,
                        );
                    }
                }
            }
        }
        out
    }

    /// The window of every tick: the last 2048 samples of its 100 ms.
    pub fn windows(&self) -> Vec<[f32; WINDOW]> {
        let samples = self.samples();
        (1..=self.ticks())
            .map(|n| {
                samples[n * TICK - WINDOW..n * TICK]
                    .try_into()
                    .expect("2048 samples")
            })
            .collect()
    }

    /// Whether one of the Dancer's beat trains, which `expected.txt` leaves out.
    pub fn is_beat_train(&self) -> bool {
        self.name.starts_with("beat train")
    }
}

/// Every signal of `signals.txt`, in the file's order.
///
/// # Panics
///
/// When the file can't be read or doesn't parse, naming the line.
pub fn signals() -> Vec<Signal> {
    let text = read("signals.txt");
    let mut signals: Vec<Signal> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let at = |what: &str| -> ! { panic!("signals.txt:{}: {what}: {line:?}", i + 1) };
        if let Some(name) = line.strip_prefix("## ") {
            signals.push(Signal {
                name: name.to_owned(),
                moved: 0.0,
                segments: Vec::new(),
            });
            continue;
        }
        if line.is_empty() || line.starts_with("# ") || signals.is_empty() {
            continue;
        }
        let signal = signals.last_mut().expect("a signal");
        let (word, rest) = line.split_once(' ').unwrap_or_else(|| at("a line"));
        if word == "moved" {
            signal.moved = rest.parse().unwrap_or_else(|_| at("a dB value"));
            continue;
        }
        let (ticks, list) = rest.split_once(": ").unwrap_or((rest, ""));
        let ticks: usize = ticks.parse().unwrap_or_else(|_| at("a tick count"));
        let db = |text: &str| -> f64 {
            text.strip_suffix(" dBFS")
                .and_then(|d| d.parse().ok())
                .unwrap_or_else(|| at("a level in dBFS"))
        };
        signal.segments.push(match word {
            "silence" => Segment::Silence(ticks),
            "noise" => Segment::Noise(ticks, db(list)),
            "tones" => Segment::Tones(
                ticks,
                list.split(", ")
                    .map(|tone| {
                        let (f, level) = tone.split_once(" Hz ").unwrap_or_else(|| at("a tone"));
                        (f.parse().unwrap_or_else(|_| at("a frequency")), db(level))
                    })
                    .collect(),
            ),
            _ => at("silence, tones, noise or moved"),
        });
    }
    signals
}

/// The signal named `name`.
///
/// # Panics
///
/// When `signals.txt` has no such signal.
pub fn signal(name: &str) -> Signal {
    signals()
        .into_iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("signals.txt has no signal {name:?}"))
}

/// One section of `expected.txt`: a signal's spectrum rows at every tick for one height and
/// set of Music settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expected {
    pub heading: String,
    pub signal: String,
    /// *H*, the area's height in rows.
    pub h: u8,
    pub peaks: bool,
    /// The stops' names; with the peaks off, the file gives no peak stop, and any draws alike.
    pub bar_fall: String,
    pub peak_fall: Option<String>,
    pub ticks: Vec<Rows>,
}

/// One tick's columns: each bar's height in rows from the floor, and with peaks on each dot's
/// height, `None` where no dot is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rows {
    pub bars: [u8; COLUMNS],
    pub dots: Option<[Option<u8>; COLUMNS]>,
}

/// Every section of `expected.txt`, in the file's order.
///
/// # Panics
///
/// When the file can't be read or doesn't parse, naming the line.
pub fn expected() -> Vec<Expected> {
    let text = read("expected.txt");
    let mut sections: Vec<Expected> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let at = |what: &str| -> ! { panic!("expected.txt:{}: {what}: {line:?}", i + 1) };
        if let Some(heading) = line.strip_prefix("## ") {
            let parts: Vec<&str> = heading.split(" · ").collect();
            let (signal, h, peaks, bars, tops) = match parts.as_slice() {
                [signal, h, "peaks off", bars] => (signal, h, false, bars, None),
                [signal, h, "peaks on", bars, tops] => (signal, h, true, bars, Some(tops)),
                _ => at("a heading"),
            };
            sections.push(Expected {
                heading: heading.to_owned(),
                signal: (*signal).to_owned(),
                h: h.strip_prefix("H ")
                    .and_then(|h| h.parse().ok())
                    .unwrap_or_else(|| at("H and a height")),
                peaks,
                bar_fall: bars
                    .strip_prefix("bars ")
                    .unwrap_or_else(|| at("a bar stop"))
                    .to_owned(),
                peak_fall: tops.map(|t| {
                    t.strip_prefix("peaks ")
                        .unwrap_or_else(|| at("a peak stop"))
                        .to_owned()
                }),
                ticks: Vec::new(),
            });
            continue;
        }
        if line.is_empty() || line.starts_with("# ") || sections.is_empty() {
            continue;
        }
        let section = sections.last_mut().expect("a section");
        let (n, rows) = line.split_once(": ").unwrap_or_else(|| at("a tick line"));
        if n.parse() != Ok(section.ticks.len()) {
            at("the next tick");
        }
        let heights = |text: &str| -> [Option<u8>; COLUMNS] {
            let chars: Vec<char> = text.chars().collect();
            if chars.len() != COLUMNS {
                at("14 heights");
            }
            std::array::from_fn(|c| match chars[c] {
                '.' => None,
                h => Some(
                    h.to_digit(16)
                        .and_then(|h| u8::try_from(h).ok())
                        .filter(|&h| (1..=12).contains(&h))
                        .unwrap_or_else(|| at("a height from 1 to c")),
                ),
            })
        };
        let (bars, dots) = match (section.peaks, rows.split_once(' ')) {
            (false, None) => (rows, None),
            (true, Some((bars, dots))) => (bars, Some(heights(dots))),
            _ => at("bars, and dots when the peaks are on"),
        };
        let bars = heights(bars).map(|h| h.unwrap_or_else(|| at("a bar in every column")));
        section.ticks.push(Rows { bars, dots });
    }
    sections
}

/// One sequence of `dancer.txt`: a signal, the ticks before which the Dancer also resets, and
/// its pose at every tick from a reset at tick 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dance {
    pub heading: String,
    pub signal: String,
    pub resets: Vec<usize>,
    pub poses: Vec<u8>,
}

/// Every sequence of `dancer.txt`, in the file's order.
///
/// # Panics
///
/// When the file can't be read or doesn't parse, naming the line.
pub fn dances() -> Vec<Dance> {
    let text = read("dancer.txt");
    let mut dances: Vec<Dance> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let at = |what: &str| -> ! { panic!("dancer.txt:{}: {what}: {line:?}", i + 1) };
        if let Some(heading) = line.strip_prefix("## ") {
            let (signal, resets) = match heading.split_once(" · resets at ticks ") {
                Some((signal, ticks)) => (
                    signal,
                    ticks
                        .split(" and ")
                        .flat_map(|t| t.split(", "))
                        .map(|t| t.parse().unwrap_or_else(|_| at("a tick")))
                        .collect(),
                ),
                None => (heading, Vec::new()),
            };
            dances.push(Dance {
                heading: heading.to_owned(),
                signal: signal.to_owned(),
                resets,
                poses: Vec::new(),
            });
            continue;
        }
        if line.is_empty() || line.starts_with("# ") || dances.is_empty() {
            continue;
        }
        let dance = dances.last_mut().expect("a sequence");
        if !dance.poses.is_empty() {
            at("one line of poses");
        }
        dance.poses = line
            .split(' ')
            .map(|p| {
                p.parse()
                    .ok()
                    .filter(|&p: &u8| p <= 10)
                    .unwrap_or_else(|| at("a pose from 0 to 10"))
            })
            .collect();
    }
    dances
}

fn read(name: &str) -> String {
    let path = Path::new(DIR).join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each signal's tick count is what expected.txt and dancer.txt give it, and every section of
    // those names a signal.
    #[test]
    fn the_three_files_agree() {
        let signals = signals();
        assert_eq!(signals.len(), 89);
        assert_eq!(signals.iter().filter(|s| s.is_beat_train()).count(), 8);
        let ticks = |name: &str| {
            signals
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("no signal {name:?}"))
                .ticks()
        };
        let expected = expected();
        // H 12 and 6, peaks off with 5 bar stops and on with 25 pairs, per signal.
        assert_eq!(expected.len(), 81 * 2 * (5 + 25));
        for section in &expected {
            assert_eq!(
                section.ticks.len(),
                ticks(&section.signal),
                "{}",
                section.heading
            );
            assert!([12, 6].contains(&section.h));
            for rows in &section.ticks {
                assert!(rows.bars.iter().all(|&b| b <= section.h));
            }
        }
        let dances = dances();
        assert_eq!(dances.len(), 89 + 8);
        for dance in &dances {
            assert_eq!(dance.poses.len(), ticks(&dance.signal), "{}", dance.heading);
        }
        assert_eq!(
            dances.iter().filter(|d| d.resets == [7, 19]).count(),
            8,
            "each beat train once more with resets"
        );
    }

    // The samples follow the file's formula: a tone's phase counts from the signal's start, and
    // the noise generator starts at x = 1.
    #[test]
    fn samples_follow_the_formula() {
        let burst = signal("burst train, column 7, -20 dBFS, 100 ms on, 400 ms off");
        let samples = burst.samples();
        assert_eq!(samples.len(), burst.ticks() * TICK);
        let a = 10f64.powf(-20.0 / 20.0);
        let f = 645.891_059;
        let n = 5 * TICK + 17;
        let expected = (a * (2.0 * PI * f * n as f64 / RATE).sin()) as f32;
        assert_eq!(
            samples[n], expected,
            "the second burst keeps the signal's phase"
        );
        assert_eq!(samples[TICK + 17], 0.0);
        let noise = signal("white noise, 0 dBFS").samples();
        let x: u64 = 1;
        let first = ((1_103_515_245 * x + 12_345) % (1 << 31)) as f64 / f64::from(1u32 << 30) - 1.0;
        assert_eq!(noise[0], first as f32);
        let windows = burst.windows();
        assert_eq!(windows.len(), burst.ticks());
        assert_eq!(windows[0][..], samples[TICK - WINDOW..TICK]);
    }
}
