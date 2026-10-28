//! The drawing parts of the spec: the glyphs, the emoji pictures, the mode table, and how a
//! picture is put together and turned (`docs/spec/display.md`, `docs/spec/music.md`).

/// The display is 14 × 14 pixels.
pub const SIZE: usize = 14;

/// One pixel of an expected frame (`docs/spec/frames/README.md`, Format).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pixel {
    Off,
    On,
    /// A spectrum pixel: it depends on the sound, so the files show it as `?`.
    Spectrum,
}

pub type Frame = [[Pixel; SIZE]; SIZE];

pub const BLANK: Frame = [[Pixel::Off; SIZE]; SIZE];

/// A glyph's rows, top to bottom; `#` is on.
pub type Glyph = &'static [&'static str];

/// The digits 0–9, 3 wide and 5 tall.
pub const DIGITS: [Glyph; 10] = [
    &["###", "#.#", "#.#", "#.#", "###"],
    &[".#.", "##.", ".#.", ".#.", "###"],
    &["###", "..#", "###", "#..", "###"],
    &["###", "..#", "###", "..#", "###"],
    &["#.#", "#.#", "###", "..#", "..#"],
    &["###", "#..", "###", "..#", "###"],
    &["###", "#..", "###", "#.#", "###"],
    &["###", "..#", ".#.", ".#.", ".#."],
    &["###", "#.#", "###", "#.#", "###"],
    &["###", "#.#", "###", "..#", "###"],
];
pub const PERCENT: Glyph = &["##..#", "##.#.", "..#..", ".#.##", "#..##"];
/// The degree dot and a C, 4 wide.
pub const DEG_C: Glyph = &["#...", "..##", ".#..", ".#..", "..##"];
/// The degree dot, a blank column and an F.
pub const DEG_F: Glyph = &["#.###", "..#..", "..###", "..#..", "..#.."];
pub const WATT: Glyph = &[".....", "#.#.#", "#.#.#", "#.#.#", ".#.#."];
/// Drawn in place of each digit of a value that can't be read (display.md, Unavailable values).
pub const DASH: Glyph = &["...", "...", "###", "...", "..."];

/// What a mode with values shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Usage,
    Temperature,
    Power,
}

/// The kind and the readings, by their names in the frame files, of each mode that shows values.
/// Modes 18–22 draw them in Music's value area, above the spectrum.
pub fn value_mode(mode: u16) -> Option<(Kind, &'static [&'static str])> {
    let shown: (Kind, &'static [&'static str]) = match mode {
        1 | 20 => (Kind::Usage, &["CPU Usage"]),
        2 | 21 => (Kind::Usage, &["Gpu usage"]),
        3 | 22 => (Kind::Usage, &["Memory Load"]),
        4 => (Kind::Usage, &["CPU Usage", "Gpu usage"]),
        5 => (Kind::Usage, &["CPU Usage", "Memory Load"]),
        6 => (Kind::Usage, &["Gpu usage", "Memory Load"]),
        7 | 18 => (Kind::Temperature, &["CPU Temperature"]),
        8 | 19 => (Kind::Temperature, &["Gpu temperature"]),
        9 => (Kind::Temperature, &["CPU Temperature", "Gpu temperature"]),
        10 => (Kind::Power, &["CPU Power"]),
        11 => (Kind::Power, &["Gpu power"]),
        12 => (Kind::Power, &["CPU Power", "Gpu power"]),
        _ => return None,
    };
    Some(shown)
}

/// Modes 18–22: a value above the spectrum.
pub fn is_music_value(mode: u16) -> bool {
    (18..=22).contains(&mode)
}

/// Modes 14–17: an emoji above the spectrum.
pub fn is_emoji(mode: u16) -> bool {
    (14..=17).contains(&mode)
}

/// The emoji pictures of modes 14–17, rows 1–7 (music.md); row 8 is off.
pub fn emoji(mode: u16) -> &'static [[&'static str; 7]] {
    match mode {
        14 => &EMOJI_1,
        15 => &EMOJI_2,
        16 => &EMOJI_3,
        17 => &EMOJI_4,
        _ => &[],
    }
}

const EMOJI_1: [[&str; 7]; 2] = [
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
const EMOJI_2: [[&str; 7]; 2] = [
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
const EMOJI_3: [[&str; 7]; 2] = [
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
const EMOJI_4: [[&str; 7]; 4] = [
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

/// The 0-based top row of each value's 5-row band: one value in rows 6–10, two in rows 2–6 and
/// 9–13, and Music's value area in rows 2–6.
pub fn band_tops(values: usize, music: bool) -> &'static [usize] {
    if music {
        &[1]
    } else if values == 1 {
        &[5]
    } else {
        &[1, 8]
    }
}

/// Lights a glyph's pixels in rows of on and off, cut off at the right edge.
pub fn put(canvas: &mut [Vec<bool>], top: usize, left: usize, glyph: Glyph) {
    for (r, line) in glyph.iter().enumerate() {
        let row = &mut canvas[top + r];
        for (c, ch) in line.bytes().enumerate() {
            if ch == b'#' && left + c < row.len() {
                row[left + c] = true;
            }
        }
    }
}

/// One row of a picture written with `#` and `.`.
pub fn row(line: &str) -> [Pixel; SIZE] {
    let mut row = [Pixel::Off; SIZE];
    for (px, ch) in row.iter_mut().zip(line.bytes()) {
        if ch == b'#' {
            *px = Pixel::On;
        }
    }
    row
}

/// The rows above the spectrum, then spectrum pixels from row `first` (0-based) to row 13, and
/// the floor, the bottom row, always lit (music.md).
pub fn with_spectrum(top: &[[Pixel; SIZE]], first: usize) -> Frame {
    let mut g = BLANK;
    g[..top.len()].copy_from_slice(top);
    for row in &mut g[first..SIZE - 1] {
        *row = [Pixel::Spectrum; SIZE];
    }
    g[SIZE - 1] = [Pixel::On; SIZE];
    g
}

/// Turns the whole finished frame `turns` × 90° clockwise (display.md, Rotation).
pub fn rotate_cw(frame: &Frame, turns: u8) -> Frame {
    let mut g = *frame;
    for _ in 0..turns % 4 {
        let mut turned = BLANK;
        for (r, row) in turned.iter_mut().enumerate() {
            for (c, px) in row.iter_mut().enumerate() {
                *px = g[SIZE - 1 - c][r];
            }
        }
        g = turned;
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quarter_turn_is_clockwise_and_four_give_back_the_frame() {
        let mut g = BLANK;
        g[0][0] = Pixel::On;
        g[SIZE - 1][SIZE - 1] = Pixel::Spectrum;
        let once = rotate_cw(&g, 1);
        assert_eq!(once[0][SIZE - 1], Pixel::On, "top left goes to top right");
        assert_eq!(
            once[SIZE - 1][0],
            Pixel::Spectrum,
            "bottom right goes to bottom left"
        );
        assert_eq!(rotate_cw(&once, 3), g);
        assert_eq!(rotate_cw(&g, 4), g);
    }
}
