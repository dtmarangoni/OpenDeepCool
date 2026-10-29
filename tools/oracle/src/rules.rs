//! OpenDeepCool's value and layout rules (`docs/spec/display.md`: Values, Unavailable values, the
//! static and scrolling layouts), in exact integer arithmetic: no floating point lies between a
//! reading and a digit.

use crate::drawing::{
    BLANK, DASH, DEG_C, DEG_F, DIGITS, Frame, Glyph, Kind, PERCENT, Pixel, SIZE, WATT, band_tops,
    is_music_value, put, value_mode,
};

/// An exact reading, `num / den` with `den > 0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ratio {
    num: i128,
    den: i128,
}

impl Ratio {
    pub fn whole(n: i64) -> Self {
        Ratio {
            num: n.into(),
            den: 1,
        }
    }

    /// An exact decimal as the scenario files write it: an optional `-`, digits, and optionally
    /// a point and more digits. `None` for anything else.
    pub fn decimal(text: &str) -> Option<Self> {
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
        let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if whole.is_empty() || !all_digits(whole) || !all_digits(fraction) || digits.ends_with('.')
        {
            return None;
        }
        let num: i128 = format!("{whole}{fraction}").parse().ok()?;
        let den = 10_i128.checked_pow(u32::try_from(fraction.len()).ok()?)?;
        Some(Ratio {
            num: if negative { -num } else { num },
            den,
        })
    }

    #[cfg(test)]
    pub fn new(num: i128, den: i128) -> Self {
        assert!(den > 0, "a ratio's denominator is positive");
        Ratio { num, den }
    }
}

/// Every number drawn is at most 9999 (owner, 2026-10-25).
const LARGEST: i128 = 9999;

/// The nearest whole number to `num / den`, halves up: floor(num / den + 1/2).
fn nearest(num: i128, den: i128) -> i128 {
    (2 * num + den).div_euclid(2 * den)
}

/// The whole number drawn for a reading, or `None` for dashes: a reading that is unavailable, or
/// negative, which no sensor can give. °F is rounded once, from the exact °C.
pub fn number(reading: Option<Ratio>, kind: Kind, fahrenheit: bool) -> Option<u32> {
    let x = reading?;
    if x.num < 0 {
        return None;
    }
    let n = if kind == Kind::Temperature && fahrenheit {
        // x · 9/5 + 32 = (9 · num + 160 · den) / (5 · den)
        nearest(9 * x.num + 160 * x.den, 5 * x.den)
    } else {
        nearest(x.num, x.den)
    };
    Some(u32::try_from(n.min(LARGEST)).expect("a number from 0 to 9999"))
}

fn symbol(kind: Kind, fahrenheit: bool) -> Glyph {
    match kind {
        Kind::Usage => PERCENT,
        Kind::Power => WATT,
        Kind::Temperature if fahrenheit => DEG_F,
        Kind::Temperature => DEG_C,
    }
}

/// Five rows: each digit, 3 wide, and a blank column, then the symbol. Dashes take two digit
/// places.
fn strip(n: Option<u32>, symbol: Glyph) -> Vec<Vec<bool>> {
    let glyphs: Vec<Glyph> = match n {
        None => vec![DASH, DASH],
        Some(n) => n
            .to_string()
            .bytes()
            .map(|b| DIGITS[usize::from(b - b'0')])
            .collect(),
    };
    let width = 4 * glyphs.len() + symbol[0].len();
    let mut rows = vec![vec![false; width]; 5];
    for (i, glyph) in glyphs.iter().enumerate() {
        put(&mut rows, 0, 4 * i, glyph);
    }
    put(&mut rows, 0, 4 * glyphs.len(), symbol);
    rows
}

/// What a mode with values shows, for the frame files: one picture, or one cycle.
pub enum Picture {
    /// Every value is 99 or less, or dashes.
    Static(Frame),
    /// A value is 100 or more: the strip moves left, one column per tick.
    Scrolling(Cycle),
}

/// One scrolling cycle, home first.
pub struct Cycle {
    strip: Strip,
}

impl Cycle {
    /// The number of frames in one cycle.
    pub fn len(&self) -> usize {
        usize::try_from(self.strip.cycle()).expect("a positive cycle")
    }

    /// The frame `k` ticks after home: the strip's first column at column 2 − `k`.
    pub fn window(&self, k: usize) -> Frame {
        let k = i64::try_from(k).expect("a small number of ticks");
        self.strip.frame_at(self.strip.home() - k)
    }
}

/// A mode's picture as the movement procedure sees it (`display.md`, Movement, precisely): the
/// strip of the mode's values over the full height, both bands ending at the same column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Strip {
    /// 14 rows of the strip's columns.
    rows: Vec<Vec<bool>>,
    /// Where the static layout puts its first column, from 1; 2 when it's wider than 14.
    home: i64,
    /// A value is 100 or more.
    needs_scroll: bool,
}

impl Strip {
    /// *w*, the strip's width in columns.
    pub fn width(&self) -> i64 {
        i64::try_from(self.rows[0].len()).expect("a narrow strip")
    }

    /// *L* = *w* + max(4, 21 − *w*): the strip and at least 4 blank columns, at least 21 in all.
    pub fn cycle(&self) -> i64 {
        let w = self.width();
        w + 4.max(21 - w)
    }

    /// *h*, the home column.
    pub fn home(&self) -> i64 {
        self.home
    }

    pub fn needs_scroll(&self) -> bool {
        self.needs_scroll
    }

    /// The frame with a copy of the strip starting at column `x` (from 1, possibly off the
    /// screen), and copies every *L* columns either side of it.
    pub fn frame_at(&self, x: i64) -> Frame {
        let (w, length) = (self.width(), self.cycle());
        let mut g = BLANK;
        for (out, row) in g.iter_mut().zip(&self.rows) {
            for (c, px) in (1..).zip(out.iter_mut()) {
                let s = (c - x).rem_euclid(length);
                if s < w && row[usize::try_from(s).expect("a column of the strip")] {
                    *px = Pixel::On;
                }
            }
        }
        g
    }
}

/// The strip of a mode with values, its readings in the order of the mode table.
pub fn strip_of(mode: u16, readings: &[Option<Ratio>], fahrenheit: bool) -> Strip {
    let (kind, names) = value_mode(mode).expect("a mode with values");
    assert_eq!(readings.len(), names.len(), "one reading per value shown");
    let tops = band_tops(names.len(), is_music_value(mode));
    let symbol = symbol(kind, fahrenheit);
    let numbers: Vec<Option<u32>> = readings
        .iter()
        .map(|&r| number(r, kind, fahrenheit))
        .collect();
    let strips: Vec<Vec<Vec<bool>>> = numbers.iter().map(|&n| strip(n, symbol)).collect();
    let wide = strips.iter().map(|s| s[0].len()).max().expect("a value");
    // One strip of columns over the full height; both values end at the same column.
    let mut rows = vec![vec![false; wide]; SIZE];
    for (&top, s) in tops.iter().zip(&strips) {
        let w = s[0].len();
        for (r, band) in s.iter().enumerate() {
            rows[top + r][wide - w..].copy_from_slice(band);
        }
    }
    // One value centred, its odd spare column on the left; two values ending at column 14; a
    // strip wider than the screen has its first column at column 2.
    let home = if wide > SIZE {
        2
    } else if names.len() == 1 {
        let spare = SIZE - wide;
        spare - spare / 2 + 1
    } else {
        SIZE - wide + 1
    };
    Strip {
        rows,
        home: i64::try_from(home).expect("a column"),
        needs_scroll: numbers.iter().any(|n| n.is_some_and(|n| n >= 100)),
    }
}

/// The picture of a mode with values, its readings in the order of the mode table: the strip at
/// home, still or over one cycle.
pub fn picture(mode: u16, readings: &[Option<Ratio>], fahrenheit: bool) -> Picture {
    let strip = strip_of(mode, readings, fahrenheit);
    if strip.needs_scroll() {
        Picture::Scrolling(Cycle { strip })
    } else {
        Picture::Static(strip.frame_at(strip.home()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::row;

    // display.md Values, and frames/README.md: every measurement to the nearest whole number,
    // halves up; °F once from the exact °C; at most 9999.
    #[test]
    fn numbers_follow_the_value_rules() {
        let n = |r: Ratio, kind: Kind, fahrenheit: bool| number(Some(r), kind, fahrenheit);
        assert_eq!(n(Ratio::whole(57), Kind::Temperature, true), Some(135));
        assert_eq!(n(Ratio::whole(7), Kind::Temperature, true), Some(45));
        assert_eq!(n(Ratio::whole(5536), Kind::Temperature, true), Some(9997));
        assert_eq!(n(Ratio::whole(5537), Kind::Temperature, true), Some(9999));
        assert_eq!(
            n(Ratio::new(55375, 10), Kind::Temperature, true),
            Some(9999)
        );
        assert_eq!(n(Ratio::whole(10_000), Kind::Power, false), Some(9999));
        assert_eq!(n(Ratio::new(1, 2), Kind::Usage, false), Some(1));
        assert_eq!(n(Ratio::new(-1, 2), Kind::Usage, false), None);
        assert_eq!(number(None, Kind::Usage, false), None);
    }

    #[test]
    fn decimals_are_read_exactly() {
        assert_eq!(Ratio::decimal("99.49"), Some(Ratio::new(9949, 100)));
        assert_eq!(Ratio::decimal("-0.5"), Some(Ratio::new(-5, 10)));
        assert_eq!(
            Ratio::decimal("1000000000"),
            Some(Ratio::whole(1_000_000_000))
        );
        for bad in ["", "-", ".5", "5.", "1e3", "+1", "4 2", "unavailable"] {
            assert_eq!(Ratio::decimal(bad), None, "{bad:?}");
        }
    }

    // display.md Unavailable values, static layout: a dash in the tens place (columns 2–4) and in
    // the units place (columns 6–8); the symbol stays: `--%`.
    #[test]
    fn an_unavailable_value_draws_dashes_in_both_digit_places() {
        let Picture::Static(g) = picture(1, &[None], false) else {
            panic!("dashes are static");
        };
        let want = [
            "..............",
            "..............",
            "..............",
            "..............",
            "..............",
            ".........##..#",
            ".........##.#.",
            ".###.###...#..",
            "..........#.##",
            ".........#..##",
            "..............",
            "..............",
            "..............",
            "..............",
        ]
        .map(row);
        assert_eq!(g, want);
    }
}
