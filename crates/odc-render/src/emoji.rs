//! The emoji animations of modes 14–17 (`docs/spec/music.md`, Emoji animations), typed from the
//! spec's pictures: rows 1–7, `#` lit. Each is a fixed loop on the 100 ms tick that the music
//! doesn't drive, and a reset restarts it (owner, 2026-10-25).

use odc_protocol::Frame;

use crate::mode::Mode;

/// One picture, rows 1–7; the rows below are the spectrum's.
type Picture = [&'static str; 7];

/// One step of a loop: the picture, and how many 100 ms ticks it shows.
type Step = (&'static Picture, u8);

const EMOJI_1: [Picture; 2] = [
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
];

const EMOJI_2: [Picture; 2] = [
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
];

const EMOJI_3: [Picture; 2] = [
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
];

const EMOJI_4: [Picture; 4] = [
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
];

/// Emoji 1: picture 2 for 400 ms, then 1 → 2 → 1 → 2 → 1, 100 ms each.
const LOOP_1: [Step; 6] = [
    (&EMOJI_1[1], 4),
    (&EMOJI_1[0], 1),
    (&EMOJI_1[1], 1),
    (&EMOJI_1[0], 1),
    (&EMOJI_1[1], 1),
    (&EMOJI_1[0], 1),
];

/// Emoji 2: picture 1 for 400 ms, then 2 → 1 → 2 → 1 → 2, 100 ms each.
const LOOP_2: [Step; 6] = [
    (&EMOJI_2[0], 4),
    (&EMOJI_2[1], 1),
    (&EMOJI_2[0], 1),
    (&EMOJI_2[1], 1),
    (&EMOJI_2[0], 1),
    (&EMOJI_2[1], 1),
];

/// Emoji 3: picture 1 for 400 ms, then 2 → 1 → 2 → 1 → 2, 100 ms each.
const LOOP_3: [Step; 6] = [
    (&EMOJI_3[0], 4),
    (&EMOJI_3[1], 1),
    (&EMOJI_3[0], 1),
    (&EMOJI_3[1], 1),
    (&EMOJI_3[0], 1),
    (&EMOJI_3[1], 1),
];

/// Emoji 4: picture 1 (100 ms) → 2 (100) → 3 (100) → 2 (100) → 1 (200) → 4 (500).
const LOOP_4: [Step; 6] = [
    (&EMOJI_4[0], 1),
    (&EMOJI_4[1], 1),
    (&EMOJI_4[2], 1),
    (&EMOJI_4[1], 1),
    (&EMOJI_4[0], 2),
    (&EMOJI_4[3], 5),
];

/// One emoji's loop and where it is.
#[derive(Clone, Debug)]
pub struct EmojiLoop {
    steps: &'static [Step],
    /// The tick of the loop the next frame shows, from 0.
    tick: usize,
}

impl EmojiLoop {
    /// The loop of `mode`, 14 to 17, at its start; `None` for any other mode.
    pub fn of(mode: Mode) -> Option<EmojiLoop> {
        let steps: &'static [Step] = match mode.number() {
            14 => &LOOP_1,
            15 => &LOOP_2,
            16 => &LOOP_3,
            17 => &LOOP_4,
            _ => return None,
        };
        Some(EmojiLoop { steps, tick: 0 })
    }

    /// A reset: the next frame shows the loop's first picture.
    pub fn reset(&mut self) {
        self.tick = 0;
    }

    /// The loop's length, in 100 ms ticks.
    pub fn ticks(&self) -> usize {
        self.steps.iter().map(|&(_, n)| usize::from(n)).sum()
    }

    /// This tick's picture, in rows 1–7 of an otherwise blank frame; the loop then moves on one
    /// tick.
    pub fn tick(&mut self) -> Frame {
        let mut left = self.tick;
        let picture = self
            .steps
            .iter()
            .find_map(|&(picture, n)| match left.checked_sub(usize::from(n)) {
                Some(rest) => {
                    left = rest;
                    None
                }
                None => Some(picture),
            })
            .expect("the tick is within the loop");
        self.tick = (self.tick + 1) % self.ticks();
        let mut frame = Frame::blank();
        for (r, row) in picture.iter().enumerate() {
            for (c, pixel) in row.bytes().enumerate() {
                frame.set(r + 1, c + 1, pixel == b'#');
            }
        }
        frame
    }
}
