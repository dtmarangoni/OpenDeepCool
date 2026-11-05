//! Layer 2, local (Phase 8 plan, Tasks 7–8): every captured DeepCreative step of a drawn Power
//! mode where OpenDeepCool's rules and DeepCreative's draw the same thing matches with 0 differing
//! pixels: three-digit power scrolls in both, so the step's frames are consecutive windows of
//! OpenDeepCool's cycle, compared in order with `cycle_match` over the window's second half, since
//! DeepCreative first finishes the cycle it was on (`display.md`, Scrolling layout). Where the two
//! differ on purpose, the step is skipped and listed with the reason (`display.md`, Values). The
//! sweeps and captures are git-ignored, so this runs only on the owner's PC, with `--ignored`; a
//! missing file fails it, never skips it.
//!
//! `cargo test -p odc-render --test differential_power -- --ignored --nocapture`

mod common;

use std::collections::BTreeMap;

use common::{drawn_modes, values};
use odc_protocol::Frame;
use odc_render::{Kind, Marquee, Mode, Shown, Unit, mode_info, number, picture};
use odc_sensors::Exact;
use odc_testkit::diff::cycle_match;
use odc_testkit::expected::reading;
use odc_testkit::sweep::{self, Step};

/// Why both sets of rules draw the step differently, or `None` when both scroll the same strip.
fn differs(readings: &[Option<Exact>]) -> Option<&'static str> {
    let mut numbers = Vec::new();
    for reading in readings {
        let Some(r) = *reading else {
            return Some("a reading the step doesn't give");
        };
        let Shown::Number(mine) = number(Some(r)) else {
            return Some("a negative reading: OpenDeepCool draws dashes");
        };
        // DeepCreative rounds power up (display.md, Values).
        let (num, den) = (i128::from(r.num()), i128::from(r.den()));
        if i128::from(mine) != (num + den - 1).div_euclid(den) {
            return Some(
                "a different number from the rounding: DeepCreative rounds power up, OpenDeepCool \
                 to the nearest",
            );
        }
        numbers.push(mine);
    }
    if numbers.iter().all(|&n| n <= 99) {
        return Some("power of 99 or less: static in OpenDeepCool, DeepCreative scrolls it");
    }
    if numbers.iter().any(|&n| n >= 1000) {
        return Some("power of 1000 or more: DeepCreative draws only its last two digits");
    }
    None
}

#[test]
#[ignore = "needs the local DeepCreative captures in research/logs (git-ignored)"]
fn every_captured_step_where_both_rules_agree_matches_pixel_for_pixel() {
    let steps = sweep::steps().unwrap_or_else(|e| panic!("{e}; this test needs the local sweeps"));
    let sent = sweep::sent().unwrap_or_else(|e| panic!("{e}; this test needs the local captures"));
    let mut failures: Vec<String> = Vec::new();
    for mode in drawn_modes(Kind::Power) {
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
            let window: Vec<Frame> = sent.within(step.start_ms, step.end_ms).copied().collect();
            let steady = &window[window.len() / 2..];
            frames += steady.len();
            if let Err(mismatch) = cycle_match(steady, &cycle_of(mode, &named, step)) {
                failures.push(format!(
                    "{}: {}, rotation {}: {mismatch:?}",
                    step.file,
                    step.label,
                    step.rotation.setting()
                ));
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

/// OpenDeepCool's cycle for the step from home, turned.
fn cycle_of(mode: Mode, named: &[(String, Option<Exact>)], step: &Step) -> Vec<Frame> {
    let p = picture(mode, &values(named), Unit::Celsius).expect("a drawn mode");
    let length = p.cycle();
    let mut m = Marquee::new(p);
    (0..length)
        .map(|_| m.tick().rotated(step.rotation))
        .collect()
}
