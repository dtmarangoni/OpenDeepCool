//! OpenDeepCool's spectrum (Phase 9 plan, Task 2; `docs/spec/music.md`, "OpenDeepCool's spectrum,
//! precisely"): every signal of `spectrum/expected.txt` through `analyse` and `Spectrum`, the
//! analysis's and the bars' and peaks' properties, and the frame they're drawn in.

use std::collections::HashMap;
use std::f64::consts::PI;

use odc_protocol::Frame;
use odc_render::{
    COLUMNS, Column, Columns, Mode, MusicSettings, Spectrum, Stop, WINDOW, analyse,
    spectrum_height, with_spectrum,
};
use odc_testkit::spectrum::{expected, signals};
use proptest::prelude::*;

const ALL_STOPS: [Stop; 5] = Stop::ALL;

fn stop(name: &str) -> Stop {
    Stop::from_name(name).unwrap_or_else(|| panic!("{name:?} isn't a stop"))
}

/// Every signal's column levels at every tick, through `analyse`.
fn levels() -> HashMap<String, Vec<[f64; COLUMNS]>> {
    signals()
        .into_iter()
        .filter(|s| !s.is_beat_train())
        .map(|s| {
            let levels = s.windows().iter().map(analyse).collect();
            (s.name, levels)
        })
        .collect()
}

#[test]
fn every_signal_draws_its_expected_rows_at_every_tick_height_and_setting() {
    let levels = levels();
    let sections = expected();
    assert_eq!(sections.len(), 81 * 2 * (5 + 25));
    for section in &sections {
        let settings = MusicSettings {
            peaks: section.peaks,
            bar_fall: stop(&section.bar_fall),
            // With the dots off, the peaks still run, at any stop: none is drawn.
            peak_fall: section.peak_fall.as_deref().map_or(Stop::Slow, stop),
        };
        let mut spectrum = Spectrum::enter();
        for (n, (tick, rows)) in levels[&section.signal]
            .iter()
            .zip(&section.ticks)
            .enumerate()
        {
            spectrum.update(tick, &settings);
            let drawn = spectrum.draw(section.h, section.peaks);
            let bars = drawn.map(|c| c.bar);
            assert_eq!(bars, rows.bars, "{}, tick {n}: bars", section.heading);
            let dots = drawn.map(|c| c.dot);
            assert_eq!(
                dots,
                rows.dots.unwrap_or([None; COLUMNS]),
                "{}, tick {n}: dots",
                section.heading
            );
        }
    }
}

#[test]
fn silence_is_level_0_in_every_column() {
    assert_eq!(analyse(&[0.0; WINDOW]), [0.0; COLUMNS]);
}

/// A tone at `f` Hz and `db` dBFS, its phase from the window's first sample.
fn tone(f: f64, db: f64) -> [f32; WINDOW] {
    let a = 10f64.powf(db / 20.0);
    std::array::from_fn(|n| (a * (2.0 * PI * f * n as f64 / 48_000.0).sin()) as f32)
}

/// The loudest column, from 1; the leftmost of equals.
fn loudest(levels: &[f64; COLUMNS]) -> usize {
    let top = levels.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    levels.iter().position(|&l| l == top).expect("a column") + 1
}

#[test]
fn a_tone_moving_from_40_hz_to_16_khz_moves_its_loudest_column_from_1_to_14() {
    let steps = 400;
    let columns: Vec<usize> = (0..steps)
        .map(|j| {
            // From 40 Hz up to just below 16 kHz, evenly on the log scale the bands use.
            let f = 40.0 * 400f64.powf(f64::from(j) / f64::from(steps));
            loudest(&analyse(&tone(f, -20.0)))
        })
        .collect();
    assert_eq!(columns.first(), Some(&1));
    assert_eq!(columns.last(), Some(&14));
    assert!(
        columns.windows(2).all(|w| w[0] <= w[1]),
        "monotonic: {columns:?}"
    );
    assert!((1..=14).all(|c| columns.contains(&c)), "every column");
}

fn settings() -> impl Strategy<Value = MusicSettings> {
    (any::<bool>(), 0..5_usize, 0..5_usize).prop_map(|(peaks, b, p)| MusicSettings {
        peaks,
        bar_fall: ALL_STOPS[b],
        peak_fall: ALL_STOPS[p],
    })
}

/// Levels as music gives them: often 0, often loud, and anything between.
fn level() -> impl Strategy<Value = f64> {
    prop_oneof![Just(0.0), Just(1.0), 0.0..=1.0_f64]
}

fn ticks() -> impl Strategy<Value = Vec<[f64; COLUMNS]>> {
    prop::collection::vec(prop::array::uniform14(level()), 1..60)
}

/// A bar's fall per tick at `stop`: Winamp's bar value over 72 (music.md points 4 and 5).
fn fall(stop: Stop) -> f64 {
    let b = match stop {
        Stop::Slower => 3.0,
        Stop::Slow => 6.0,
        Stop::Moderate => 12.0,
        Stop::Fast => 16.0,
        Stop::Faster => 32.0,
    };
    b / 72.0
}

/// Checks one drawn column against the layout rules: a bar at least 1 row, within `h`; a dot only
/// with the peaks on, above the bar and within the area.
fn check_column(column: Column, h: u8, peaks: bool) -> Result<(), TestCaseError> {
    prop_assert!((1..=h).contains(&column.bar));
    if let Some(dot) = column.dot {
        prop_assert!(peaks);
        prop_assert!(dot > column.bar && dot <= h);
    }
    Ok(())
}

proptest! {
    #[test]
    fn bars_and_peaks_follow_their_settings(
        ticks in ticks(),
        changes in prop::collection::vec(settings(), 1..4),
    ) {
        let mut spectrum = Spectrum::enter();
        for (n, levels) in ticks.iter().enumerate() {
            // A setting change, never a reset, every few ticks (point 7).
            let settings = changes[n * changes.len() / ticks.len()];
            let (bars, peaks) = (spectrum.bars(), spectrum.peaks());
            spectrum.update(levels, &settings);
            for c in 0..COLUMNS {
                let bar = spectrum.bars()[c];
                // A bar rises to a higher level at once; it never falls faster than its setting,
                // nor below the level.
                prop_assert!(bar >= levels[c]);
                prop_assert!(bar >= bars[c] - fall(settings.bar_fall) - 1e-12);
                prop_assert!(bar <= bars[c].max(levels[c]));
                // A bar at its peak or above sets the peak; otherwise the peak only falls, and
                // never below 0.
                let peak = spectrum.peaks()[c];
                if bar >= peaks[c] {
                    prop_assert_eq!(peak, bar);
                } else {
                    prop_assert!(peak <= peaks[c] && peak >= 0.0);
                }
            }
            for h in [12, 6] {
                for column in spectrum.draw(h, settings.peaks) {
                    check_column(column, h, settings.peaks)?;
                }
            }
        }
    }
}

#[test]
fn a_peak_holds_then_falls_faster_and_faster() {
    for stop in ALL_STOPS {
        let settings = MusicSettings {
            peak_fall: stop,
            bar_fall: Stop::Faster,
            peaks: true,
        };
        let mut spectrum = Spectrum::enter();
        spectrum.update(&[1.0; COLUMNS], &settings);
        let mut previous = (1.0, 0.0);
        for _ in 0..60 {
            spectrum.update(&[0.0; COLUMNS], &settings);
            let peak = spectrum.peaks()[0];
            let fell = previous.0 - peak;
            assert!(
                peak == 0.0 || fell > previous.1,
                "{stop:?}: {fell} after {previous:?}"
            );
            previous = (peak, fell);
        }
        assert_eq!(spectrum.peaks()[0], 0.0, "{stop:?}: down within 6 s");
    }
}

// music.md point 6 (owner, 2026-11-05): a peak fall change keeps a falling peak where it is, and
// it falls on from there, from rest, at the new speed; a bar fall change keeps every level.
#[test]
fn a_fall_change_keeps_every_level_and_a_falling_peak_falls_on_from_rest() {
    for (from, to) in [(Stop::Faster, Stop::Slower), (Stop::Slower, Stop::Faster)] {
        let fast = MusicSettings {
            peaks: true,
            bar_fall: Stop::Moderate,
            peak_fall: from,
        };
        let mut spectrum = Spectrum::enter();
        spectrum.update(&[1.0; COLUMNS], &fast);
        for _ in 0..2 {
            spectrum.update(&[0.0; COLUMNS], &fast);
        }
        let (bar, peak) = (spectrum.bars()[0], spectrum.peaks()[0]);
        assert!(peak > bar, "{from:?}: the peak is falling above its bar");
        let changed = MusicSettings {
            peak_fall: to,
            bar_fall: Stop::Slower,
            ..fast
        };
        spectrum.update(&[0.0; COLUMNS], &changed);
        // One tick of the new stops, from where each was.
        let g = if to == Stop::Slower { 1.05 } else { 1.6 };
        let fallen = 3.0 / 3840.0 * (f64::powf(g, 10.0 / 3.0) - 1.0) / (g - 1.0);
        assert!(
            (spectrum.peaks()[0] - (peak - fallen)).abs() < 1e-12,
            "{from:?} → {to:?}"
        );
        assert!((spectrum.bars()[0] - (bar - 3.0 / 72.0)).abs() < 1e-12);
    }
}

#[test]
fn the_spectrum_area_is_12_rows_in_mode_13_and_6_in_14_to_22() {
    for mode in Mode::all() {
        let h = match mode.number() {
            13 => Some(12),
            14..=22 => Some(6),
            _ => None,
        };
        assert_eq!(spectrum_height(mode), h, "mode {}", mode.number());
    }
}

/// Columns of random heights within `h`, each dot above its bar.
fn columns(h: u8) -> impl Strategy<Value = Columns> {
    prop::array::uniform14((1..=h, prop::option::of(0..=h))).prop_map(move |cs| {
        cs.map(|(bar, dot)| Column {
            bar,
            dot: dot.map(|d| d.max(bar + 1)).filter(|&d| d <= h),
        })
    })
}

fn frame() -> impl Strategy<Value = Frame> {
    prop::array::uniform14(prop::array::uniform14(any::<bool>())).prop_map(|rows| {
        let mut frame = Frame::blank();
        for (r, row) in rows.iter().enumerate() {
            for (c, &on) in row.iter().enumerate() {
                frame.set(r + 1, c + 1, on);
            }
        }
        frame
    })
}

proptest! {
    // The rows above the area are the top's; in the area each column is its bar from the floor,
    // solid, then at most its one dot (music.md, Spectrum: layout).
    #[test]
    fn a_spectrum_is_drawn_from_the_floor_below_the_top(
        top in frame(),
        (h, columns) in prop_oneof![Just(12_u8), Just(6_u8)].prop_flat_map(|h| (Just(h), columns(h))),
    ) {
        let frame = with_spectrum(&top, &columns, h);
        let first = 14 - usize::from(h);
        for r in 1..=first {
            for c in 1..=14 {
                prop_assert_eq!(frame.get(r, c), top.get(r, c));
            }
        }
        for (c, column) in columns.iter().enumerate() {
            for height in 1..=usize::from(h) {
                let lit = height <= usize::from(column.bar)
                    || column.dot.map(usize::from) == Some(height);
                prop_assert_eq!(frame.get(15 - height, c + 1), lit);
            }
        }
    }
}
