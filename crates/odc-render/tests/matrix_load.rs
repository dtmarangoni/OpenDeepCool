//! Layer 1, the matrix (Phase 7 plan, Task 3): every case of each drawn Load mode's
//! `docs/spec/frames/mode-00N.txt` (4 rotations × 3 value sets), and its dash cases of
//! `unavailable.txt`, drawn through `picture` and the `Marquee`, frame by frame from the reset; a
//! scrolling case over its whole cycle.

mod common;

use common::{drawn_load_modes, values};
use odc_render::{Marquee, Mode, Unit, Values, mode_info, picture};
use odc_sensors::Exact;
use odc_testkit::diff;
use odc_testkit::expected::{Case, Kind, load_mode, load_unavailable};
use proptest::prelude::*;

/// Draws `case` in `mode` and compares every frame with the expected one, turned by the case's
/// rotation as the expected files are.
fn check(mode: Mode, case: &Case, file: &str) {
    let context = |what: String| format!("{file}, {}, {what}", case.heading);
    let p = picture(mode, &values(&case.readings), Unit::Celsius).expect("a drawn mode");
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
        Kind::Loop(_) => panic!("{}: Load has no loop", context(String::new())),
    }
}

#[test]
fn every_case_of_each_drawn_load_mode_matches_its_expected_frames() {
    let modes = drawn_load_modes();
    assert!(!modes.is_empty(), "no Load mode drawn yet");
    for mode in modes {
        let file = format!("mode-{:03}.txt", mode.number());
        let frames = load_mode(mode.number());
        assert_eq!(frames.cases.len(), 12, "{file}: 4 rotations × 3 value sets");
        for case in &frames.cases {
            check(mode, case, &file);
        }
    }
}

#[test]
fn the_dash_cases_of_each_drawn_load_mode_match() {
    let dashes = load_unavailable();
    for mode in drawn_load_modes() {
        let cases: Vec<&Case> = dashes
            .iter()
            .filter(|(m, _)| *m == mode.number())
            .map(|(_, case)| case)
            .collect();
        for case in &cases {
            check(mode, case, "unavailable.txt");
        }
        // unavailable.txt has dash cases of modes 1, 4 and 6, each in rotations 0–3.
        let want = match mode.number() {
            1 | 4 => 8,
            6 => 4,
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
        (-1000_i64..=12_000, 1_u64..=1000).prop_map(|(num, den)| Exact::new(num, den)),
        (0_i64..=99).prop_map(|n| Some(Exact::whole(n))),
    ]
}

proptest! {
    // For every value, only its band's rows change (Phase 7 plan, Task 3), at every position of
    // the cycle.
    #[test]
    fn only_the_band_rows_are_ever_lit(top in reading(), bottom in reading()) {
        for mode in drawn_load_modes() {
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
}
