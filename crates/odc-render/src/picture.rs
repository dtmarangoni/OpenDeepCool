//! One or two values as the strip the display shows (`docs/spec/display.md`: Bands; Static
//! layout, OpenDeepCool: alignment; Values, OpenDeepCool: one smooth movement; Movement,
//! precisely). The static picture and the scrolling text are the same strip: a static picture is
//! the strip stopped at home.

use odc_protocol::Frame;

use crate::glyph::{self, DASH, DIGITS, Glyph};
use crate::value::Shown;

const SIZE: usize = Frame::SIZE;
const SCREEN: i32 = SIZE as i32;

/// One value as drawn: its number or dashes, then its symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    pub shown: Shown,
    pub symbol: &'static Glyph,
}

/// A picture as the movement procedure sees it: the strip of columns over the full height, where
/// the static layout puts it, and whether it must scroll.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    /// The strip, left column first; each column is 14 pixels, row 1 first.
    columns: Vec<[bool; SIZE]>,
    home: i32,
    must_scroll: bool,
}

impl Picture {
    /// *w*, the strip's width in columns.
    pub fn width(&self) -> i32 {
        i32::try_from(self.columns.len()).expect("a narrow strip")
    }

    /// *L* = *w* + max(4, 21 − *w*): the strip and at least 4 blank columns, 21 at least.
    pub fn cycle(&self) -> i32 {
        let w = self.width();
        w + 4.max(21 - w)
    }

    /// *h*: the column, from 1, where the static layout puts the strip's first column; 2 when the
    /// strip is wider than the screen.
    pub fn home(&self) -> i32 {
        self.home
    }

    /// Whether a value is 100 or more.
    pub fn must_scroll(&self) -> bool {
        self.must_scroll
    }

    /// The frame with a copy of the strip starting at column `x`, which may be off the screen,
    /// and further copies every *L* columns.
    pub fn frame_at(&self, x: i32) -> Frame {
        let mut frame = Frame::blank();
        let cycle = self.cycle();
        for column in 1..=SCREEN {
            let s = (column - x).rem_euclid(cycle);
            let Some(pixels) = usize::try_from(s).ok().and_then(|s| self.columns.get(s)) else {
                continue;
            };
            for (row, &lit) in pixels.iter().enumerate() {
                if lit {
                    frame.set(row + 1, usize::try_from(column).expect("1–14"), true);
                }
            }
        }
        frame
    }
}

/// The top row of each band: one value in rows 6–10, two values in rows 2–6 and 9–13.
fn band_tops(values: usize) -> &'static [usize] {
    match values {
        1 => &[6],
        2 => &[2, 9],
        _ => panic!("a picture shows one or two values, not {values}"),
    }
}

/// One or two values as one picture: each value's digits in places of 4 columns (3 for the digit,
/// 1 blank), then its symbol; dashes take two places. Two values end at the same column.
pub fn layout(bands: &[Band]) -> Picture {
    let tops = band_tops(bands.len());
    let strips: Vec<Vec<[bool; 5]>> = bands.iter().map(band_columns).collect();
    let width = strips.iter().map(Vec::len).max().expect("a value");
    let mut columns = vec![[false; SIZE]; width];
    for (&top, strip) in tops.iter().zip(&strips) {
        let first = width - strip.len();
        for (column, pixels) in columns[first..].iter_mut().zip(strip) {
            column[top - 1..top + 4].copy_from_slice(pixels);
        }
    }
    let home = if width > SIZE {
        2
    } else {
        let spare = SIZE - width;
        // One value centred, an odd spare column on the left; two values ending at column 14.
        let left = if bands.len() == 1 {
            spare - spare / 2
        } else {
            spare
        };
        left + 1
    };
    Picture {
        columns,
        home: i32::try_from(home).expect("a column"),
        must_scroll: bands
            .iter()
            .any(|band| matches!(band.shown, Shown::Number(n) if n >= 100)),
    }
}

/// A value's columns, five pixels each.
fn band_columns(band: &Band) -> Vec<[bool; 5]> {
    let places: Vec<&Glyph> = match band.shown {
        Shown::Dashes => vec![&DASH, &DASH],
        Shown::Number(n) => n
            .to_string()
            .bytes()
            .map(|digit| &DIGITS[usize::from(digit - b'0')])
            .collect(),
    };
    let mut columns = Vec::new();
    for glyph in places {
        columns.extend(glyph::columns(glyph));
        columns.push([false; 5]);
    }
    columns.extend(glyph::columns(band.symbol));
    columns
}
