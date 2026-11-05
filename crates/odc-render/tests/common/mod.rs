//! What the tests of each kind of mode share: the modes drawn so far, a case's readings as
//! `Values`, and the replay of the scenarios. Each test file uses part of it.
#![allow(dead_code)]

use odc_protocol::{Frame, Rotation};
use odc_render::{Kind, Marquee, Mode, Unit, Values, draws, mode_info, picture};
use odc_sensors::{Exact, Metric};
use odc_testkit::diff;
use odc_testkit::expected::{self, Change, Picture, Reset, Scenario, every_metric, mode_scenarios};

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
        .expect("modes 1–12 have no spectrum pixel")
        .rotated(rotation)
}

/// The renderer's unit for a unit of the expected files.
pub fn unit(unit: expected::Unit) -> Unit {
    match unit {
        expected::Unit::Celsius => Unit::Celsius,
        expected::Unit::Fahrenheit => Unit::Fahrenheit,
    }
}

/// Replays `scenario` from `base`; `false` when it switches into a mode this build doesn't draw
/// yet.
fn replay(scenario: &Scenario, base: Rotation) -> bool {
    let start = scenario
        .ticks
        .first()
        .map(|t| t.changes.as_slice())
        .unwrap_or_default();
    let mut v = values(&readings(start));
    let mut mode = Mode::new(scenario.mode).expect("a mode from 1 to 23");
    let mut rotation = base;
    let mut shown_unit = unit(scenario.unit);
    let mut marquee: Option<Marquee> = None;
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
                // Already set, refused, the cooler away and back, a wake-up: the
                // picture and its movement don't change.
                _ => {}
            }
        }
        let p = picture(mode, &v, shown_unit).expect("a drawn mode");
        let m = match &mut marquee {
            Some(m) if !reset => {
                if changed {
                    m.update(p);
                }
                m
            }
            _ => marquee.insert(Marquee::new(p)),
        };
        if missed {
            assert!(tick.frame.is_none());
            continue;
        }
        let expected = tick.frame.as_ref().expect("a frame");
        let frame = m.tick();
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
    // Phase 8 plan, Task 1: the switches into modes 7–12 wait for those modes.
    println!("{skipped} switching into a mode not drawn yet, skipped");
    if (1..=12).all(|n| Mode::new(n).is_some_and(draws)) {
        assert_eq!(skipped, 0);
    }
}
