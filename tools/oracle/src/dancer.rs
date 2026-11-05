//! OpenDeepCool's Dancer, mode 23 (`docs/spec/music.md`, Dancer), from the spec alone (Phase 9
//! plan, Task 1): the eleven poses, read from the spec's own pictures, and the rules that choose
//! one at every tick from the spectrum's column levels.

use std::fmt::Write as _;

use crate::drawing::{BLANK, Frame, Pixel, SIZE};
use crate::spectrum::{COLUMNS, Segment, Signal};

/// The spec's text, whose pictures are what counts (music.md).
const MUSIC_MD: &str = include_str!("../../../docs/spec/music.md");

/// The big-move routine: arms up, squat, left knee, right knee, point left, point right, arms
/// out, jump, squat, arms up.
const ROUTINE: [u8; 10] = [3, 4, 5, 6, 7, 8, 9, 10, 4, 3];

/// The eleven poses, 0 to 10, read from the three picture blocks of music.md's Dancer section.
pub fn poses() -> Vec<Frame> {
    let section = MUSIC_MD
        .split("## Dancer")
        .nth(1)
        .and_then(|s| s.split("\n## ").next())
        .expect("music.md has a Dancer section");
    let mut poses: Vec<Frame> = Vec::new();
    for block in section.split("```").skip(1).step_by(2) {
        let lines: Vec<&str> = block.lines().filter(|l| !l.trim().is_empty()).collect();
        // The first line names the poses; then 14 rows, the pictures side by side.
        let count = lines[0]
            .split_whitespace()
            .filter(|w| w.parse::<u8>().is_ok())
            .count();
        let mut frames = vec![BLANK; count];
        for (r, line) in lines[1..=SIZE].iter().enumerate() {
            let rows: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(rows.len(), count, "music.md, a Dancer row: {line:?}");
            for (frame, row) in frames.iter_mut().zip(rows) {
                assert_eq!(row.len(), SIZE, "music.md, a Dancer row: {row:?}");
                for (c, ch) in row.bytes().enumerate() {
                    if ch == b'#' {
                        frame[r][c] = Pixel::On;
                    }
                }
            }
        }
        poses.extend(frames);
    }
    assert_eq!(poses.len(), 11, "music.md has eleven Dancer poses");
    poses
}

/// The Dancer's state (music.md, Dancer: State).
#[derive(Clone, Debug)]
pub struct Dancer {
    pose: u8,
    previous_bass: f64,
    since_beat: u32,
    quiet: u32,
    step: usize,
    next_right: bool,
}

impl Dancer {
    /// A reset: pose 0, previous bass 0, 99 ticks since a beat, no quiet tick, the routine's
    /// first step, and the next sway to the right.
    pub fn reset() -> Dancer {
        Dancer {
            pose: 0,
            previous_bass: 0.0,
            since_beat: 99,
            quiet: 0,
            step: 0,
            next_right: true,
        }
    }

    fn sway(&mut self) {
        self.pose = if self.next_right { 2 } else { 1 };
        self.next_right = !self.next_right;
    }

    /// One tick from the column levels, 0–1, before any bar fall; returns the pose.
    pub fn tick(&mut self, levels: &[f64; COLUMNS]) -> u8 {
        let bass = levels[..4].iter().copied().fold(0.0, f64::max);
        let loudest = levels.iter().copied().fold(0.0, f64::max);
        self.since_beat += 1;
        if loudest < 0.1 {
            self.quiet += 1;
        } else {
            self.quiet = 0;
        }
        if self.quiet >= 3 {
            self.pose = 0;
        } else if bass >= 0.35 && bass - self.previous_bass >= 0.12 && self.since_beat >= 2 {
            self.since_beat = 0;
            if bass >= 0.6 {
                self.pose = ROUTINE[self.step];
                self.step = (self.step + 1) % ROUTINE.len();
            } else {
                self.sway();
            }
        } else if self.since_beat >= 8 && loudest >= 0.1 && (self.since_beat - 8).is_multiple_of(5)
        {
            self.sway();
        }
        self.previous_bass = bass;
        self.pose
    }
}

/// Whether a tick's levels lie within 10⁻⁶ of one of the Dancer's thresholds, so the two FFTs'
/// last bits could decide a pose. The rise is checked from the previous tick's *bass* and from a
/// reset's 0.
pub fn near_a_threshold(previous: &[f64; COLUMNS], levels: &[f64; COLUMNS]) -> bool {
    let max = |l: &[f64]| l.iter().copied().fold(0.0, f64::max);
    let (bass, loudest) = (max(&levels[..4]), max(levels));
    let rise = bass - max(&previous[..4]);
    [
        (bass, 0.35),
        (bass, 0.6),
        (loudest, 0.1),
        (rise, 0.12),
        (bass, 0.12),
    ]
    .iter()
    .any(|&(x, t)| (x - t).abs() < 1e-6)
}

/// The beat trains' tone, in column 1, whose *bass* (column 2's level, the nearer bin) is what
/// the trains set.
const BEAT_HZ: f64 = 60.0;

/// The tone's dBFS, to 0.1 dB, that gives *bass* the level asked. A level moves by 1/50 per dB
/// (music.md point 3), so it's measured once at 0 dBFS.
fn bass_db(level: f64) -> f64 {
    let probe = Signal {
        name: String::new(),
        segments: vec![Segment::Tones(1, vec![(BEAT_HZ, 0.0)])],
        moved_db: 0.0,
    };
    let at_0_dbfs = probe.tick_levels()[0][..4]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    (500.0 * (level - at_0_dbfs)).round() / 10.0
}

/// Ticks of the tone at a *bass* level.
fn tone(ticks: usize, level: f64) -> Segment {
    Segment::Tones(ticks, vec![(BEAT_HZ, bass_db(level))])
}

/// The Dancer's beat trains (Phase 9 plan, Task 1): a 60 Hz tone at *bass* levels just above and
/// below 0.35 and 0.6, rises just above and below 0.12, beats a tick apart, and a steady tone.
/// Each lasts 28 ticks or more, so the resets at ticks 7 and 19 fall inside it, and ends in
/// silence long enough to stand still.
pub fn beat_trains() -> Vec<(String, Vec<Segment>)> {
    let pulses = |level: f64, silent: usize, times: usize| {
        let mut segments = Vec::new();
        for _ in 0..times {
            segments.push(tone(1, level));
            segments.push(Segment::Silence(silent));
        }
        segments.push(Segment::Silence(28 - times * (1 + silent)));
        segments
    };
    let rises = |to: f64| {
        let mut segments = Vec::new();
        for _ in 0..8 {
            segments.push(tone(2, 0.40));
            segments.push(tone(1, to));
        }
        segments.push(Segment::Silence(4));
        segments
    };
    let mut stairs = Vec::new();
    for _ in 0..4 {
        for level in [0.38, 0.51, 0.64, 0.77, 0.90] {
            stairs.push(tone(1, level));
        }
        stairs.push(Segment::Silence(2));
    }
    stairs.push(Segment::Silence(4));
    vec![
        (
            "beat train, bass 0.37 for a tick, then 2 silent: just above a beat".into(),
            pulses(0.37, 2, 8),
        ),
        (
            "beat train, bass 0.33 for a tick, then 2 silent: just below a beat".into(),
            pulses(0.33, 2, 8),
        ),
        (
            "beat train, bass 0.62 for a tick, then 1 silent: just above a big move".into(),
            pulses(0.62, 1, 12),
        ),
        (
            "beat train, bass 0.58 for a tick, then 1 silent: just below a big move".into(),
            pulses(0.58, 1, 12),
        ),
        (
            "beat train, bass 0.40 for 2 ticks, then 0.53: rises just above 0.12".into(),
            rises(0.53),
        ),
        (
            "beat train, bass 0.40 for 2 ticks, then 0.51: rises just below 0.12".into(),
            rises(0.51),
        ),
        (
            "beat train, bass rising 0.13 a tick from 0.38 to 0.90, then 2 silent".into(),
            stairs,
        ),
        (
            "beat train, bass 0.50 steady for 22 ticks, then silent".into(),
            vec![tone(22, 0.50), Segment::Silence(6)],
        ),
    ]
}

/// The ticks at which the beat trains' second sequence resets the Dancer.
const RESETS: [usize; 2] = [7, 19];

/// `spectrum/dancer.txt`: the pose at every tick for each signal, and for the beat trains the
/// same with resets at chosen ticks.
pub fn dancer_file(signals: &[Signal]) -> String {
    let mut out = String::from(
        "# Dancer: expected poses\n\n\
         For each signal of `signals.txt`, the Dancer's pose at every tick, 0 to 10, from a reset \
         at tick 0, as music.md's Dancer rules give it from the column levels, before any bar \
         fall. Each beat train comes twice: the second time, the Dancer also resets before the \
         ticks its heading names.\n\n",
    );
    for s in signals {
        let levels: Vec<[f64; COLUMNS]> = s
            .tick_levels()
            .iter()
            .map(|l| l.map(|x| x.clamp(0.0, 1.0)))
            .collect();
        let variants: Vec<&[usize]> = if s.is_beat_train() {
            vec![&[], &RESETS]
        } else {
            vec![&[]]
        };
        for resets in variants {
            let mut heading = format!("## {}", s.name);
            if !resets.is_empty() {
                let ticks: Vec<String> = resets.iter().map(usize::to_string).collect();
                let _ = write!(heading, " · resets at ticks {}", ticks.join(" and "));
            }
            let mut dancer = Dancer::reset();
            let poses: Vec<String> = levels
                .iter()
                .enumerate()
                .map(|(n, tick)| {
                    if resets.contains(&n) {
                        dancer = Dancer::reset();
                    }
                    dancer.tick(tick).to_string()
                })
                .collect();
            let _ = writeln!(out, "{heading}\n\n{}\n", poses.join(" "));
        }
    }
    let mut text = out.trim_end().to_owned();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(frame: &Frame) -> usize {
        frame.iter().flatten().filter(|&&p| p == Pixel::On).count()
    }

    // music.md: a solid 3 × 3 head in every pose, and pose 0 stands on column 8 with rows 1–2
    // free.
    #[test]
    fn every_pose_has_its_head_and_pose_0_stands_on_column_8() {
        let poses = poses();
        for (i, pose) in poses.iter().enumerate() {
            let head = (0..SIZE - 2).any(|r| {
                (0..SIZE - 2)
                    .any(|c| (0..3).all(|dr| (0..3).all(|dc| pose[r + dr][c + dc] == Pixel::On)))
            });
            assert!(head, "pose {i} has no 3 × 3 head");
            assert!(lit(pose) > 12, "pose {i}");
        }
        assert!(poses[0][..2].iter().flatten().all(|&p| p == Pixel::Off));
        assert_eq!(poses[0][5][7], Pixel::On, "the neck on column 8");
    }

    fn levels_with(bass: f64, loudest: f64) -> [f64; COLUMNS] {
        let mut l = [0.0; COLUMNS];
        l[0] = bass;
        l[10] = loudest;
        l
    }

    // Phase 9 plan, Task 2's properties: 3 quiet ticks give pose 0; beats are at least 2 ticks
    // apart; a reset gives pose 0 and the next sway goes right.
    #[test]
    fn quiet_ticks_stand_still_beats_keep_apart_and_the_first_sway_goes_right() {
        let mut d = Dancer::reset();
        assert_eq!(
            d.tick(&levels_with(0.5, 0.5)),
            2,
            "a beat after a reset sways right"
        );
        assert_eq!(d.tick(&levels_with(0.0, 0.0)), 2);
        assert_eq!(
            d.tick(&levels_with(0.5, 0.5)),
            1,
            "two ticks on: the next sway, left"
        );
        assert_eq!(
            d.tick(&levels_with(0.9, 0.9)),
            1,
            "one tick after a beat: no beat"
        );
        for _ in 0..3 {
            d.tick(&levels_with(0.0, 0.05));
        }
        assert_eq!(d.pose, 0, "3 quiet ticks");
        let mut d = Dancer::reset();
        assert_eq!(
            d.tick(&levels_with(0.7, 0.7)),
            3,
            "a big move starts the routine"
        );
    }

    // Each beat train's tone ticks have the bass its name asks for, so they sit on the intended
    // side of each threshold.
    #[test]
    fn each_beat_train_has_the_bass_its_name_asks_for() {
        let asked: [&[f64]; 8] = [
            &[0.37],
            &[0.33],
            &[0.62],
            &[0.58],
            &[0.40, 0.53],
            &[0.40, 0.51],
            &[0.38, 0.51, 0.64, 0.77, 0.90],
            &[0.50],
        ];
        let trains: Vec<Signal> = crate::spectrum::signals()
            .into_iter()
            .filter(Signal::is_beat_train)
            .collect();
        assert_eq!(trains.len(), asked.len());
        for (train, levels) in trains.iter().zip(asked) {
            for tick in train.tick_levels() {
                let bass = tick[..4].iter().copied().fold(f64::NEG_INFINITY, f64::max);
                assert!(
                    bass == f64::NEG_INFINITY || levels.iter().any(|l| (bass - l).abs() < 0.005),
                    "{}: bass {bass}",
                    train.name
                );
            }
        }
    }
}
