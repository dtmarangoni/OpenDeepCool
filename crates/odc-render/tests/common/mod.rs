//! What the tests of each kind of mode share: the modes drawn so far, a case's readings as
//! `Values`, and the replay of the scenarios. Each test file uses part of it.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::OnceLock;

use odc_protocol::{Frame, Rotation};
use odc_render::{
    Dancer, EmojiLoop, Kind, Marquee, Mode, MusicSettings, Spectrum, Stop, Unit, Values, WINDOW,
    analyse, draws, mode_info, picture, spectrum_height, value_picture, with_spectrum,
};
use odc_sensors::{Exact, Metric};
use odc_testkit::diff;
use odc_testkit::expected::{self, Change, Picture, Reset, Scenario, every_metric, mode_scenarios};
use odc_testkit::spectrum::signals;

/// The modes of `kind` this build draws (Phase 7 plan, Task 3; Phase 8 plan, Tasks 4–8: one per
/// commit).
pub fn drawn_modes(kind: Kind) -> Vec<Mode> {
    Mode::all()
        .filter(|&mode| mode_info(mode).kind == kind && draws(mode))
        .collect()
}

/// Every metric's value: the readings named, and a filler of its own for every other metric, so
/// drawing the wrong metric shows (AGENTS.md rule 5).
pub fn values(readings: &[(String, Option<Exact>)]) -> Values {
    let mut values = Values::new();
    for (metric, value) in every_metric(readings) {
        values.set(metric, value);
    }
    values
}

/// An expected picture of a mode without a spectrum, turned.
pub fn turned(picture: &Picture, rotation: Rotation) -> Frame {
    picture
        .to_frame()
        .expect("a scenario's frame has no spectrum pixel")
        .rotated(rotation)
}

/// The renderer's unit for a unit of the expected files.
pub fn unit(unit: expected::Unit) -> Unit {
    match unit {
        expected::Unit::Celsius => Unit::Celsius,
        expected::Unit::Fahrenheit => Unit::Fahrenheit,
    }
}

/// What a mode draws above its spectrum, or in mode 23 in its place.
enum Top {
    Strip(Marquee),
    Nothing,
    Emoji(EmojiLoop),
    Dancer(Dancer),
}

impl Top {
    /// The top of `mode` from its start: its picture at home, the loop's first picture, or the
    /// Dancer reset.
    fn of(mode: Mode, values: &Values, unit: Unit) -> Top {
        match mode.number() {
            13 => Top::Nothing,
            14..=17 => Top::Emoji(EmojiLoop::of(mode).expect("an emoji")),
            23 => Top::Dancer(Dancer::new()),
            _ => Top::Strip(Marquee::new(strip(mode, values, unit))),
        }
    }
}

/// The value's picture: the whole grid in modes 1–12, the value area in 18–22.
fn strip(mode: Mode, values: &Values, unit: Unit) -> odc_render::Picture {
    match mode_info(mode).kind {
        Kind::Music => value_picture(mode, values, unit).expect("a value"),
        _ => picture(mode, values, unit).expect("a drawn mode"),
    }
}

/// Every signal of `spectrum/signals.txt` by its name, as each tick's window.
fn windows() -> &'static HashMap<String, Vec<[f32; WINDOW]>> {
    static WINDOWS: OnceLock<HashMap<String, Vec<[f32; WINDOW]>>> = OnceLock::new();
    WINDOWS.get_or_init(|| {
        signals()
            .into_iter()
            .map(|s| (s.name.clone(), s.windows()))
            .collect()
    })
}

/// Replays `scenario` from `base`; `false` when it switches into a mode this build doesn't draw
/// yet. A Music mode plays the scenario's sound from the tick that names it, a window a tick, and
/// keeps its spectrum across the Music modes with one (`music.md` point 8), as the engine does
/// (Phase 9 plan, Task 6).
fn replay(scenario: &Scenario, base: Rotation) -> bool {
    let start = scenario
        .ticks
        .first()
        .map(|t| t.changes.as_slice())
        .unwrap_or_default();
    let mut v = values(&readings(start));
    let mut mode = Mode::new(scenario.mode).expect("a mode from 1 to 23");
    if !draws(mode) {
        return false;
    }
    let mut rotation = base;
    let mut shown_unit = unit(scenario.unit);
    let mut top: Option<Top> = None;
    let mut music = MusicSettings::default();
    let mut spectrum: Option<Spectrum> = None;
    let mut playing: Option<(&[[f32; WINDOW]], u32)> = None;
    for tick in &scenario.ticks {
        let (mut reset, mut changed, mut missed) = (false, false, false);
        for change in &tick.changes {
            match change {
                Change::Reading { metric, value } => {
                    let metric = Metric::from_spec_name(metric).expect("a metric");
                    v.set(metric, *value);
                    changed = true;
                }
                Change::Reset(Reset::Mode(n)) => {
                    mode = Mode::new(*n).expect("a mode from 1 to 23");
                    if !draws(mode) {
                        return false;
                    }
                    reset = true;
                }
                Change::Reset(Reset::Rotation(turns)) => {
                    for _ in 0..*turns {
                        rotation = rotation.next();
                    }
                    reset = true;
                }
                Change::Reset(Reset::Unit(new)) => {
                    shown_unit = unit(*new);
                    reset = true;
                }
                Change::Reset(_) => reset = true,
                // Saved for the next mode that shows a temperature.
                Change::UnitSaved(new) => shown_unit = unit(*new),
                Change::Missed => missed = true,
                Change::Audio(name) => {
                    let signal = windows()
                        .get(name)
                        .unwrap_or_else(|| panic!("{}: no signal {name:?}", scenario.heading));
                    playing = Some((signal, tick.n));
                }
                // A Music setting is never a reset (music.md point 7).
                Change::MusicPeaks(on) => music.peaks = *on,
                Change::MusicBarFall(stop) => music.bar_fall = fall(stop),
                Change::MusicPeakFall(stop) => music.peak_fall = fall(stop),
                // Already set, refused, the cooler away and back, a wake-up: the
                // picture and its movement don't change.
                _ => {}
            }
        }
        match (&mut top, reset) {
            (Some(Top::Strip(m)), false) => {
                if changed {
                    m.update(strip(mode, &v, shown_unit));
                }
            }
            (Some(_), false) => {}
            _ => top = Some(Top::of(mode, &v, shown_unit)),
        }
        // Entering a mode with a spectrum from any other starts its levels at 0.
        if spectrum_height(mode).is_none() {
            spectrum = None;
        } else if spectrum.is_none() {
            spectrum = Some(Spectrum::enter());
        }
        if missed {
            // Nothing moves (music.md point 8).
            assert!(tick.frame.is_none());
            continue;
        }
        let window = playing
            .and_then(|(signal, from)| signal.get(usize::try_from(tick.n - from).ok()?))
            .copied()
            .unwrap_or([0.0; WINDOW]);
        let levels = (mode_info(mode).kind == Kind::Music).then(|| analyse(&window));
        if let (Some(s), Some(l)) = (spectrum.as_mut(), levels.as_ref()) {
            s.update(l, &music);
        }
        let drawn = match top.as_mut().expect("a top") {
            Top::Strip(m) => m.tick(),
            Top::Nothing => Frame::blank(),
            Top::Emoji(e) => e.tick(),
            Top::Dancer(d) => d
                .tick(levels.as_ref().expect("mode 23 takes sound"))
                .frame(),
        };
        let frame = match (spectrum_height(mode), &spectrum) {
            (Some(h), Some(s)) => with_spectrum(&drawn, &s.draw(h, music.peaks), h),
            _ => drawn,
        };
        let expected = tick.frame.as_ref().expect("a frame");
        if frame.rotated(rotation) != turned(expected, rotation) {
            diff::assert_matches(
                expected,
                &frame,
                &format!(
                    "{}, from rotation {}, tick {}",
                    scenario.heading,
                    base.setting(),
                    tick.n
                ),
            );
            panic!("{}: the turned frames differ", scenario.heading);
        }
    }
    true
}

/// A falloff stop of the scenario files, by its name.
fn fall(name: &str) -> Stop {
    Stop::from_name(name).unwrap_or_else(|| panic!("{name:?} isn't a stop"))
}

/// The readings a tick names.
fn readings(changes: &[Change]) -> Vec<(String, Option<odc_sensors::Exact>)> {
    changes
        .iter()
        .filter_map(|c| match c {
            Change::Reading { metric, value } => Some((metric.clone(), *value)),
            _ => None,
        })
        .collect()
}

/// Layer 1, the scenarios: every scenario of each drawn mode of `kind`, in all four rotations,
/// each tick's frame compared. A scenario that switches into a mode not drawn yet is skipped,
/// until every mode from 1 to 12 is.
pub fn every_scenario_of_each_drawn_mode(kind: Kind) {
    let modes = drawn_modes(kind);
    assert!(!modes.is_empty(), "no {kind:?} mode drawn yet");
    let mut skipped = 0;
    for mode in modes {
        let mine: Vec<Scenario> = mode_scenarios(mode.number());
        assert!(!mine.is_empty(), "mode {}: no scenario", mode.number());
        let mut replayed = 0;
        for scenario in &mine {
            let mut done = true;
            for base in Rotation::ALL {
                done &= replay(scenario, base);
            }
            if done {
                replayed += 1;
            } else {
                skipped += 1;
            }
        }
        println!("mode {}: {replayed} scenarios replayed", mode.number());
    }
    // Phase 8 plan, Task 1, and Phase 9's: the switches into a mode wait for that mode.
    println!("{skipped} switching into a mode not drawn yet, skipped");
    if Mode::all().all(draws) {
        assert_eq!(skipped, 0);
    }
}
