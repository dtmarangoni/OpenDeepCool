//! Layer 1, the matrix (Phase 8 plan, Tasks 4–6): every case of each drawn Temperature mode's
//! `docs/spec/frames/mode-00N.txt` (4 rotations × °C and °F × 3 value sets), and its dash cases of
//! `unavailable.txt`, drawn through `picture` and the `Marquee`, frame by frame from the reset; a
//! scrolling case over its whole cycle.

mod common;

use common::{drawn_modes, unit, values};
use odc_render::{Band, CELSIUS, FAHRENHEIT, layout, number, picture};
use odc_render::{Kind as ModeKind, Marquee, Mode, Shown, Unit, Values, fahrenheit, mode_info};
use odc_sensors::Exact;
use odc_testkit::diff;
use odc_testkit::expected::{Case, Kind, load_mode, load_unavailable};
use proptest::prelude::*;

fn temperature_modes() -> Vec<Mode> {
    drawn_modes(ModeKind::Temperature)
}

/// Draws `case` in `mode`, in the case's unit, and compares every frame with the expected one,
/// turned by the case's rotation as the expected files are.
fn check(mode: Mode, case: &Case, file: &str) {
    let context = |what: String| format!("{file}, {}, {what}", case.heading);
    let shown_unit = unit(case.unit.expect("a temperature case names its unit"));
    let p = picture(mode, &values(&case.readings), shown_unit).expect("a drawn mode");
    let mut m = Marquee::new(p);
    match &case.kind {
        Kind::Static(expected) => {
            for tick in 0..3 {
                let frame = m.tick().rotated(case.rotation);
                diff::assert_matches(expected, &frame, &context(format!("tick {tick}")));
            }
        }
        Kind::Scrolling(cycle) => {
            // The whole cycle from the reset, then its first frame again.
            for (k, expected) in cycle.iter().chain(cycle.first()).enumerate() {
                let frame = m.tick().rotated(case.rotation);
                diff::assert_matches(expected, &frame, &context(format!("frame {k}")));
            }
        }
        Kind::Loop(_) => panic!("{}: Temperature has no loop", context(String::new())),
    }
}

#[test]
fn every_case_of_each_drawn_temperature_mode_matches_its_expected_frames() {
    let modes = temperature_modes();
    assert!(!modes.is_empty(), "no Temperature mode drawn yet");
    for mode in modes {
        let file = format!("mode-{:03}.txt", mode.number());
        let frames = load_mode(mode.number());
        assert_eq!(
            frames.cases.len(),
            24,
            "{file}: 4 rotations × °C and °F × 3 value sets"
        );
        for case in &frames.cases {
            check(mode, case, &file);
        }
    }
}

#[test]
fn the_dash_cases_of_each_drawn_temperature_mode_match() {
    let dashes = load_unavailable();
    for mode in temperature_modes() {
        let cases: Vec<&Case> = dashes
            .iter()
            .filter(|(m, _)| *m == mode.number())
            .map(|(_, case)| case)
            .collect();
        for case in &cases {
            check(mode, case, "unavailable.txt");
        }
        // unavailable.txt: mode 7 in °C and in °F, mode 9 in °F, each in rotations 0–3.
        let want = match mode.number() {
            7 => 8,
            9 => 4,
            _ => 0,
        };
        assert_eq!(cases.len(), want, "mode {}", mode.number());
    }
}

/// The rows of each band: one value in rows 6–10; two values in rows 2–6 and 9–13.
fn band_rows(values: usize) -> Vec<usize> {
    if values == 1 {
        (6..=10).collect()
    } else {
        (2..=6).chain(9..=13).collect()
    }
}

fn reading() -> impl Strategy<Value = Option<Exact>> {
    prop_oneof![
        Just(None),
        (-1000_i64..=12_000_000, 1_u64..=1000).prop_map(|(num, den)| Exact::new(num, den)),
        (0_i64..=150).prop_map(|n| Some(Exact::whole(n))),
    ]
}

fn units() -> impl Strategy<Value = Unit> {
    prop_oneof![Just(Unit::Celsius), Just(Unit::Fahrenheit)]
}

proptest! {
    // Only the band's rows ever light, at every position of the cycle, in each unit.
    #[test]
    fn only_the_band_rows_are_ever_lit(top in reading(), bottom in reading(), u in units()) {
        for mode in temperature_modes() {
            let metrics = mode_info(mode).metrics;
            let mut v = Values::new();
            for (&metric, value) in metrics.iter().zip([top, bottom]) {
                v.set(metric, value);
            }
            let p = picture(mode, &v, u).expect("a drawn mode");
            let rows = band_rows(metrics.len());
            let mut m = Marquee::new(p);
            for _ in 0..m.picture().cycle() {
                let frame = m.tick();
                for row in (1..=14).filter(|r| !rows.contains(r)) {
                    prop_assert!((1..=14).all(|c| !frame.get(row, c)), "mode {}: row {} lit", mode.number(), row);
                }
            }
        }
    }

    // The picture in each unit is the one the value rules give for the mode's own metric: the
    // °F number is the nearest whole number to the exact reading's °F value (Phase 8 plan,
    // Task 4), and another metric's reading never changes it (AGENTS.md rule 11).
    #[test]
    fn each_unit_draws_the_number_its_rule_gives(c in reading(), other in reading()) {
        for mode in temperature_modes() {
            let metrics = mode_info(mode).metrics;
            let mut v = Values::new();
            for &metric in metrics {
                v.set(metric, c);
            }
            let mut with_other = v.clone();
            for metric in odc_sensors::Metric::ALL.into_iter().filter(|m| !metrics.contains(m)) {
                with_other.set(metric, other);
            }
            for (u, rule, symbol) in [
                (Unit::Celsius, number as fn(Option<Exact>) -> Shown, &CELSIUS),
                (Unit::Fahrenheit, fahrenheit, &FAHRENHEIT),
            ] {
                let p = picture(mode, &v, u).expect("a drawn mode");
                prop_assert_eq!(&p, &picture(mode, &with_other, u).expect("a drawn mode"));
                let bands: Vec<Band> = metrics.iter().map(|_| Band { shown: rule(c), symbol }).collect();
                prop_assert_eq!(p, layout(&bands));
            }
        }
    }
}
