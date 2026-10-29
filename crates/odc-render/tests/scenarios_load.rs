//! Layer 1, the scenarios (Phase 7 plan, Task 3): every scenario of
//! `docs/spec/frames/scenarios/load.txt` that starts in a drawn Load mode, driven through
//! `picture` and the `Marquee` in all four rotations, the expected frames turned: every tick's
//! frame must match. On one tick the readings change first, then a reset applies (frames README).
//! The service's engine replays the same scenarios with its threads in Task 6.

mod common;

use common::{drawn_load_modes, turned, values};
use odc_protocol::Rotation;
use odc_render::{Marquee, Mode, Unit, draws, picture};
use odc_sensors::Metric;
use odc_testkit::diff;
use odc_testkit::expected::{Change, Reset, Scenario, load_scenarios};

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
                Change::Reset(_) => reset = true,
                Change::Missed => missed = true,
                // Only saved, already set, refused, the cooler away and back, a wake-up: the
                // picture and its movement don't change.
                _ => {}
            }
        }
        let p = picture(mode, &v, Unit::Celsius).expect("a drawn mode");
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

#[test]
fn every_scenario_of_each_drawn_load_mode_matches_in_every_rotation() {
    let scenarios = load_scenarios("load.txt");
    let modes = drawn_load_modes();
    assert!(!modes.is_empty(), "no Load mode drawn yet");
    let mut skipped = 0;
    for mode in modes {
        let mine: Vec<&Scenario> = scenarios
            .iter()
            .filter(|s| s.mode == mode.number())
            .collect();
        assert!(!mine.is_empty(), "mode {}: no scenario", mode.number());
        let mut replayed = 0;
        for scenario in mine {
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
    // A scenario is skipped only when it switches into a mode not drawn yet (S14, S21), until
    // all six Load modes are.
    println!("{skipped} switching into a mode not drawn yet, skipped");
    if drawn_load_modes().len() == 6 {
        assert_eq!(skipped, 0);
    }
}
