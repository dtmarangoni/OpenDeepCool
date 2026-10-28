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

/// What a mode with values shows.
pub enum Picture {
    /// Every value is 99 or less, or dashes.
    Static(Frame),
    /// A value is 100 or more: the strip moves left, one column per tick.
    Scrolling(Cycle),
}

/// The scrolling strip followed by blank columns: 14 rows of the whole cycle.
pub struct Cycle {
    rows: Vec<Vec<bool>>,
}

impl Cycle {
    /// The number of frames in one cycle.
    pub fn len(&self) -> usize {
        self.rows[0].len()
    }

    /// The frame `k` ticks after home: window column `c` shows cycle column `(c − 1 + k) mod
    /// length`, so at home the strip's first column is at column 2.
    pub fn window(&self, k: usize) -> Frame {
        let length = self.len();
        let mut g = BLANK;
        for (out, row) in g.iter_mut().zip(&self.rows) {
            for (c, px) in out.iter_mut().enumerate() {
                if row[(c + k + length - 1) % length] {
                    *px = Pixel::On;
                }
            }
        }
        g
    }
}

/// The picture of a mode with values, its readings in the order of the mode table.
pub fn picture(mode: u16, readings: &[Option<Ratio>], fahrenheit: bool) -> Picture {
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
    let mut band = vec![vec![false; wide]; SIZE];
    for (&top, s) in tops.iter().zip(&strips) {
        let w = s[0].len();
        for (r, rows) in s.iter().enumerate() {
            band[top + r][wide - w..].copy_from_slice(rows);
        }
    }
    if numbers.iter().all(|n| n.is_none_or(|n| n <= 99)) {
        // One value centred, its odd spare column on the left; two values ending at column 14.
        let left = if names.len() == 1 {
            let spare = SIZE - wide;
            spare - spare / 2
        } else {
            SIZE - wide
        };
        let mut g = BLANK;
        for (out, rows) in g.iter_mut().zip(&band) {
            for (c, &on) in rows.iter().enumerate() {
                if on {
                    out[left + c] = Pixel::On;
                }
            }
        }
        return Picture::Static(g);
    }
    // The strip, then at least 4 blank columns, at least 21 columns in all.
    let length = 21.max(wide + 4);
    let rows = band
        .into_iter()
        .map(|mut r| {
            r.resize(length, false);
            r
        })
        .collect();
    Picture::Scrolling(Cycle { rows })
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
