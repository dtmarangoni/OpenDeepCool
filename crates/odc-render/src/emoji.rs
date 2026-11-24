//! The emoji animations of modes 14–16 and 24–26 (`docs/spec/music.md`, Emoji animations), typed
//! from the spec's pictures: `#` lit. Modes 14–16 draw a picture in rows 1–7, over the spectrum
//! in rows 9–14; modes 24–26 draw the same three animations alone, 14 rows tall, with no
//! spectrum. Each is a fixed loop on the 100 ms tick, which the music doesn't shape but starts and
//! stops: an emoji stands on its first picture until sound plays, runs while it does, and is back
//! on its first picture a second after the sound stops (owner, 2026-11-23). A reset restarts it
//! (owner, 2026-10-25). The animations are OpenDeepCool's own: DeepCreative's four are gone.

use odc_protocol::Frame;

use crate::mode::Mode;
use crate::spectrum::COLUMNS;

/// How many quiet ticks it takes an emoji to go back to rest: one second (owner, 2026-11-23).
pub const REST_AFTER: u32 = 10;

/// Whether a tick's 14 column levels (`music.md`, point 3: 0 at −75 dB or less) hold any sound: a
/// column above 0. Digital silence, which is what a PC that plays nothing gives, is all 0.
pub fn is_sound(levels: &[f64; COLUMNS]) -> bool {
    levels.iter().any(|&level| level > 0.0)
}

/// One picture: its rows from row 1 down, 14 characters each. With a spectrum a picture has 7
/// rows and the rows below are the spectrum's; alone it has all 14.
type Picture = &'static [&'static str];

/// One step of a loop: the picture's index, and how many 100 ms ticks it shows.
type Step = (usize, u8);

/// Mode 14, Lub-dub and the spectrum: the pictures.
const LUB_DUB_AND_SPECTRUM: [&[&str]; 3] = [
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
];

/// Its loop: each picture's index and its 100 ms ticks, 10 ticks in all.
const LUB_DUB_AND_SPECTRUM_LOOP: [(usize, u8); 4] = [(0, 6), (1, 1), (0, 1), (2, 2)];

/// Mode 15, Coffee break and the spectrum: the pictures.
const COFFEE_BREAK_AND_SPECTRUM: [&[&str]; 6] = [
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
];

/// Its loop: each picture's index and its 100 ms ticks, 12 ticks in all.
const COFFEE_BREAK_AND_SPECTRUM_LOOP: [(usize, u8); 6] =
    [(0, 2), (1, 2), (2, 2), (3, 2), (4, 2), (5, 2)];

/// Mode 16, Cat tail and the spectrum: the pictures.
const CAT_TAIL_AND_SPECTRUM: [&[&str]; 3] = [
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
];

/// Its loop: each picture's index and its 100 ms ticks, 7 ticks in all.
const CAT_TAIL_AND_SPECTRUM_LOOP: [(usize, u8); 4] = [(0, 3), (1, 1), (2, 2), (1, 1)];

/// Mode 24, Lub-dub alone: the pictures.
const LUB_DUB_ALONE: [&[&str]; 4] = [
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
];

/// Its loop: each picture's index and its 100 ms ticks, 12 ticks in all.
const LUB_DUB_ALONE_LOOP: [(usize, u8); 5] = [(0, 7), (1, 1), (0, 1), (2, 2), (3, 1)];

/// Mode 25, Coffee break alone: the pictures.
const COFFEE_BREAK_ALONE: [&[&str]; 4] = [
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
];

/// Its loop: each picture's index and its 100 ms ticks, 12 ticks in all.
const COFFEE_BREAK_ALONE_LOOP: [(usize, u8); 4] = [(0, 3), (1, 3), (2, 3), (3, 3)];

/// Mode 26, Cat tail alone: the pictures.
const CAT_TAIL_ALONE: [&[&str]; 5] = [
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
];

/// Its loop: each picture's index and its 100 ms ticks, 12 ticks in all.
const CAT_TAIL_ALONE_LOOP: [(usize, u8); 8] = [
    (0, 3),
    (4, 1),
    (0, 2),
    (1, 1),
    (2, 1),
    (3, 2),
    (2, 1),
    (1, 1),
];

/// One animation: its pictures, and its loop.
struct Animation {
    pictures: &'static [Picture],
    steps: &'static [Step],
}

const LUB_DUB: Animation = Animation {
    pictures: &LUB_DUB_AND_SPECTRUM,
    steps: &LUB_DUB_AND_SPECTRUM_LOOP,
};
const COFFEE_BREAK: Animation = Animation {
    pictures: &COFFEE_BREAK_AND_SPECTRUM,
    steps: &COFFEE_BREAK_AND_SPECTRUM_LOOP,
};
const CAT_TAIL: Animation = Animation {
    pictures: &CAT_TAIL_AND_SPECTRUM,
    steps: &CAT_TAIL_AND_SPECTRUM_LOOP,
};
const LUB_DUB_BIG: Animation = Animation {
    pictures: &LUB_DUB_ALONE,
    steps: &LUB_DUB_ALONE_LOOP,
};
const COFFEE_BREAK_BIG: Animation = Animation {
    pictures: &COFFEE_BREAK_ALONE,
    steps: &COFFEE_BREAK_ALONE_LOOP,
};
const CAT_TAIL_BIG: Animation = Animation {
    pictures: &CAT_TAIL_ALONE,
    steps: &CAT_TAIL_ALONE_LOOP,
};

/// One emoji's loop and where it is.
#[derive(Clone, Debug)]
pub struct EmojiLoop {
    pictures: &'static [Picture],
    steps: &'static [Step],
    /// The tick of the loop the next frame shows, from 0.
    tick: usize,
    /// The ticks since sound last played, up to [`REST_AFTER`], which is the loop at rest.
    quiet: u32,
}

impl EmojiLoop {
    /// The loop of `mode`, 14–16 (with a spectrum) or 24–26 (alone), at its start; `None` for any
    /// other mode.
    pub fn of(mode: Mode) -> Option<EmojiLoop> {
        let animation = match mode.number() {
            14 => &LUB_DUB,
            15 => &COFFEE_BREAK,
            16 => &CAT_TAIL,
            24 => &LUB_DUB_BIG,
            25 => &COFFEE_BREAK_BIG,
            26 => &CAT_TAIL_BIG,
            _ => return None,
        };
        Some(EmojiLoop {
            pictures: animation.pictures,
            steps: animation.steps,
            tick: 0,
            quiet: REST_AFTER,
        })
    }

    /// A reset: the loop is at rest, so the next frame shows its first picture, and runs on from
    /// there if sound plays.
    pub fn reset(&mut self) {
        self.tick = 0;
        self.quiet = REST_AFTER;
    }

    /// The loop's length, in 100 ms ticks.
    pub fn ticks(&self) -> usize {
        self.steps.iter().map(|&(_, n)| usize::from(n)).sum()
    }

    /// This tick's picture, from row 1 of an otherwise blank frame. While `sound` plays, and for
    /// [`REST_AFTER`] ticks after it stops, the loop then moves on one tick; after that it is at
    /// rest, on its first picture, and starts again from it with the next sound.
    pub fn tick(&mut self, sound: bool) -> Frame {
        if sound {
            self.quiet = 0;
        } else if self.quiet < REST_AFTER {
            self.quiet += 1;
        }
        let running = self.quiet < REST_AFTER;
        if !running {
            self.tick = 0;
        }
        let mut left = self.tick;
        let picture = self
            .steps
            .iter()
            .find_map(|&(picture, n)| match left.checked_sub(usize::from(n)) {
                Some(rest) => {
                    left = rest;
                    None
                }
                None => Some(self.pictures[picture]),
            })
            .expect("the tick is within the loop");
        if running {
            self.tick = (self.tick + 1) % self.ticks();
        }
        let mut frame = Frame::blank();
        for (r, row) in picture.iter().enumerate() {
            for (c, pixel) in row.bytes().enumerate() {
                frame.set(r + 1, c + 1, pixel == b'#');
            }
        }
        frame
    }
}
