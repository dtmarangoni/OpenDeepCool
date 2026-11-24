//! The emoji loops of modes 14–16 and 24–26 (Phase 9 plan, Task 2; Phase 11a Task 57;
//! `docs/spec/music.md`, Emoji animations): each picture against a copy typed again here from the
//! spec, each loop's order and durations, and the restart at a reset.

use odc_protocol::Frame;
use odc_render::{COLUMNS, EmojiLoop, Mode, REST_AFTER, is_sound};

/// The modes that have an emoji: 14–16 over the spectrum, 24–26 alone.
const EMOJI_MODES: [u16; 6] = [14, 15, 16, 24, 25, 26];

/// A picture's rows from row 1; the rows below it are off.
fn frame(rows: &[&str]) -> Frame {
    let mut all = [".............."; 14];
    all[..rows.len()].copy_from_slice(rows);
    Frame::from_rows(&all)
}

/// Each mode's pictures, typed from `music.md`, and its loop: picture (from 1) and duration in ms.
fn expected(mode: u16) -> Vec<(Frame, u32)> {
    let (pictures, order): (Vec<&[&str]>, &[(usize, u32)]) = match mode {
        14 => (
            vec![
                &[
                    "..............",
                    "....##..##....",
                    "...########...",
                    "...########...",
                    "....######....",
                    ".....####.....",
                    "......##......",
                ],
                &[
                    "...###..###...",
                    "..##########..",
                    "..##########..",
                    "...########...",
                    "....######....",
                    ".....####.....",
                    "......##......",
                ],
                &[
                    "...###..###...",
                    "..##########..",
                    "#.##########.#",
                    "...########...",
                    "....######....",
                    ".....####.....",
                    "......##......",
                ],
            ],
            &[(1, 600), (2, 100), (1, 100), (3, 200)],
        ),
        15 => (
            vec![
                &[
                    ".........#....",
                    "........#.....",
                    ".....#........",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
                &[
                    ".........#....",
                    "......#.......",
                    ".....#........",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
                &[
                    ".....#........",
                    "......#.......",
                    ".....#........",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
                &[
                    ".....#........",
                    "......#.......",
                    ".........#....",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
                &[
                    ".....#........",
                    "........#.....",
                    ".........#....",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
                &[
                    ".........#....",
                    "........#.....",
                    ".........#....",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
            ],
            &[(1, 200), (2, 200), (3, 200), (4, 200), (5, 200), (6, 200)],
        ),
        16 => (
            vec![
                &[
                    "....#....#....",
                    "....######....",
                    "....######....",
                    ".....####.....",
                    "...########...",
                    "...#########.#",
                    "...########.##",
                ],
                &[
                    "....#....#....",
                    "....######....",
                    "....######....",
                    ".....####.....",
                    "...########..#",
                    "...##########.",
                    "...########...",
                ],
                &[
                    "....#....#....",
                    "....######....",
                    "....######...#",
                    ".....####...#.",
                    "...########.#.",
                    "...#########..",
                    "...########...",
                ],
            ],
            &[(1, 300), (2, 100), (3, 200), (2, 100)],
        ),
        24 => (
            vec![
                &[
                    "..............",
                    "..............",
                    "..............",
                    "....##..##....",
                    "...########...",
                    "...########...",
                    "....######....",
                    ".....####.....",
                    "......##......",
                    "..............",
                    "..............",
                    "..............",
                    "##############",
                    "..............",
                ],
                &[
                    "..............",
                    "...###..###...",
                    "..##########..",
                    "..##########..",
                    "..##########..",
                    "...########...",
                    "....######....",
                    ".....####.....",
                    "......##......",
                    "..............",
                    "......#.......",
                    ".....#.#......",
                    "#####...######",
                    "........#.....",
                ],
                &[
                    "..####..####..",
                    ".############.",
                    ".############.",
                    ".############.",
                    "#.##########.#",
                    "...########...",
                    "....######....",
                    ".....####.....",
                    "......##......",
                    "..............",
                    "........#.....",
                    ".......#.#....",
                    "#######...####",
                    "..............",
                ],
                &[
                    "..............",
                    "...###..###...",
                    "..##########..",
                    "..##########..",
                    "..##########..",
                    "...########...",
                    "....######....",
                    ".....####.....",
                    "......##......",
                    "..............",
                    "..............",
                    "..............",
                    "##############",
                    "..............",
                ],
            ],
            &[(1, 700), (2, 100), (1, 100), (3, 200), (4, 100)],
        ),
        25 => (
            vec![
                &[
                    "......#.......",
                    ".....#........",
                    ".........#....",
                    "........#.....",
                    "......#.......",
                    ".....#........",
                    ".........#....",
                    "........#.....",
                    "......#.......",
                    ".....#........",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
                &[
                    "........#.....",
                    "......#.......",
                    ".....#........",
                    ".........#....",
                    "........#.....",
                    "......#.......",
                    ".....#........",
                    ".........#....",
                    "........#.....",
                    "......#.......",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
                &[
                    ".........#....",
                    "........#.....",
                    "......#.......",
                    ".....#........",
                    ".........#....",
                    "........#.....",
                    "......#.......",
                    ".....#........",
                    ".........#....",
                    "........#.....",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
                &[
                    ".....#........",
                    ".........#....",
                    "........#.....",
                    "......#.......",
                    ".....#........",
                    ".........#....",
                    "........#.....",
                    "......#.......",
                    ".....#........",
                    ".........#....",
                    "....######.##.",
                    "....######..#.",
                    ".....####.....",
                    "...########...",
                ],
            ],
            &[(1, 300), (2, 300), (3, 300), (4, 300)],
        ),
        26 => (
            vec![
                &[
                    "..............",
                    "...#......#...",
                    "...##....##...",
                    "...########...",
                    "...##.##.##...",
                    "...########...",
                    "....######....",
                    "...########...",
                    "..##########..",
                    "..##########..",
                    "..##########..",
                    "..##########..",
                    "..###....###.#",
                    "...........###",
                ],
                &[
                    "..............",
                    "...#......#...",
                    "...##....##...",
                    "...########...",
                    "...##.##.##...",
                    "...########...",
                    "....######....",
                    "...########...",
                    "..##########..",
                    "..##########.#",
                    "..##########.#",
                    "..###########.",
                    "..###....####.",
                    "..............",
                ],
                &[
                    "..............",
                    "...#......#...",
                    "...##....##...",
                    "...########...",
                    "...##.##.##...",
                    "...########...",
                    "....######....",
                    "...########..#",
                    "..###########.",
                    "..###########.",
                    "..###########.",
                    "..##########..",
                    "..###....###..",
                    "..............",
                ],
                &[
                    "..............",
                    "...#......#...",
                    "...##....##...",
                    "...########...",
                    "...##.##.##...",
                    "...#########..",
                    "....######.#..",
                    "...########.#.",
                    "..###########.",
                    "..###########.",
                    "..###########.",
                    "..##########..",
                    "..###....###..",
                    "..............",
                ],
                &[
                    "..............",
                    "...#......#...",
                    "...##....##...",
                    "...########...",
                    "...########...",
                    "...########...",
                    "....######....",
                    "...########...",
                    "..##########..",
                    "..##########..",
                    "..##########..",
                    "..##########..",
                    "..###....###.#",
                    "...........###",
                ],
            ],
            &[
                (1, 300),
                (5, 100),
                (1, 200),
                (2, 100),
                (3, 100),
                (4, 200),
                (3, 100),
                (2, 100),
            ],
        ),
        _ => unreachable!("modes 14–16 and 24–26"),
    };
    order
        .iter()
        .map(|&(picture, ms)| (frame(pictures[picture - 1]), ms))
        .collect()
}

/// The frame of each 100 ms tick of one loop.
fn per_tick(steps: &[(Frame, u32)]) -> Vec<Frame> {
    steps
        .iter()
        .flat_map(|&(frame, ms)| std::iter::repeat_n(frame, usize::try_from(ms / 100).unwrap()))
        .collect()
}

#[test]
fn each_loop_shows_its_pictures_in_order_for_their_durations() {
    for mode in EMOJI_MODES {
        let steps = expected(mode);
        let total: u32 = steps.iter().map(|&(_, ms)| ms).sum();
        let want = match mode {
            14 => 1000,
            16 => 700,
            _ => 1200,
        };
        assert_eq!(total, want, "mode {mode}");
        let mut emoji = EmojiLoop::of(Mode::new(mode).unwrap()).expect("an emoji mode");
        assert_eq!(emoji.ticks() * 100, usize::try_from(total).unwrap());
        // Three loops in a row, from the start.
        let ticks = per_tick(&steps);
        for (n, expected) in ticks.iter().cycle().take(3 * ticks.len()).enumerate() {
            assert_eq!(emoji.tick(true), *expected, "mode {mode}, tick {n}");
        }
    }
}

#[test]
fn a_reset_shows_the_first_picture_on_the_next_frame_at_every_position() {
    for mode in EMOJI_MODES {
        let ticks = per_tick(&expected(mode));
        for position in 0..ticks.len() {
            let mut emoji = EmojiLoop::of(Mode::new(mode).unwrap()).unwrap();
            for _ in 0..position {
                emoji.tick(true);
            }
            emoji.reset();
            for (n, expected) in ticks.iter().enumerate() {
                assert_eq!(
                    emoji.tick(true),
                    *expected,
                    "mode {mode}, reset at {position}, {n}"
                );
            }
        }
    }
}

// Over the spectrum a picture has the seven rows above it and nothing below row 8; alone it has
// the whole grid, and a picture of the three alone really uses the rows below row 8.
#[test]
fn a_picture_over_the_spectrum_stays_in_rows_1_to_7_and_one_alone_uses_the_rest() {
    for mode in EMOJI_MODES {
        let mut emoji = EmojiLoop::of(Mode::new(mode).unwrap()).unwrap();
        let mut below = false;
        for _ in 0..emoji.ticks() {
            let frame = emoji.tick(true);
            below |= (8..=14).any(|r| (1..=14).any(|c| frame.get(r, c)));
        }
        assert_eq!(below, mode >= 24, "mode {mode}");
    }
}

// An emoji moves only while sound plays (owner, 2026-11-23): in silence it stands on its first
// picture for ever; with sound it runs from that picture; REST_AFTER quiet ticks after the sound
// stops it is back on its first picture, and the next sound starts the loop from the top.
#[test]
fn an_emoji_stands_still_without_sound_runs_while_it_plays_and_rests_a_second_after() {
    for mode in EMOJI_MODES {
        let ticks = per_tick(&expected(mode));
        let first = ticks[0];
        let mut emoji = EmojiLoop::of(Mode::new(mode).unwrap()).unwrap();
        for n in 0..3 * ticks.len() {
            assert_eq!(emoji.tick(false), first, "mode {mode}, silent tick {n}");
        }
        // Sound from here: the loop runs from its first picture.
        for (n, want) in ticks.iter().cycle().take(ticks.len() + 3).enumerate() {
            assert_eq!(emoji.tick(true), *want, "mode {mode}, sound tick {n}");
        }
        // The sound stops, the loop goes on for REST_AFTER - 1 ticks, then rests on picture 1.
        let mut at = (ticks.len() + 3) % ticks.len();
        for n in 1..REST_AFTER {
            assert_eq!(emoji.tick(false), ticks[at], "mode {mode}, quiet tick {n}");
            at = (at + 1) % ticks.len();
        }
        for n in 0..5 {
            assert_eq!(emoji.tick(false), first, "mode {mode}, rest tick {n}");
        }
        // The next sound starts from the top.
        assert_eq!(emoji.tick(true), first);
        assert_eq!(emoji.tick(true), ticks[1 % ticks.len()]);
    }
}

// What counts as sound: any of the 14 columns above level 0; digital silence is all 0.
#[test]
fn sound_is_any_column_above_level_0() {
    assert!(!is_sound(&[0.0; COLUMNS]));
    let mut levels = [0.0; COLUMNS];
    for column in 0..COLUMNS {
        levels[column] = 0.001;
        assert!(is_sound(&levels), "column {column}");
        levels[column] = 0.0;
    }
}

#[test]
fn only_modes_14_to_16_and_24_to_26_have_an_emoji() {
    for mode in Mode::all() {
        assert_eq!(
            EmojiLoop::of(mode).is_some(),
            EMOJI_MODES.contains(&mode.number()),
            "mode {}",
            mode.number()
        );
    }
}
