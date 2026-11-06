//! Layer 2, local (Phase 9 plan, Tasks 6–8): every captured DeepCreative step of a drawn Music
//! mode, against the layout both draw (`music.md`, Spectrum: layout). DeepCreative's
//! sound and its analysis aren't OpenDeepCool's (owner, 2026-10-22), so its bars are never
//! compared with ours: each of its frames must keep the area above the spectrum off in mode 13,
//! and draw each column as a solid bar from the lit floor, at most *H* rows high, with no dot
//! (DeepCreative draws none; OpenDeepCool's with its dots off draw the same way). In an emoji mode
//! (14–17) the rows above the spectrum, 1–8, are the loop's pictures, frame for frame and in order
//! (`cycle_match`), from wherever DeepCreative's loop was. In a value mode (18–22) the rows above
//! the spectrum show the value where both sets of rules draw it alike: a still value frame for
//! frame, a scrolling one in order over the window's second half, since DeepCreative first
//! finishes the cycle it was on; every other step is listed with its reason (`music.md`, Value
//! area). The sweeps and captures are git-ignored, so this runs only on the owner's PC, with
//! `--ignored`; a missing file fails it, never skips it.
//!
//! `cargo test -p odc-render --test differential_music -- --ignored --nocapture`

mod common;

use std::collections::BTreeMap;

use common::{drawn_modes, values};
use odc_protocol::{Frame, Rotation};
use odc_render::{
    EmojiLoop, Kind, Marquee, Mode, Shown, Unit, fahrenheit, mode_info, number, spectrum_height,
    value_picture,
};
use odc_sensors::Exact;
use odc_testkit::diff::cycle_match;
use odc_testkit::expected::reading;
use odc_testkit::sweep::{self, Step};

/// `frame`'s rows above a spectrum area `h` rows high, the rows below off.
fn above(frame: &Frame, h: usize) -> Frame {
    let mut top = *frame;
    for r in 15 - h..=14 {
        for c in 1..=14 {
            top.set(r, c, false);
        }
    }
    top
}

/// Why `frame`, upright, breaks the layout of a spectrum area `h` rows high, or `None`.
fn layout(frame: &Frame, h: usize) -> Option<String> {
    for c in 1..=14 {
        let lit: Vec<usize> = (1..=h)
            .filter(|&height| frame.get(15 - height, c))
            .collect();
        if lit.first() != Some(&1) {
            return Some(format!("column {c}: the floor is off"));
        }
        if lit.iter().enumerate().any(|(i, &height)| height != i + 1) {
            return Some(format!("column {c}: not a solid bar, {lit:?}"));
        }
    }
    None
}

#[test]
#[ignore = "needs the local DeepCreative captures in research/logs (git-ignored)"]
fn every_captured_music_frame_keeps_the_layout_both_draw() {
    let steps = sweep::steps().unwrap_or_else(|e| panic!("{e}; this test needs the local sweeps"));
    let sent = sweep::sent().unwrap_or_else(|e| panic!("{e}; this test needs the local captures"));
    let mut failures: Vec<String> = Vec::new();
    for mode in drawn_modes(Kind::Music) {
        let Some(h) = spectrum_height(mode).map(usize::from) else {
            continue;
        };
        // Mode 13's rows above the spectrum: 1–2.
        let top_rows = 14 - h;
        // An emoji mode's loop, a picture a tick.
        let emoji: Option<Vec<Frame>> = EmojiLoop::of(mode).map(|mut emoji| {
            (0..emoji.ticks())
                .map(|_| above(&emoji.tick(), h))
                .collect()
        });
        let (mut checked, mut frames) = (0, 0);
        let mut quirk = Vec::new();
        let mine = steps
            .iter()
            .filter(|s| s.mode == mode.number() && s.family != "Transition");
        for step in mine {
            // DeepCreative's mode 18 in °F draws another metric, and over a loud chord not even the
            // layout (`music.md`, the quirk): no frame of it is OpenDeepCool's to match.
            if mode.number() == 18 && step.fahrenheit {
                quirk.push(step.label.clone());
                continue;
            }
            let back = Rotation::from_setting((4 - step.rotation.setting()) % 4).expect("0–3");
            let window: Vec<&Frame> = sent.within(step.start_ms, step.end_ms).collect();
            if window.is_empty() {
                failures.push(format!(
                    "{}: {}: no frame in its window",
                    step.file, step.label
                ));
                continue;
            }
            let upright: Vec<Frame> = window.iter().map(|f| f.rotated(back)).collect();
            for frame in &upright {
                frames += 1;
                if mode.number() == 13 && (1..=top_rows).any(|r| (1..=14).any(|c| frame.get(r, c)))
                {
                    failures.push(format!(
                        "{}: {}: a pixel lit in rows 1–{top_rows}",
                        step.file, step.label
                    ));
                }
                if let Some(why) = layout(frame, h) {
                    failures.push(format!("{}: {}: {why}", step.file, step.label));
                }
            }
            if let Some(cycle) = &emoji {
                let tops: Vec<Frame> = upright.iter().map(|f| above(f, h)).collect();
                if let Err(mismatch) = cycle_match(&tops, cycle) {
                    failures.push(format!(
                        "{}: {}, rotation {}: the emoji's loop: {mismatch:?}",
                        step.file,
                        step.label,
                        step.rotation.setting()
                    ));
                }
            }
            checked += 1;
        }
        println!(
            "mode {}: {checked} steps, {frames} frames checked",
            mode.number()
        );
        if !quirk.is_empty() {
            println!(
                "  skipped {} (mode 18 in °F, DeepCreative's quirk): {}",
                quirk.len(),
                quirk.join("; ")
            );
        }
        assert!(checked > 0, "mode {}: no step captured", mode.number());
    }
    failures.dedup();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// How a value step can be compared.
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

/// How both sets of rules draw `mode`'s value of `reading`, or why they differ (`music.md`, Value
/// area; `display.md`, Values and Static layout).
fn compare(
    mode: Mode,
    reading: Option<Exact>,
    in_fahrenheit: bool,
) -> Result<Compare, &'static str> {
    let Some(r) = reading else {
        return Err("a reading the step doesn't give");
    };
    let temperature = matches!(mode.number(), 18 | 19);
    if temperature && mode.number() == 18 && in_fahrenheit {
        return Err("mode 18 in °F: DeepCreative draws the Gpu temperature (music.md quirk)");
    }
    let fahrenheit_shown = temperature && in_fahrenheit;
    let rule = if fahrenheit_shown { fahrenheit } else { number };
    let Shown::Number(mine) = rule(Some(r)) else {
        return Err("a negative reading: OpenDeepCool draws dashes");
    };
    let theirs = if temperature {
        deepcreative(r, fahrenheit_shown)
    } else if r.num() % i64::try_from(r.den()).unwrap_or(i64::MAX) == 0 {
        i128::from(r.num() / i64::try_from(r.den()).unwrap_or(1))
    } else {
        return Err(
            "a fractional reading: DeepCreative rounds usage up, OpenDeepCool to the nearest",
        );
    };
    if i128::from(mine) != theirs {
        return Err(
            "a different number from the rounding: DeepCreative rounds the °C reading up (and \
             drops the °F fraction), OpenDeepCool rounds once to the nearest",
        );
    }
    if fahrenheit_shown {
        if mine <= 99 {
            return Err("°F of 99 or less: static in OpenDeepCool, DeepCreative scrolls it");
        }
        if mine >= 1000 {
            return Err("°F of 1000 or more: DeepCreative draws only its last two digits");
        }
        return Ok(Compare::Scrolling);
    }
    if mine >= 100 {
        return Err("a value of 100 or more: DeepCreative caps it at 99, OpenDeepCool scrolls");
    }
    if mine < 10 {
        return Err("a value below 10: OpenDeepCool centres it, DeepCreative uses the units place");
    }
    Ok(Compare::Static)
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

/// OpenDeepCool's rows above the spectrum for the step, upright, from home: one cycle, or the
/// still picture once.
fn tops_of(mode: Mode, named: &[(String, Option<Exact>)], step: &Step) -> Vec<Frame> {
    let unit = if step.fahrenheit {
        Unit::Fahrenheit
    } else {
        Unit::Celsius
    };
    let picture = value_picture(mode, &values(named), unit).expect("a value mode");
    let length = if picture.must_scroll() {
        picture.cycle()
    } else {
        1
    };
    let mut marquee = Marquee::new(picture);
    (0..length).map(|_| above(&marquee.tick(), 6)).collect()
}

#[test]
#[ignore = "needs the local DeepCreative captures in research/logs (git-ignored)"]
fn every_captured_value_step_where_both_rules_agree_shows_the_value_above_the_spectrum() {
    let steps = sweep::steps().unwrap_or_else(|e| panic!("{e}; this test needs the local sweeps"));
    let sent = sweep::sent().unwrap_or_else(|e| panic!("{e}; this test needs the local captures"));
    let mut failures: Vec<String> = Vec::new();
    let modes = drawn_modes(Kind::Music)
        .into_iter()
        .filter(|&mode| !mode_info(mode).metrics.is_empty());
    for mode in modes {
        let [metric] = mode_info(mode).metrics else {
            unreachable!("modes 18–22 show one value each")
        };
        let (mut still, mut scrolling, mut frames) = (0, 0, 0);
        let mut skipped: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mine = steps
            .iter()
            .filter(|s| s.mode == mode.number() && s.family != "Transition");
        for step in mine {
            let named = step_readings(step);
            let shown = named
                .iter()
                .find(|(name, _)| name == metric.spec_name())
                .and_then(|(_, v)| *v);
            let how = match compare(mode, shown, step.fahrenheit) {
                Ok(how) => how,
                Err(why) => {
                    skipped.entry(why).or_default().push(step.label.clone());
                    continue;
                }
            };
            let back = Rotation::from_setting((4 - step.rotation.setting()) % 4).expect("0–3");
            let tops: Vec<Frame> = sent
                .within(step.start_ms, step.end_ms)
                .map(|f| above(&f.rotated(back), 6))
                .collect();
            if tops.is_empty() {
                failures.push(format!(
                    "{}: {}: no frame in its window",
                    step.file, step.label
                ));
                continue;
            }
            let ours = tops_of(mode, &named, step);
            let context = format!(
                "{}: {}, rotation {}",
                step.file,
                step.label,
                step.rotation.setting()
            );
            match how {
                Compare::Static => {
                    frames += tops.len();
                    for top in &tops {
                        let differing = (1..=8)
                            .flat_map(|r| (1..=14).map(move |c| (r, c)))
                            .filter(|&(r, c)| top.get(r, c) != ours[0].get(r, c))
                            .count();
                        if differing > 0 {
                            failures.push(format!("{context}: {differing} pixels differ"));
                        }
                    }
                    still += 1;
                }
                Compare::Scrolling => {
                    let steady = &tops[tops.len() / 2..];
                    frames += steady.len();
                    if let Err(mismatch) = cycle_match(steady, &ours) {
                        failures.push(format!("{context}: {mismatch:?}"));
                    }
                    scrolling += 1;
                }
            }
        }
        println!(
            "mode {}: {} steps compared ({still} still, {scrolling} scrolling), {frames} frames",
            mode.number(),
            still + scrolling
        );
        for (why, labels) in &skipped {
            println!("  skipped {} ({why}): {}", labels.len(), labels.join("; "));
        }
        assert!(still > 0, "mode {}: no still step compared", mode.number());
        // Mode 19's °F scrolls in both; DeepCreative's mode 18 °F draws another metric.
        if mode.number() == 19 {
            assert!(scrolling > 0, "mode 19: no °F step compared");
        }
    }
    failures.dedup();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
