//! The emoji loops of modes 14–17 (Phase 9 plan, Task 2; `docs/spec/music.md`, Emoji
//! animations): each picture against a copy typed again here from the spec, each loop's order
//! and durations, and the restart at a reset.

use odc_protocol::Frame;
use odc_render::{EmojiLoop, Mode};

/// Rows 1–7 of a picture; rows 8–14 are off.
fn frame(rows: [&str; 7]) -> Frame {
    let mut all = [".............."; 14];
    all[..7].copy_from_slice(&rows);
    Frame::from_rows(&all)
}

/// Each mode's pictures, typed from `music.md`, and its loop: picture and duration in ms.
fn expected(mode: u16) -> Vec<(Frame, u32)> {
    let (pictures, order): (Vec<[&str; 7]>, &[(usize, u32)]) = match mode {
        14 => (
            vec![
                [
                    "..............",
                    "..............",
                    "...###..###...",
                    "..............",
                    "..............",
                    "..............",
                    ".....####.....",
                ],
                [
                    "...#......#...",
                    "....#....#....",
                    ".....#..#.....",
                    "....#....#....",
                    "...#......#...",
                    "..............",
                    ".....####.....",
                ],
            ],
            &[(2, 400), (1, 100), (2, 100), (1, 100), (2, 100), (1, 100)],
        ),
        15 => (
            vec![
                [
                    "...#......#...",
                    "..#.#....#.#..",
                    ".#...#..#...#.",
                    "..............",
                    "..............",
                    "..............",
                    ".....####.....",
                ],
                [
                    "..............",
                    "..............",
                    "..###....###..",
                    "..............",
                    "..............",
                    "..............",
                    ".....####.....",
                ],
            ],
            &[(1, 400), (2, 100), (1, 100), (2, 100), (1, 100), (2, 100)],
        ),
        16 => (
            vec![
                [
                    "..............",
                    "..###....###..",
                    "..###....###..",
                    "..###....###..",
                    "..............",
                    "....#....#....",
                    ".....####.....",
                ],
                [
                    "..............",
                    "..............",
                    "..###....###..",
                    "..............",
                    "..............",
                    "....#....#....",
                    ".....####.....",
                ],
            ],
            &[(1, 400), (2, 100), (1, 100), (2, 100), (1, 100), (2, 100)],
        ),
        17 => (
            vec![
                [
                    "..............",
                    ".#####..#####.",
                    "...##.....##..",
                    "..............",
                    "..............",
                    ".....####.....",
                    "..............",
                ],
                [
                    "..............",
                    ".#####..#####.",
                    "..##.....##...",
                    "..............",
                    "..............",
                    ".....####.....",
                    "..............",
                ],
                [
                    "..............",
                    ".#####..#####.",
                    ".##.....##....",
                    "..............",
                    "..............",
                    ".....####.....",
                    "..............",
                ],
                [
                    "..............",
                    ".#####..#####.",
                    "....##.....##.",
                    "..............",
                    "..............",
                    ".....####.....",
                    "..............",
                ],
            ],
            &[(1, 100), (2, 100), (3, 100), (2, 100), (1, 200), (4, 500)],
        ),
        _ => unreachable!("modes 14–17"),
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
    for mode in 14..=17 {
        let steps = expected(mode);
        let total: u32 = steps.iter().map(|&(_, ms)| ms).sum();
        assert_eq!(total, if mode == 17 { 1100 } else { 900 }, "mode {mode}");
        let mut emoji = EmojiLoop::of(Mode::new(mode).unwrap()).expect("an emoji mode");
        assert_eq!(emoji.ticks() * 100, usize::try_from(total).unwrap());
        // Three loops in a row, from the start.
        let ticks = per_tick(&steps);
        for (n, expected) in ticks.iter().cycle().take(3 * ticks.len()).enumerate() {
            assert_eq!(emoji.tick(), *expected, "mode {mode}, tick {n}");
        }
    }
}

#[test]
fn a_reset_shows_the_first_picture_on_the_next_frame_at_every_position() {
    for mode in 14..=17 {
        let ticks = per_tick(&expected(mode));
        for position in 0..ticks.len() {
            let mut emoji = EmojiLoop::of(Mode::new(mode).unwrap()).unwrap();
            for _ in 0..position {
                emoji.tick();
            }
            emoji.reset();
            for (n, expected) in ticks.iter().enumerate() {
                assert_eq!(
                    emoji.tick(),
                    *expected,
                    "mode {mode}, reset at {position}, {n}"
                );
            }
        }
    }
}

#[test]
fn only_modes_14_to_17_have_an_emoji() {
    for mode in Mode::all() {
        assert_eq!(
            EmojiLoop::of(mode).is_some(),
            (14..=17).contains(&mode.number()),
            "mode {}",
            mode.number()
        );
    }
}
