//! A 14 × 14 picture and its rotation (`docs/spec/display.md`, Rotation).

use std::fmt;

const SIZE: usize = 14;

/// One complete picture: 14 rows of 14 pixels, each on or off. Row 1 is at the top and column 1
/// at the left, as the cooler shows them (`docs/spec/protocol.md`, Frame).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frame {
    /// Bit `c − 1` of `rows[r − 1]` is pixel (r, c).
    rows: [u16; SIZE],
}

impl Frame {
    /// The width and the height, in pixels.
    pub const SIZE: usize = SIZE;

    /// Every pixel off.
    pub const fn blank() -> Self {
        Frame { rows: [0; SIZE] }
    }

    /// Whether pixel (`row`, `col`) is on, numbered from 1 as the spec numbers them.
    ///
    /// # Panics
    ///
    /// When `row` or `col` is outside 1–14, a programming error.
    pub fn get(&self, row: usize, col: usize) -> bool {
        let (r, c) = index(row, col);
        (self.rows[r] >> c) & 1 == 1
    }

    /// Switches pixel (`row`, `col`) on or off, numbered from 1 as the spec numbers them.
    ///
    /// # Panics
    ///
    /// When `row` or `col` is outside 1–14, a programming error.
    pub fn set(&mut self, row: usize, col: usize, on: bool) {
        let (r, c) = index(row, col);
        if on {
            self.rows[r] |= 1 << c;
        } else {
            self.rows[r] &= !(1 << c);
        }
    }

    /// A picture from 14 rows of 14 characters, `#` on and `.` off, row 1 first.
    ///
    /// # Panics
    ///
    /// When a row isn't 14 characters of `#` and `.`.
    pub fn from_rows(rows: &[&str; SIZE]) -> Self {
        let mut frame = Frame::blank();
        for (r, line) in rows.iter().enumerate() {
            assert_eq!(
                line.chars().count(),
                SIZE,
                "row {} isn't 14 characters: {line:?}",
                r + 1
            );
            for (c, ch) in line.chars().enumerate() {
                match ch {
                    '#' => frame.set(r + 1, c + 1, true),
                    '.' => {}
                    _ => panic!("row {}, column {}: {ch:?} isn't '#' or '.'", r + 1, c + 1),
                }
            }
        }
        frame
    }

    /// The whole frame turned `rotation` × 90° clockwise (`docs/spec/display.md`, Rotation).
    pub fn rotated(&self, rotation: Rotation) -> Self {
        let mut frame = *self;
        for _ in 0..rotation.setting() {
            frame = frame.quarter_turn();
        }
        frame
    }

    /// 90° clockwise: pixel (r, c) moves to (c, 15 − r).
    fn quarter_turn(&self) -> Self {
        let mut turned = Frame::blank();
        for r in 1..=SIZE {
            for c in 1..=SIZE {
                if self.get(r, c) {
                    turned.set(c, SIZE + 1 - r, true);
                }
            }
        }
        turned
    }
}

fn index(row: usize, col: usize) -> (usize, usize) {
    assert!(
        (1..=SIZE).contains(&row) && (1..=SIZE).contains(&col),
        "pixel ({row}, {col}) is outside the 14 × 14 frame"
    );
    (row - 1, col - 1)
}

/// Prints the picture, `#` on and `.` off, so a failing comparison shows both pictures.
impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Frame [")?;
        for r in 1..=SIZE {
            let line: String = (1..=SIZE)
                .map(|c| if self.get(r, c) { '#' } else { '.' })
                .collect();
            writeln!(f, "    {line}")?;
        }
        write!(f, "]")
    }
}

/// The Rotate Screen setting, 0–3: the number of quarter turns clockwise applied to the whole
/// finished frame (`docs/spec/display.md`, Rotation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rotation(u8);

impl Rotation {
    /// The four settings, 0 to 3.
    pub const ALL: [Rotation; 4] = [Rotation(0), Rotation(1), Rotation(2), Rotation(3)];

    /// The rotation for a setting value, or `None` outside 0–3.
    pub fn from_setting(value: u8) -> Option<Self> {
        (value <= 3).then_some(Rotation(value))
    }

    /// The setting value, 0–3.
    pub fn setting(self) -> u8 {
        self.0
    }

    /// One press of Rotate Screen: a quarter turn more, 3 wrapping to 0 (`docs/spec/settings.md`,
    /// Settings; parity checklist T3).
    pub fn next(self) -> Self {
        Rotation((self.0 + 1) % 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn any_frame() -> impl Strategy<Value = Frame> {
        proptest::array::uniform14(0u16..1 << SIZE).prop_map(|rows| Frame { rows })
    }

    fn lit(frame: &Frame) -> usize {
        (1..=SIZE)
            .map(|r| (1..=SIZE).filter(|&c| frame.get(r, c)).count())
            .sum()
    }

    fn quarter() -> Rotation {
        Rotation::from_setting(1).expect("setting 1")
    }

    proptest! {
        #[test]
        fn four_quarter_turns_give_back_the_original(f in any_frame()) {
            let q = quarter();
            prop_assert_eq!(f.rotated(q).rotated(q).rotated(q).rotated(q), f);
        }

        #[test]
        fn turning_by_r_equals_r_single_quarter_turns(f in any_frame(), r in 0u8..4) {
            let mut stepwise = f;
            for _ in 0..r {
                stepwise = stepwise.rotated(quarter());
            }
            prop_assert_eq!(f.rotated(Rotation::from_setting(r).expect("0–3")), stepwise);
        }

        #[test]
        fn turning_keeps_the_number_of_lit_pixels(f in any_frame(), r in 0u8..4) {
            let turned = f.rotated(Rotation::from_setting(r).expect("0–3"));
            prop_assert_eq!(lit(&turned), lit(&f));
        }
    }

    #[test]
    fn next_cycles_through_the_four_settings() {
        let settings: Vec<u8> =
            std::iter::successors(Some(Rotation::default()), |r| Some(r.next()))
                .take(5)
                .map(Rotation::setting)
                .collect();
        assert_eq!(settings, [0, 1, 2, 3, 0]);
        assert_eq!(Rotation::ALL.map(Rotation::setting), [0, 1, 2, 3]);
    }

    #[test]
    fn only_settings_0_to_3_are_rotations() {
        assert!((0..=3).all(|v| Rotation::from_setting(v).is_some()));
        assert_eq!(Rotation::from_setting(4), None);
        assert_eq!(Rotation::from_setting(u8::MAX), None);
    }

    #[test]
    fn pixels_are_numbered_from_1() {
        let mut f = Frame::blank();
        f.set(1, 1, true);
        f.set(14, 14, true);
        assert!(f.get(1, 1) && f.get(14, 14) && !f.get(1, 14));
        f.set(1, 1, false);
        assert!(!f.get(1, 1));
        assert_eq!(lit(&f), 1);
    }

    #[test]
    #[should_panic(expected = "outside the 14 × 14 frame")]
    fn row_0_is_outside_the_frame() {
        Frame::blank().get(0, 1);
    }

    #[test]
    #[should_panic(expected = "outside the 14 × 14 frame")]
    fn column_15_is_outside_the_frame() {
        Frame::blank().set(1, 15, true);
    }

    #[test]
    fn from_rows_reads_the_picture() {
        let mut rows = [".............."; SIZE];
        rows[0] = "#.............";
        rows[13] = ".............#";
        let f = Frame::from_rows(&rows);
        assert!(f.get(1, 1) && f.get(14, 14));
        assert_eq!(lit(&f), 2);
        assert!(format!("{f:?}").contains("    #.............\n"));
    }

    #[test]
    #[should_panic(expected = "isn't '#' or '.'")]
    fn from_rows_refuses_other_characters() {
        let mut rows = [".............."; SIZE];
        rows[2] = "......?.......";
        Frame::from_rows(&rows);
    }

    #[test]
    #[should_panic(expected = "isn't 14 characters")]
    fn from_rows_refuses_a_short_row() {
        let mut rows = [".............."; SIZE];
        rows[5] = ".............";
        Frame::from_rows(&rows);
    }
}
