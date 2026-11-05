//! The Dancer, mode 23 (Phase 9 plan, Task 2; `docs/spec/music.md`, Dancer): each pose against
//! `mode-023.txt` in all four rotations, every sequence of `spectrum/dancer.txt` from its samples,
//! and the rules' properties.

use odc_render::{COLUMNS, Dancer, Pose, analyse};
use odc_testkit::diff;
use odc_testkit::expected::{Kind, load_mode};
use odc_testkit::spectrum::{dances, signals};
use proptest::prelude::*;

#[test]
fn each_pose_draws_its_frame_of_mode_023_in_every_rotation() {
    let file = load_mode(23);
    assert_eq!(file.cases.len(), 44);
    for case in &file.cases {
        let number = case.pose.expect("mode 23's cases name a pose");
        let pose = Pose::new(number).expect("a pose from 0 to 10");
        let Kind::Static(expected) = &case.kind else {
            panic!("{}: a pose is one picture", case.heading)
        };
        let frame = pose.frame().rotated(case.rotation);
        diff::assert_matches(expected, &frame, &format!("mode-023.txt, {}", case.heading));
    }
    assert_eq!(Pose::all().count(), 11);
    assert_eq!(Pose::new(11), None);
}

#[test]
fn every_sequence_of_dancer_txt_gives_its_poses_from_the_samples() {
    let signals = signals();
    let dances = dances();
    assert_eq!(dances.len(), 97);
    for dance in &dances {
        let signal = signals
            .iter()
            .find(|s| s.name == dance.signal)
            .expect("a signal of signals.txt");
        let mut dancer = Dancer::new();
        let poses: Vec<u8> = signal
            .windows()
            .iter()
            .enumerate()
            .map(|(n, window)| {
                if dance.resets.contains(&n) {
                    dancer.reset();
                }
                dancer.tick(&analyse(window)).number()
            })
            .collect();
        assert_eq!(poses, dance.poses, "{}", dance.heading);
    }
}

/// One tick's levels: the bass in columns 1–4, and something else elsewhere.
#[derive(Clone, Debug)]
struct Levels([f64; COLUMNS]);

fn levels() -> impl Strategy<Value = Levels> {
    let level = || prop_oneof![Just(0.0), 0.0..0.1_f64, 0.0..=1.0_f64, 0.3..0.7_f64];
    (
        prop::array::uniform4(level()),
        prop::array::uniform10(prop_oneof![Just(0.0), 0.0..=1.0_f64]),
    )
        .prop_map(|(bass, rest)| {
            let mut all = [0.0; COLUMNS];
            all[..4].copy_from_slice(&bass);
            all[4..].copy_from_slice(&rest);
            Levels(all)
        })
}

/// A tick, or a reset before it.
#[derive(Clone, Debug)]
enum Step {
    Tick(Levels),
    Reset,
}

fn steps() -> impl Strategy<Value = Vec<Step>> {
    prop::collection::vec(
        prop_oneof![
            12 => levels().prop_map(Step::Tick),
            // Quiet stretches, so silence comes often.
            4 => Just(Step::Tick(Levels([0.0; COLUMNS]))),
            1 => Just(Step::Reset),
        ],
        1..120,
    )
}

fn max(levels: &[f64]) -> f64 {
    levels.iter().copied().fold(0.0, f64::max)
}

proptest! {
    #[test]
    fn the_rules_hold_on_any_sound(steps in steps()) {
        let mut dancer = Dancer::new();
        let mut quiet = 0;
        let mut previous_bass = 0.0;
        // The ticks since the last beat, as far as the test has seen.
        let mut since_beat: Option<u32> = None;
        // Whether a sway has come since the last reset.
        let mut swayed = false;
        for step in &steps {
            let Step::Tick(Levels(levels)) = step else {
                // A reset: pose 0, and the next sway goes right.
                dancer.reset();
                prop_assert_eq!(dancer.pose(), Pose::STILL);
                prop_assert!(dancer.ticks_since_beat() >= 2);
                (quiet, previous_bass, since_beat, swayed) = (0, 0.0, None, false);
                continue;
            };
            let before = dancer.pose();
            let pose = dancer.tick(levels);
            let (bass, loudest) = (max(&levels[..4]), max(levels));
            quiet = if loudest < 0.1 { quiet + 1 } else { 0 };
            // Three quiet ticks or more: standing still.
            if quiet >= 3 {
                prop_assert_eq!(pose, Pose::STILL);
            }
            if dancer.ticks_since_beat() == 0 {
                // A beat needs a loud enough bass that rose enough, and comes at least 2 ticks
                // after the last one.
                prop_assert!(quiet < 3 && bass >= 0.35 && bass - previous_bass >= 0.12);
                prop_assert!(since_beat.is_none_or(|s| s + 1 >= 2), "beats {since_beat:?} apart");
                since_beat = Some(0);
            } else {
                since_beat = since_beat.map(|s| s + 1);
            }
            // The first sway after a reset goes right; then they alternate.
            if [1, 2].contains(&pose.number()) && pose != before {
                if !swayed {
                    prop_assert_eq!(pose.number(), 2, "the first sway goes right");
                }
                swayed = true;
            }
            previous_bass = bass;
        }
    }
}

#[test]
fn a_reset_stands_still_and_sways_right_first() {
    let mut dancer = Dancer::new();
    let mut bass = [0.0; COLUMNS];
    // Two soft beats: right, then left.
    for (level, pose) in [(0.5, 2), (0.0, 2), (0.5, 1)] {
        bass[0] = level;
        assert_eq!(dancer.tick(&bass).number(), pose);
    }
    dancer.reset();
    assert_eq!(dancer.pose(), Pose::STILL);
    bass[0] = 0.5;
    assert_eq!(
        dancer.tick(&bass).number(),
        2,
        "right again after the reset"
    );
}
