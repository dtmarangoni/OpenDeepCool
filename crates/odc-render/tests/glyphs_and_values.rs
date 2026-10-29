//! The glyphs and the value rules (Phase 7 plan, Task 2; `docs/spec/display.md`, Glyphs, Values
//! and Unavailable values).

use odc_render::{CAP, DASH, DIGITS, PERCENT, Shown, number};
use odc_sensors::Exact;
use proptest::prelude::*;

/// The digits as `display.md` prints them, side by side, typed here apart from the crate's own
/// table so that a copying slip in either shows up.
const SPEC_DIGITS: [&str; 5] = [
    "###  .#.  ###  ###  #.#  ###  ###  ###  ###  ###",
    "#.#  ##.  ..#  ..#  #.#  #..  #..  ..#  #.#  #.#",
    "#.#  .#.  ###  ###  ###  ###  ###  .#.  ###  ###",
    "#.#  .#.  #..  ..#  ..#  ..#  #.#  .#.  #.#  ..#",
    "###  ###  ###  ###  ..#  ###  ###  .#.  ###  ###",
];

#[test]
fn each_digit_is_the_spec_picture() {
    for (d, digit) in DIGITS.iter().enumerate() {
        for (r, row) in digit.iter().enumerate() {
            let spec = SPEC_DIGITS[r].split("  ").nth(d).expect("ten digits");
            assert_eq!(*row, spec, "digit {d}, row {}", r + 1);
        }
    }
}

#[test]
fn percent_and_the_dash_are_the_spec_pictures() {
    let percent = ["##..#", "##.#.", "..#..", ".#.##", "#..##"];
    assert_eq!(PERCENT, percent);
    // display.md, Unavailable values: 3 × 5, the middle row lit.
    assert_eq!(DASH, ["...", "...", "###", "...", "..."]);
}

fn exact(num: i64, den: u64) -> Option<Exact> {
    Some(Exact::new(num, den).expect("a positive denominator"))
}

fn n(num: i64, den: u64) -> Shown {
    number(exact(num, den))
}

#[test]
fn every_measurement_rounds_to_the_nearest_halves_up() {
    // Owner, 2026-10-26: no longer DeepCreative's rounding up for usage (0.4 → 1).
    let rows = [
        ((4, 10), 0),
        ((5, 10), 1),
        ((949, 100), 9),
        ((95, 10), 10),
        ((9949, 100), 99),
        ((995, 10), 100),
    ];
    for ((num, den), want) in rows {
        assert_eq!(n(num, den), Shown::Number(want), "{num}/{den}");
    }
}

#[test]
fn values_are_drawn_in_full_up_to_9999_and_capped_there() {
    for whole in [100, 1000, 9999] {
        assert_eq!(n(whole, 1), Shown::Number(u16::try_from(whole).unwrap()));
    }
    for past in [10_000, 123_456, i64::MAX] {
        assert_eq!(n(past, 1), Shown::Number(CAP), "{past}");
    }
}

// display.md S20: both sides of every length boundary.
#[test]
fn the_length_boundaries_of_s20() {
    let rows = [
        ((0, 1), 0),
        ((1, 1000), 0),
        ((49, 100), 0),
        ((5, 10), 1),
        ((9, 1), 9),
        ((10, 1), 10),
        ((99, 1), 99),
        ((100, 1), 100),
        ((99_949, 100), 999),
        ((9995, 10), 1000),
        ((999, 1), 999),
        ((1000, 1), 1000),
        ((999_949, 100), 9999),
        ((99_995, 10), 9999),
        ((10_000, 1), 9999),
    ];
    for ((num, den), want) in rows {
        assert_eq!(n(num, den), Shown::Number(want), "{num}/{den}");
    }
}

// display.md S19: a negative reading, which no sensor can give, draws dashes; so does one that
// can't be read.
#[test]
fn negative_and_missing_readings_draw_dashes() {
    for (num, den) in [(-1, 1), (-1, 1000), (-4, 10), (-5, 10), (i64::MIN, 1)] {
        assert_eq!(n(num, den), Shown::Dashes, "{num}/{den}");
    }
    assert_eq!(number(None), Shown::Dashes);
}

// display.md Values: 23 busy jiffies of 40 is exactly 57.5 %, which draws 58; floating point
// makes it 57.49999999999999, which would draw 57.
#[test]
fn raw_counters_round_exactly() {
    let busy_share = Exact::new(23 * 100, 40).expect("a share");
    assert_eq!(number(Some(busy_share)), Shown::Number(58));
    assert!(
        std::hint::black_box(23.0_f64) / 40.0 * 100.0 < 57.5,
        "floating point loses the half"
    );
    // Memory Load from kB: (MemTotal − MemAvailable) / MemTotal, 1 of 8 GiB used, 12.5 % → 13.
    let used = Exact::new(100 * 1_048_576, 8_388_608).expect("a share");
    assert_eq!(number(Some(used)), Shown::Number(13));
}

proptest! {
    #[test]
    fn any_reading_of_0_or_more_draws_a_number_up_to_9999(num in 0..=i64::MAX, den in 1..=u64::MAX) {
        match n(num, den) {
            Shown::Number(drawn) => prop_assert!(drawn <= CAP),
            Shown::Dashes => prop_assert!(false, "{num}/{den} drew dashes"),
        }
    }

    #[test]
    fn any_negative_reading_draws_dashes(num in i64::MIN..0, den in 1..=u64::MAX) {
        prop_assert_eq!(n(num, den), Shown::Dashes);
    }
}
