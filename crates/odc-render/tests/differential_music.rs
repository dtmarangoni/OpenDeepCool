//! Layer 2, local (Phase 9 plan, Tasks 6–7): every captured DeepCreative step of a drawn Music
//! mode without values, against the layout both draw (`music.md`, Spectrum: layout). DeepCreative's
//! sound and its analysis aren't OpenDeepCool's (owner, 2026-10-22), so its bars are never
//! compared with ours: each of its frames must keep the area above the spectrum off in mode 13,
//! and draw each column as a solid bar from the lit floor, at most *H* rows high, with no dot
//! (DeepCreative draws none; OpenDeepCool's with its dots off draw the same way). In an emoji mode
//! (14–17) the rows above the spectrum, 1–8, are the loop's pictures, frame for frame and in order
//! (`cycle_match`), from wherever DeepCreative's loop was. The sweeps and captures are git-ignored,
//! so this runs only on the owner's PC, with `--ignored`; a missing file fails it, never skips it.
//!
//! `cargo test -p odc-render --test differential_music -- --ignored --nocapture`

mod common;

use common::drawn_modes;
use odc_protocol::{Frame, Rotation};
use odc_render::{EmojiLoop, Kind, spectrum_height};
use odc_testkit::diff::cycle_match;
use odc_testkit::sweep;

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
        let mine = steps
            .iter()
            .filter(|s| s.mode == mode.number() && s.family != "Transition");
        for step in mine {
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
        assert!(checked > 0, "mode {}: no step captured", mode.number());
    }
    failures.dedup();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
