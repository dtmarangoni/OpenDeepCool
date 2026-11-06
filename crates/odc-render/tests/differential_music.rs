//! Layer 2, local (Phase 9 plan, Task 6): every captured DeepCreative step of a drawn Music mode
//! without values, against the layout both draw (`music.md`, Spectrum: layout). DeepCreative's
//! sound and its analysis aren't OpenDeepCool's (owner, 2026-10-22), so its bars are never
//! compared with ours: each of its frames must keep the area above the spectrum off in mode 13,
//! and draw each column as a solid bar from the lit floor, at most *H* rows high, with no dot
//! (DeepCreative draws none; OpenDeepCool's with its dots off draw the same way). The sweeps and
//! captures are git-ignored, so this runs only on the owner's PC, with `--ignored`; a missing
//! file fails it, never skips it.
//!
//! `cargo test -p odc-render --test differential_music -- --ignored --nocapture`

mod common;

use common::drawn_modes;
use odc_protocol::{Frame, Rotation};
use odc_render::{Kind, spectrum_height};
use odc_testkit::sweep;

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
        let above = 14 - h;
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
            for frame in window {
                let upright = frame.rotated(back);
                frames += 1;
                if mode.number() == 13 && (1..=above).any(|r| (1..=14).any(|c| upright.get(r, c))) {
                    failures.push(format!(
                        "{}: {}: a pixel lit in rows 1–{above}",
                        step.file, step.label
                    ));
                }
                if let Some(why) = layout(&upright, h) {
                    failures.push(format!("{}: {}: {why}", step.file, step.label));
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
