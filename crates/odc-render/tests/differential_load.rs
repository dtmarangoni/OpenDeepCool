//! Layer 2, local (Phase 7 plan, Task 3): every captured DeepCreative step of a drawn Load mode
//! where OpenDeepCool's rules agree with DeepCreative's draws, in every frame of its steady
//! window, exactly the picture at home: 0 differing pixels. Where the two differ on purpose, the
//! step is skipped and listed with the reason (`display.md`, Values and Static layout). The
//! sweeps and captures are git-ignored, so this runs only on the owner's PC, with `--ignored`; a
//! missing file fails it, never skips it.
//!
//! `cargo test -p odc-render --test differential_load -- --ignored --nocapture`

mod common;

use std::collections::BTreeMap;

use common::{drawn_modes, values};
use odc_render::{Marquee, Mode, Unit, mode_info, picture};
use odc_sensors::Exact;
use odc_testkit::expected::reading;
use odc_testkit::sweep::{self, Step};

/// Why a step can't be compared, or `None` when both sets of rules draw it the same way: whole
/// readings from 0 to 99, and in one-value modes two digits.
fn differs(readings: &[Option<Exact>]) -> Option<&'static str> {
    let mut whole = Vec::new();
    for reading in readings {
        let Some(r) = reading else {
            return Some("a reading the step doesn't give");
        };
        if r.num() % i64::try_from(r.den()).unwrap_or(i64::MAX) != 0 {
            return Some(
                "a fractional reading: DeepCreative rounds usage up, OpenDeepCool to the nearest",
            );
        }
        whole.push(r.num() / i64::try_from(r.den()).unwrap_or(1));
    }
    if whole.iter().any(|&n| n >= 100) {
        return Some("a value of 100 or more: DeepCreative caps it at 99, OpenDeepCool scrolls");
    }
    if whole.len() == 1 && whole[0] < 10 {
        return Some(
            "one value below 10: OpenDeepCool centres it, DeepCreative uses the units place",
        );
    }
    None
}

#[test]
#[ignore = "needs the local DeepCreative captures in research/logs (git-ignored)"]
fn every_captured_step_where_both_rules_agree_matches_pixel_for_pixel() {
    let steps = sweep::steps().unwrap_or_else(|e| panic!("{e}; this test needs the local sweeps"));
    let sent = sweep::sent().unwrap_or_else(|e| panic!("{e}; this test needs the local captures"));
    let mut failures: Vec<String> = Vec::new();
    for mode in drawn_modes(odc_render::Kind::Load) {
        let metrics = mode_info(mode).metrics;
        let (mut compared, mut frames) = (0, 0);
        let mut skipped: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mine = steps
            .iter()
            .filter(|s| s.mode == mode.number() && s.family != "Transition");
        for step in mine {
            let named = step_readings(step);
            let shown: Vec<Option<Exact>> = metrics
                .iter()
                .map(|m| {
                    named
                        .iter()
                        .find(|(name, _)| name == m.spec_name())
                        .and_then(|(_, v)| *v)
                })
                .collect();
            if let Some(why) = differs(&shown) {
                skipped.entry(why).or_default().push(step.label.clone());
                continue;
            }
            let expected = home(mode, &named).rotated(step.rotation);
            let window: Vec<_> = sent.within(step.start_ms, step.end_ms).collect();
            if window.is_empty() {
                failures.push(format!(
                    "{}: {}: no frame in its window",
                    step.file, step.label
                ));
                continue;
            }
            for frame in window {
                frames += 1;
                let differing = (1..=14)
                    .flat_map(|r| (1..=14).map(move |c| (r, c)))
                    .filter(|&(r, c)| frame.get(r, c) != expected.get(r, c))
                    .count();
                if differing > 0 {
                    failures.push(format!(
                        "{}: {}, rotation {}: {differing} pixels differ",
                        step.file,
                        step.label,
                        step.rotation.setting()
                    ));
                }
            }
            compared += 1;
        }
        println!(
            "mode {}: {compared} steps, {frames} frames compared",
            mode.number()
        );
        for (why, labels) in &skipped {
            println!("  skipped {} ({why}): {}", labels.len(), labels.join("; "));
        }
        assert!(compared > 0, "mode {}: no step compared", mode.number());
    }
    failures.dedup();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The step's readings, exact, by their names.
fn step_readings(step: &Step) -> Vec<(String, Option<Exact>)> {
    step.readings
        .iter()
        .map(|(name, text)| {
            let value = reading(text).unwrap_or_else(|| {
                panic!("{}: {}: {text:?} isn't a decimal", step.file, step.label)
            });
            (name.clone(), value)
        })
        .collect()
}

/// The picture at home, unrotated.
fn home(mode: Mode, named: &[(String, Option<Exact>)]) -> odc_protocol::Frame {
    let p = picture(mode, &values(named), Unit::Celsius).expect("a drawn mode");
    Marquee::new(p).tick()
}
