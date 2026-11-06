//! Layer 1, the matrix (Phase 9 plan, Tasks 6–9): every case of each drawn Music mode's
//! `docs/spec/frames/mode-0NN.txt`, in silence, its `?` pixels resolved as the floor line the files
//! draw: mode 13's spectrum alone, an emoji's whole loop, a value over the spectrum, the Dancer's
//! poses. And every signal of `spectrum/expected.txt` at *H* 12 as mode 13 draws it, rows 1–2
//! always off, and at *H* 6 under each upright case of each drawn mode with a top, an emoji's loop
//! or a value: the top as the mode's file draws it, the spectrum's rows as `expected.txt` says.

mod common;

use std::collections::HashMap;

use common::{drawn_modes, unit, values};
use odc_protocol::{Frame, Rotation};
use odc_render::{
    COLUMNS, Column, EmojiLoop, Kind, Marquee, Mode, MusicSettings, Pose, Spectrum, Stop, analyse,
    spectrum_height, value_picture, with_spectrum,
};
use odc_testkit::diff;
use odc_testkit::expected::{self, Case, load_mode};
use odc_testkit::spectrum::{Expected, Rows, expected, signals};

/// Silence: every bar 1 row high, the floor line.
const FLOOR: [Column; COLUMNS] = [Column { bar: 1, dot: None }; COLUMNS];

/// `top` over the floor line of `mode`'s spectrum area, when it has one.
fn in_silence(mode: Mode, top: Frame) -> Frame {
    match spectrum_height(mode) {
        Some(h) => with_spectrum(&top, &FLOOR, h),
        None => top,
    }
}

/// The renderer's top of `case` of `mode`, before rotation, a frame a tick from its start: nothing
/// in mode 13, an emoji's loop, a value's picture through the `Marquee`, the Dancer's pose.
fn top_of(mode: Mode, case: &Case) -> Box<dyn FnMut() -> Frame> {
    match mode.number() {
        13 => Box::new(Frame::blank),
        14..=17 => {
            let mut emoji = EmojiLoop::of(mode).expect("an emoji mode");
            Box::new(move || emoji.tick())
        }
        23 => {
            let pose = Pose::new(case.pose.expect("a pose")).expect("a pose from 0 to 10");
            Box::new(move || pose.frame())
        }
        _ => {
            let shown = case.unit.map_or(odc_render::Unit::Celsius, unit);
            let picture = value_picture(mode, &values(&case.readings), shown).expect("a value");
            let mut marquee = Marquee::new(picture);
            Box::new(move || marquee.tick())
        }
    }
}

/// `case`'s pictures a tick each over one period, from its start: a still picture once, a
/// scrolling value's cycle, an emoji's loop a picture a tick for its duration.
fn period(case: &Case) -> Vec<&expected::Picture> {
    match &case.kind {
        expected::Kind::Static(p) => vec![p],
        expected::Kind::Scrolling(cycle) => cycle.iter().collect(),
        expected::Kind::Loop(steps) => steps
            .iter()
            .flat_map(|(duration, p)| {
                let n = usize::try_from(duration.as_millis() / 100).expect("a short duration");
                std::iter::repeat_n(p, n)
            })
            .collect(),
    }
}

/// Draws `case` of `mode` from its start in silence and compares every frame: a still picture
/// for three ticks, a scrolling value over its whole cycle and its first frame again, an emoji's
/// loop a picture a tick for its duration and its first picture again.
fn check(mode: Mode, case: &Case, file: &str) {
    let context = |what: String| format!("{file}, {}, {what}", case.heading);
    let expected: Vec<expected::Picture> = match &case.kind {
        expected::Kind::Static(p) => vec![p.in_silence(); 3],
        expected::Kind::Scrolling(cycle) => cycle
            .iter()
            .chain(cycle.first())
            .map(expected::Picture::in_silence)
            .collect(),
        expected::Kind::Loop(steps) => {
            let mut ticks: Vec<expected::Picture> = steps
                .iter()
                .flat_map(|(duration, p)| {
                    let n = usize::try_from(duration.as_millis() / 100).expect("a short duration");
                    std::iter::repeat_n(p.in_silence(), n)
                })
                .collect();
            ticks.extend(ticks.first().cloned());
            ticks
        }
    };
    let mut next = top_of(mode, case);
    for (k, want) in expected.iter().enumerate() {
        let frame = in_silence(mode, next()).rotated(case.rotation);
        diff::assert_matches(want, &frame, &context(format!("frame {k}")));
    }
}

#[test]
fn every_case_of_each_drawn_music_mode_matches_its_expected_frames_in_silence() {
    let modes = drawn_modes(Kind::Music);
    assert!(!modes.is_empty(), "no Music mode drawn yet");
    for mode in modes {
        let name = format!("mode-{:03}.txt", mode.number());
        let file = load_mode(mode.number());
        for case in &file.cases {
            check(mode, case, &name);
        }
        println!("{name}: {} cases", file.cases.len());
    }
}

/// Each column's lit heights in a spectrum area `h` rows high, from the floor.
fn heights(frame: &Frame, h: u8) -> Vec<Vec<u8>> {
    (1..=14)
        .map(|c| {
            (1..=h)
                .filter(|&height| frame.get(15 - usize::from(height), c))
                .collect()
        })
        .collect()
}

/// Each test signal's levels, one per tick, by name, analysed once for every section that plays
/// it; the beat trains are the Dancer's.
fn levels() -> HashMap<String, Vec<[f64; COLUMNS]>> {
    signals()
        .into_iter()
        .filter(|s| !s.is_beat_train())
        .map(|s| (s.name.clone(), s.windows().iter().map(analyse).collect()))
        .collect()
}

/// The Music settings a section of `expected.txt` names.
fn settings(section: &Expected) -> MusicSettings {
    let stop = |name: &str| Stop::from_name(name).expect("a stop");
    MusicSettings {
        peaks: section.peaks,
        bar_fall: stop(&section.bar_fall),
        peak_fall: section.peak_fall.as_deref().map_or(Stop::Slow, stop),
    }
}

/// The lit heights `rows` give each column, bar and dot.
fn wanted(rows: &Rows) -> Vec<Vec<u8>> {
    let dots = rows.dots.unwrap_or([None; 14]);
    rows.bars
        .iter()
        .zip(dots)
        .map(|(&bar, dot)| (1..=bar).chain(dot).collect())
        .collect()
}

// Mode 13 (Phase 9 plan, Task 6): every signal at H 12 with every listed peaks setting and stop,
// drawn on the whole frame: rows 1–2 off, the spectrum's rows `expected.txt`'s. A dot right above
// its bar looks like a taller bar, so columns compare by their lit heights.
#[test]
fn every_signal_draws_its_rows_in_mode_13_with_rows_1_and_2_off() {
    let thirteen = Mode::new(13).expect("mode 13");
    if !drawn_modes(Kind::Music).contains(&thirteen) {
        return;
    }
    let levels = levels();
    let mut sections = 0;
    for section in expected().iter().filter(|s| s.h == 12) {
        let settings = settings(section);
        let mut spectrum = Spectrum::enter();
        for (n, (tick, rows)) in levels[&section.signal]
            .iter()
            .zip(&section.ticks)
            .enumerate()
        {
            spectrum.update(tick, &settings);
            let frame = with_spectrum(&Frame::blank(), &spectrum.draw(12, settings.peaks), 12);
            assert!(
                (1..=2).all(|r| (1..=14).all(|c| !frame.get(r, c))),
                "{}, tick {n}: rows 1–2 lit",
                section.heading
            );
            assert_eq!(
                heights(&frame, 12),
                wanted(rows),
                "{}, tick {n}",
                section.heading
            );
        }
        sections += 1;
    }
    assert_eq!(sections, 81 * 30);
}

// Modes 14–22 (Phase 9 plan, Tasks 7–8): every signal at H 6 with every listed peaks setting and
// stop, under each upright case of each drawn mode with a top, from its start, on the whole frame:
// rows 1–8 the emoji's loop or the value as the mode's file draws them, the spectrum's rows
// `expected.txt`'s. The music drives neither, so they move a picture a tick whatever the sound,
// and only the value moves in the value area.
#[test]
fn every_signal_draws_its_rows_under_each_drawn_top() {
    let modes: Vec<Mode> = drawn_modes(Kind::Music)
        .into_iter()
        .filter(|&mode| spectrum_height(mode) == Some(6))
        .collect();
    if modes.is_empty() {
        return;
    }
    let levels = levels();
    let upright = Rotation::from_setting(0).expect("0");
    for mode in modes {
        let name = format!("mode-{:03}.txt", mode.number());
        let file = load_mode(mode.number());
        let cases: Vec<&Case> = file
            .cases
            .iter()
            .filter(|case| case.rotation == upright)
            .collect();
        assert!(!cases.is_empty(), "{name}: no upright case");
        for case in cases {
            let pictures = period(case);
            let mut sections = 0;
            for section in expected().iter().filter(|s| s.h == 6) {
                let settings = settings(section);
                let mut spectrum = Spectrum::enter();
                let mut top = top_of(mode, case);
                for (n, (tick, rows)) in levels[&section.signal]
                    .iter()
                    .zip(&section.ticks)
                    .enumerate()
                {
                    spectrum.update(tick, &settings);
                    let frame = with_spectrum(&top(), &spectrum.draw(6, settings.peaks), 6);
                    let context =
                        format!("{name}, {}, {}, tick {n}", case.heading, section.heading);
                    diff::assert_matches(pictures[n % pictures.len()], &frame, &context);
                    assert_eq!(heights(&frame, 6), wanted(rows), "{context}");
                }
                sections += 1;
            }
            assert_eq!(sections, 81 * 30, "{name}, {}", case.heading);
        }
    }
}
