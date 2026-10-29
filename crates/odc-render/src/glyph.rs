//! The glyphs of `docs/spec/display.md` (Glyphs; Unavailable values), typed from the spec's
//! pictures: five rows, top first, `#` lit. Phase 8 adds `°C`, `°F` and `W`.

/// Five rows of one width, `#` lit and `.` dark.
pub type Glyph = [&'static str; 5];

/// The digits 0–9, 3 × 5.
pub const DIGITS: [Glyph; 10] = [
    ["###", "#.#", "#.#", "#.#", "###"],
    [".#.", "##.", ".#.", ".#.", "###"],
    ["###", "..#", "###", "#..", "###"],
    ["###", "..#", "###", "..#", "###"],
    ["#.#", "#.#", "###", "..#", "..#"],
    ["###", "#..", "###", "..#", "###"],
    ["###", "#..", "###", "#.#", "###"],
    ["###", "..#", ".#.", ".#.", ".#."],
    ["###", "#.#", "###", "#.#", "###"],
    ["###", "#.#", "###", "..#", "###"],
];

/// `%`, 5 × 5.
pub const PERCENT: Glyph = ["##..#", "##.#.", "..#..", ".#.##", "#..##"];

/// The dash drawn in each digit place of a value that can't be read, 3 × 5.
pub const DASH: Glyph = ["...", "...", "###", "...", "..."];

/// The glyph's columns, left first, each five pixels, top first.
pub fn columns(glyph: &Glyph) -> Vec<[bool; 5]> {
    (0..glyph[0].len())
        .map(|c| std::array::from_fn(|r| glyph[r].as_bytes()[c] == b'#'))
        .collect()
}
