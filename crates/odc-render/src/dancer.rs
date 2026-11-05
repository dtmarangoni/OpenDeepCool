//! The Dancer, mode 23 (`docs/spec/music.md`, Dancer): eleven poses typed from the spec's
//! pictures, which are what counts, and the rules that pick one at every tick from the column
//! levels, in the spec's order, with no clock.

use odc_protocol::Frame;

use crate::spectrum::COLUMNS;

/// The eleven poses, numbered as the rules use them; the whole grid, row 1 first.
const POSES: [[&str; Frame::SIZE]; 11] = [
    // 0 standing still
    [
        "..............",
        "..............",
        "......###.....",
        "......###.....",
        "......###.....",
        ".......#......",
        "......###.....",
        ".....#.#.#....",
        ".....#.#.#....",
        ".......#......",
        "......#.#.....",
        "......#.#.....",
        "......#.#.....",
        "......#.#.....",
    ],
    // 1 sway left
    [
        "..............",
        "..............",
        ".....###......",
        ".....###......",
        ".....###......",
        "......#.......",
        ".....###......",
        "....#.#.#.....",
        "...#...#.#....",
        ".......#......",
        "......#.#.....",
        "......#.#.....",
        ".....#..#.....",
        ".....#..#.....",
    ],
    // 2 sway right
    [
        "..............",
        "..............",
        ".......###....",
        ".......###....",
        ".......###....",
        "........#.....",
        ".......###....",
        "......#.#.#...",
        ".....#.#...#..",
        ".......#......",
        "......#.#.....",
        "......#.#.....",
        "......#..#....",
        "......#..#....",
    ],
    // 3 arms up
    [
        "..............",
        "..............",
        "....#.###.#...",
        "....#.###.#...",
        "....#.###.#...",
        ".....#.#.#....",
        "......###.....",
        ".......#......",
        ".......#......",
        ".......#......",
        "......#.#.....",
        "......#.#.....",
        ".....#...#....",
        ".....#...#....",
    ],
    // 4 squat
    [
        "..............",
        "..............",
        "..............",
        "..............",
        "......###.....",
        "......###.....",
        "......###.....",
        ".......#......",
        "......###.....",
        "...###.#.###..",
        ".......#......",
        ".....##.##....",
        ".....#...#....",
        ".....#...#....",
    ],
    // 5 left knee
    [
        "..............",
        "..............",
        "......###.....",
        "......###.....",
        "....#.###.....",
        ".....#.#......",
        "......###.....",
        ".......#.#....",
        ".......#..#...",
        ".......#......",
        ".....##.#.....",
        ".....#..#.....",
        ".....#..#.....",
        "........#.....",
    ],
    // 6 right knee
    [
        "..............",
        "..............",
        "......###.....",
        "......###.....",
        "......###.#...",
        ".......#.#....",
        "......###.....",
        ".....#.#......",
        "....#..#......",
        ".......#......",
        "......#.##....",
        "......#..#....",
        "......#..#....",
        "......#.......",
    ],
    // 7 point left
    [
        "..............",
        "..............",
        "......###.....",
        "...#..###.....",
        "....#.###.....",
        ".....#.#......",
        "......###.....",
        ".......#.#....",
        ".......##.....",
        ".......#......",
        "......#.#.....",
        "......#.#.....",
        "......#..#....",
        "......#..#....",
    ],
    // 8 point right
    [
        "..............",
        "..............",
        "......###.....",
        "......###..#..",
        "......###.#...",
        ".......#.#....",
        "......###.....",
        ".....#.#......",
        "......##......",
        ".......#......",
        "......#.#.....",
        "......#.#.....",
        ".....#..#.....",
        ".....#..#.....",
    ],
    // 9 arms out
    [
        "..............",
        "..............",
        "......###.....",
        "......###.....",
        "......###.....",
        ".......#......",
        "...#########..",
        ".......#......",
        ".......#......",
        ".......#......",
        "......#.#.....",
        "......#.#.....",
        ".....#...#....",
        ".....#...#....",
    ],
    // 10 jump
    [
        "....#.###.#...",
        "....#.###.#...",
        "....#.###.#...",
        ".....#.#.#....",
        "......###.....",
        ".......#......",
        ".......#......",
        ".......#......",
        "......#.#.....",
        ".....#...#....",
        ".....#...#....",
        "......#.#.....",
        "..............",
        "..............",
    ],
];

/// The big-move routine: arms up, squat, left knee, right knee, point left, point right, arms
/// out, jump, squat, arms up, then from the start again.
const ROUTINE: [u8; 10] = [3, 4, 5, 6, 7, 8, 9, 10, 4, 3];

/// The thresholds of the rules, on the 0–1 level scale.
const BEAT: f64 = 0.35;
const BIG_MOVE: f64 = 0.6;
const RISE: f64 = 0.12;
const QUIET: f64 = 0.1;

/// One of the eleven poses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pose(u8);

impl Pose {
    /// Standing still and straight: what silence shows.
    pub const STILL: Pose = Pose(0);

    /// Pose `number`, 0 to 10.
    pub fn new(number: u8) -> Option<Pose> {
        (usize::from(number) < POSES.len()).then_some(Pose(number))
    }

    /// Every pose, in order.
    pub fn all() -> impl Iterator<Item = Pose> {
        (0..11).map(Pose)
    }

    pub fn number(self) -> u8 {
        self.0
    }

    /// The pose on the whole grid, before rotation.
    pub fn frame(self) -> Frame {
        Frame::from_rows(&POSES[usize::from(self.0)])
    }
}

/// The Dancer's state (Rules, State).
#[derive(Clone, Debug)]
pub struct Dancer {
    pose: Pose,
    previous_bass: f64,
    since_beat: u32,
    quiet: u32,
    /// The next pose of the routine, as its place in `ROUTINE`.
    step: usize,
    next_sway_right: bool,
}

impl Default for Dancer {
    fn default() -> Dancer {
        Dancer::new()
    }
}

impl Dancer {
    /// As after a reset.
    pub fn new() -> Dancer {
        Dancer {
            pose: Pose::STILL,
            previous_bass: 0.0,
            since_beat: 99,
            quiet: 0,
            step: 0,
            next_sway_right: true,
        }
    }

    /// A reset (start-up, a switch into mode 23, a rotation change, another GPU, Refresh): pose 0,
    /// previous bass 0, 99 ticks since a beat, no quiet ticks, the routine's start, and the next
    /// sway to the right. The reset tick then runs the rules like any other (owner, 2026-11-05).
    pub fn reset(&mut self) {
        *self = Dancer::new();
    }

    /// The pose now.
    pub fn pose(&self) -> Pose {
        self.pose
    }

    /// The ticks since the last beat: 0 right after one, 99 or more after a reset.
    pub fn ticks_since_beat(&self) -> u32 {
        self.since_beat
    }

    /// One tick with this tick's column levels, 0 to 1 before any bar fall: the rules in order.
    pub fn tick(&mut self, levels: &[f64; COLUMNS]) -> Pose {
        let bass = levels[..4].iter().copied().fold(0.0, f64::max);
        let loudest = levels.iter().copied().fold(0.0, f64::max);
        // 1–2.
        self.since_beat = self.since_beat.saturating_add(1);
        if loudest < QUIET {
            self.quiet = self.quiet.saturating_add(1);
        } else {
            self.quiet = 0;
        }
        if self.quiet >= 3 {
            // 3. Silence.
            self.pose = Pose::STILL;
        } else if bass >= BEAT && bass - self.previous_bass >= RISE && self.since_beat >= 2 {
            // 4. A beat.
            self.since_beat = 0;
            if bass >= BIG_MOVE {
                self.pose = Pose(ROUTINE[self.step]);
                self.step = (self.step + 1) % ROUTINE.len();
            } else {
                self.sway();
            }
        } else if self.since_beat >= 8
            && loudest >= QUIET
            && (self.since_beat - 8).is_multiple_of(5)
        {
            // 5. No beat for a while.
            self.sway();
        }
        // 6. Otherwise the pose stays; 7.
        self.previous_bass = bass;
        self.pose
    }

    /// A sway: the side flips, right first after a reset.
    fn sway(&mut self) {
        self.pose = if self.next_sway_right {
            Pose(2)
        } else {
            Pose(1)
        };
        self.next_sway_right = !self.next_sway_right;
    }
}
