//! Layer 2, local (Phase 8 plan, Tasks 4–6): every captured DeepCreative step of a drawn
//! Temperature mode where OpenDeepCool's rules and DeepCreative's draw the same thing matches with
//! 0 differing pixels. A static step (two-digit °C) equals the picture at home in every frame of
//! its window; a scrolling step (three-digit °F) is consecutive windows of OpenDeepCool's cycle,
//! compared in order with `cycle_match` over the window's second half, since DeepCreative first
//! finishes the cycle it was on (`display.md`, Scrolling layout). Where the two differ on purpose,
//! the step is skipped and listed with the reason (`display.md`, Values and Static layout). The
//! sweeps and captures are git-ignored, so this runs only on the owner's PC, with `--ignored`; a
//! missing file fails it, never skips it.
//!
//! `cargo test -p odc-render --test differential_temperature -- --ignored --nocapture`

mod common;

use std::collections::BTreeMap;

use common::{drawn_modes, values};
use odc_protocol::Frame;
use odc_render::{Kind, Marquee, Mode, Shown, Unit, fahrenheit, mode_info, number, picture};
use odc_sensors::Exact;
use odc_testkit::diff::cycle_match;
use odc_testkit::expected::reading;
use odc_testkit::sweep::{self, Step};

/// How a step can be compared.
enum Compare {
    Static,
    Scrolling,
}

/// DeepCreative's number for a temperature (`display.md`, Values): °C rounded up; in °F, the °C
/// reading rounded up, converted, then the fraction dropped.
fn deepcreative(reading: Exact, in_fahrenheit: bool) -> i128 {
    let (num, den) = (i128::from(reading.num()), i128::from(reading.den()));
    let up = (num + den - 1).div_euclid(den);
    if in_fahrenheit {
        (9 * up + 160).div_euclid(5)
    } else {
        up
    }
}

/// How both sets of rules draw the step, or why they differ: the readings each value shows.
fn compare(readings: &[Option<Exact>], in_fahrenheit: bool) -> Result<Compare, &'static str> {
    let mut numbers = Vec::new();
    for reading in readings {
        let Some(r) = *reading else {
            return Err("a reading the step doesn't give");
        };
        let rule = if in_fahrenheit { fahrenheit } else { number };
        let Shown::Number(mine) = rule(Some(r)) else {
            return Err("a negative reading: OpenDeepCool draws dashes");
        };
        if i128::from(mine) != deepcreative(r, in_fahrenheit) {
            return Err(
                "a different number from the rounding: DeepCreative rounds the °C reading up (and \
                 drops the °F fraction), OpenDeepCool rounds once to the nearest",
            );
        }
        numbers.push(mine);
    }
    let two = numbers.len() == 2;
    if in_fahrenheit {
        if numbers.iter().all(|&n| n <= 99) {
            return Err("°F of 99 or less: static in OpenDeepCool, DeepCreative scrolls it");
        }
        if numbers.iter().any(|&n| n >= 1000) {
            return Err("°F of 1000 or more: DeepCreative draws only its last two digits");
        }
        return Ok(Compare::Scrolling);
    }
    if two {
        return Err("OpenDeepCool ends `°C` at column 14 in two-value modes (owner, 2026-10-24)");
    }
    if numbers.iter().any(|&n| n >= 100) {
        return Err("a value of 100 or more: DeepCreative caps it at 99, OpenDeepCool scrolls");
    }
    if numbers.iter().any(|&n| n < 10) {
        return Err(
            "one value below 10: OpenDeepCool centres it, DeepCreative uses the units place",
        );
    }
    Ok(Compare::Static)
}

#[test]
#[ignore = "needs the local DeepCreative captures in research/logs (git-ignored)"]
fn every_captured_step_where_both_rules_agree_matches_pixel_for_pixel() {
    let steps = sweep::steps().unwrap_or_else(|e| panic!("{e}; this test needs the local sweeps"));
    let sent = sweep::sent().unwrap_or_else(|e| panic!("{e}; this test needs the local captures"));
    let mut failures: Vec<String> = Vec::new();
    for mode in drawn_modes(Kind::Temperature) {
        let metrics = mode_info(mode).metrics;
        let (mut celsius, mut fahrenheit_steps, mut frames) = (0, 0, 0);
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
            let how = match compare(&shown, step.fahrenheit) {
                Ok(how) => how,
                Err(why) => {
                    skipped.entry(why).or_default().push(step.label.clone());
                    continue;
                }
            };
            let window: Vec<Frame> = sent.within(step.start_ms, step.end_ms).copied().collect();
            if window.is_empty() {
                failures.push(format!(
                    "{}: {}: no frame in its window",
                    step.file, step.label
                ));
                continue;
            }
            let cycle = cycle_of(mode, &named, step);
            match how {
                Compare::Static => {
                    for frame in &window {
                        frames += 1;
                        let differing = (1..=14)
                            .flat_map(|r| (1..=14).map(move |c| (r, c)))
                            .filter(|&(r, c)| frame.get(r, c) != cycle[0].get(r, c))
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
                    celsius += 1;
                }
                Compare::Scrolling => {
                    let steady = &window[window.len() / 2..];
                    frames += steady.len();
                    if let Err(mismatch) = cycle_match(steady, &cycle) {
                        failures.push(format!(
                            "{}: {}, rotation {}: {mismatch:?}",
                            step.file,
                            step.label,
                            step.rotation.setting()
                        ));
                    }
                    fahrenheit_steps += 1;
                }
            }
        }
        println!(
            "mode {}: {} steps compared ({celsius} in °C, {fahrenheit_steps} in °F), {frames} \
             frames",
            mode.number(),
            celsius + fahrenheit_steps
        );
        for (why, labels) in &skipped {
            println!("  skipped {} ({why}): {}", labels.len(), labels.join("; "));
        }
        // Two values end at column 14 in °C, so only °F compares in mode 9 (Phase 8 plan, Task 6).
        if metrics.len() == 1 {
            assert!(celsius > 0, "mode {}: no °C step compared", mode.number());
        }
        assert!(
            fahrenheit_steps > 0,
            "mode {}: no °F step compared",
            mode.number()
        );
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

/// OpenDeepCool's frames for the step from home, turned: one cycle, or the static picture once.
fn cycle_of(mode: Mode, named: &[(String, Option<Exact>)], step: &Step) -> Vec<Frame> {
    let unit = if step.fahrenheit {
        Unit::Fahrenheit
    } else {
        Unit::Celsius
    };
    let p = picture(mode, &values(named), unit).expect("a drawn mode");
    let length = if p.must_scroll() { p.cycle() } else { 1 };
    let mut m = Marquee::new(p);
    (0..length)
        .map(|_| m.tick().rotated(step.rotation))
        .collect()
}
