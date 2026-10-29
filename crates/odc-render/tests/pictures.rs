//! One or two values as a strip: its width *w*, cycle *L*, home column *h* and whether it must
//! scroll, for every value length from dashes to four digits (Phase 7 plan, Task 2;
//! `docs/spec/display.md`, Static layout and Movement, precisely).

use odc_render::{Band, PERCENT, Picture, Shown, layout};

/// One value of each length: dashes, then 1 to 4 digits.
const LENGTHS: [(Shown, usize); 5] = [
    (Shown::Dashes, 2),
    (Shown::Number(7), 1),
    (Shown::Number(42), 2),
    (Shown::Number(100), 3),
    (Shown::Number(1000), 4),
];

fn band(shown: Shown) -> Band {
    Band {
        shown,
        symbol: &PERCENT,
    }
}

fn scrolls(shown: Shown) -> bool {
    matches!(shown, Shown::Number(n) if n >= 100)
}

#[test]
fn one_value_of_every_length() {
    // (w, L, h): digit places of 4 columns and the 5-wide `%`; one value centred, the odd spare
    // column on the left; home at column 2 past 14 columns.
    let want = [
        (13, 21, 2),
        (9, 21, 4),
        (13, 21, 2),
        (17, 21, 2),
        (21, 25, 2),
    ];
    for ((shown, places), (w, cycle, home)) in LENGTHS.into_iter().zip(want) {
        let p = layout(&[band(shown)]);
        assert_eq!(
            p.width(),
            i32::try_from(4 * places + 5).unwrap(),
            "{shown:?}"
        );
        assert_eq!(
            (p.width(), p.cycle(), p.home(), p.must_scroll()),
            (w, cycle, home, scrolls(shown)),
            "{shown:?}"
        );
    }
}

#[test]
fn two_values_of_every_length_end_at_the_same_column() {
    for (top, top_places) in LENGTHS {
        for (bottom, bottom_places) in LENGTHS {
            let p = layout(&[band(top), band(bottom)]);
            let w = i32::try_from(4 * top_places.max(bottom_places) + 5).unwrap();
            let home = if w > 14 { 2 } else { 15 - w };
            let cycle = w + 4.max(21 - w);
            assert_eq!(
                (p.width(), p.cycle(), p.home(), p.must_scroll()),
                (w, cycle, home, scrolls(top) || scrolls(bottom)),
                "{top:?} over {bottom:?}"
            );
        }
    }
}

/// The columns of `picture` at home that hold a lit pixel, from 1.
fn lit_columns(picture: &Picture) -> Vec<usize> {
    let frame = picture.frame_at(picture.home());
    (1..=14)
        .filter(|&c| (1..=14).any(|r| frame.get(r, c)))
        .collect()
}

// display.md, OpenDeepCool: alignment: `7%` takes columns 4–12; `42%` ends at column 14 with one
// spare column on the left.
#[test]
fn a_static_value_sits_where_the_alignment_rule_puts_it() {
    let seven = layout(&[band(Shown::Number(7))]);
    let columns = lit_columns(&seven);
    assert_eq!((columns[0], *columns.last().unwrap()), (4, 12));
    let forty_two = layout(&[band(Shown::Number(42))]);
    let columns = lit_columns(&forty_two);
    assert_eq!((columns[0], *columns.last().unwrap()), (2, 14));
}

// display.md, Bands: one value in rows 6–10; two in rows 2–6 and 9–13, the shorter one aligned
// to the longer one's right end.
#[test]
fn values_are_drawn_in_their_bands() {
    let one = layout(&[band(Shown::Number(42))]).frame_at(2);
    let rows: Vec<usize> = (1..=14)
        .filter(|&r| (1..=14).any(|c| one.get(r, c)))
        .collect();
    assert_eq!(rows, [6, 7, 8, 9, 10]);
    let two = layout(&[band(Shown::Number(7)), band(Shown::Number(42))]);
    let frame = two.frame_at(two.home());
    let rows: Vec<usize> = (1..=14)
        .filter(|&r| (1..=14).any(|c| frame.get(r, c)))
        .collect();
    assert_eq!(rows, [2, 3, 4, 5, 6, 9, 10, 11, 12, 13]);
    // The `%` of both values ends at column 14: `7` starts at column 6, `42` at column 2.
    for row in [2, 6, 9, 13] {
        assert!(frame.get(row, 14), "row {row}");
    }
    assert!(
        frame.get(2, 6) && !frame.get(2, 5),
        "the 7's top row starts at column 6"
    );
    assert!(frame.get(10, 2), "the 4 starts at column 2");
}

// display.md, Unavailable values: a dash in each of two digit places, columns 2–4 and 6–8, and
// the symbol kept: `--%`.
#[test]
fn dashes_take_two_digit_places_and_keep_the_symbol() {
    let frame = layout(&[band(Shown::Dashes)]).frame_at(2);
    let middle: Vec<usize> = (1..=14).filter(|&c| frame.get(8, c)).collect();
    assert_eq!(middle, [2, 3, 4, 6, 7, 8, 12]);
    for row in [6, 7, 9, 10] {
        assert!(
            (2..=8).all(|c| !frame.get(row, c)),
            "row {row}: only the dashes' middle row"
        );
    }
}

// A strip wider than the screen: its copies repeat every L columns, the gap blank.
#[test]
fn copies_repeat_every_cycle() {
    let p = layout(&[band(Shown::Number(100))]);
    assert_eq!(p.frame_at(2), p.frame_at(2 + p.cycle()));
    assert_eq!(p.frame_at(2), p.frame_at(2 - 3 * p.cycle()));
}
