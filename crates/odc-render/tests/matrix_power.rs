//! Layer 1, the matrix (Phase 8 plan, Tasks 7–8): every case of each drawn Power mode's
//! `docs/spec/frames/mode-0NN.txt` (4 rotations × 3 value sets), and its dash cases of
//! `unavailable.txt`, drawn through `picture` and the `Marquee`, frame by frame from the reset; a
//! scrolling case over its whole cycle. Power has no unit to choose: both units draw it the same.

mod common;

use common::{drawn_modes, values};
use odc_render::{Band, Kind as ModeKind, Marquee, Mode, Unit, Values, WATT, layout, mode_info};
use odc_render::{number, picture};
use odc_sensors::Exact;
use odc_testkit::diff;
use odc_testkit::expected::{Case, Kind, load_mode, load_unavailable};
use proptest::prelude::*;

fn power_modes() -> Vec<Mode> {
    drawn_modes(ModeKind::Power)
}

/// Draws `case` in `mode`, in each unit, and compares every frame with the expected one, turned
/// by the case's rotation as the expected files are.
fn check(mode: Mode, case: &Case, file: &str) {
    for unit in [Unit::Celsius, Unit::Fahrenheit] {
        let context = |what: String| format!("{file}, {}, {unit:?}, {what}", case.heading);
        let p = picture(mode, &values(&case.readings), unit).expect("a drawn mode");
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
            Kind::Loop(_) => panic!("{}: Power has no loop", context(String::new())),
        }
    }
}

#[test]
fn every_case_of_each_drawn_power_mode_matches_its_expected_frames() {
    let modes = power_modes();
    assert!(!modes.is_empty(), "no Power mode drawn yet");
    for mode in modes {
        let file = format!("mode-{:03}.txt", mode.number());
        let frames = load_mode(mode.number());
        assert_eq!(frames.cases.len(), 12, "{file}: 4 rotations × 3 value sets");
        for case in &frames.cases {
            assert_eq!(case.unit, None, "{file}: {}", case.heading);
            check(mode, case, &file);
        }
    }
}

#[test]
fn the_dash_cases_of_each_drawn_power_mode_match() {
    let dashes = load_unavailable();
    for mode in power_modes() {
        let cases: Vec<&Case> = dashes
            .iter()
            .filter(|(m, _)| *m == mode.number())
            .map(|(_, case)| case)
            .collect();
        for case in &cases {
            check(mode, case, "unavailable.txt");
        }
        // unavailable.txt: one case of each Power mode, in rotations 0–3.
        assert_eq!(cases.len(), 4, "mode {}", mode.number());
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
        (-1000_i64..=12_000_000, 1_u64..=1_000_000).prop_map(|(num, den)| Exact::new(num, den)),
        (0_i64..=1500).prop_map(|n| Some(Exact::whole(n))),
    ]
}

proptest! {
    // Only the band's rows ever light, at every position of the cycle.
    #[test]
    fn only_the_band_rows_are_ever_lit(top in reading(), bottom in reading()) {
        for mode in power_modes() {
            let metrics = mode_info(mode).metrics;
            let mut v = Values::new();
            for (&metric, value) in metrics.iter().zip([top, bottom]) {
                v.set(metric, value);
            }
            let p = picture(mode, &v, Unit::Celsius).expect("a drawn mode");
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

    // The picture is the strip of the power rule's number for the mode's own metric, followed by
    // `W`, in either unit; another metric's reading never changes it (AGENTS.md rule 11).
    #[test]
    fn each_power_draws_its_own_metric_with_the_power_rule(w in reading(), other in reading()) {
        for mode in power_modes() {
            let metrics = mode_info(mode).metrics;
            let mut v = Values::new();
            for &metric in metrics {
                v.set(metric, w);
            }
            let mut with_other = v.clone();
            for metric in odc_sensors::Metric::ALL.into_iter().filter(|m| !metrics.contains(m)) {
                with_other.set(metric, other);
            }
            let bands: Vec<Band> = metrics.iter().map(|_| Band { shown: number(w), symbol: &WATT }).collect();
            for unit in [Unit::Celsius, Unit::Fahrenheit] {
                let p = picture(mode, &v, unit).expect("a drawn mode");
                prop_assert_eq!(&p, &picture(mode, &with_other, unit).expect("a drawn mode"));
                prop_assert_eq!(p, layout(&bands));
            }
        }
    }
}
