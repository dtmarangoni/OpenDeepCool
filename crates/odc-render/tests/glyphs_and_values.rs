//! The glyphs and the value rules (Phase 7 plan, Task 2; Phase 8 plan, Task 2;
//! `docs/spec/display.md`, Glyphs, Values and Unavailable values).

use odc_render::{
    CAP, CELSIUS, DASH, DIGITS, FAHRENHEIT, Glyph, PERCENT, Shown, WATT, fahrenheit, number,
};
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

/// The symbols as `display.md` prints them, side by side: `%`, `°C`, `°F` and `W`, typed here
/// apart from the crate's own.
const SPEC_SYMBOLS: [&str; 5] = [
    "##..#    #...    #.###    .....",
    "##.#.    ..##    ..#..    #.#.#",
    "..#..    .#..    ..###    #.#.#",
    ".#.##    .#..    ..#..    #.#.#",
    "#..##    ..##    ..#..    .#.#.",
];

#[test]
fn each_symbol_is_the_spec_picture() {
    let symbols: [(&str, &Glyph); 4] = [
        ("%", &PERCENT),
        ("°C", &CELSIUS),
        ("°F", &FAHRENHEIT),
        ("W", &WATT),
    ];
    for (i, (name, glyph)) in symbols.into_iter().enumerate() {
        for (r, row) in glyph.iter().enumerate() {
            let spec = SPEC_SYMBOLS[r]
                .split_whitespace()
                .nth(i)
                .expect("four symbols");
            assert_eq!(*row, spec, "{name}, row {}", r + 1);
        }
    }
    // display.md, Glyphs: `°C` 4 wide, `°F` and `W` 5 wide; `W` on the lower four rows.
    assert_eq!(
        (CELSIUS[0].len(), FAHRENHEIT[0].len(), WATT[0].len()),
        (4, 5, 5)
    );
    assert_eq!(WATT[0], ".....");
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

/// The number drawn in °F for `num / den` °C.
fn f(num: i64, den: u64) -> Shown {
    fahrenheit(exact(num, den))
}

// display.md, Values (owner, 2026-10-26): °C to the nearest; °F rounded once from the exact
// reading, so 37 °C (98.6) draws 99 and 57 °C (134.6) draws 135.
#[test]
fn temperatures_round_to_the_nearest_in_each_unit() {
    let celsius = [
        ((372, 10), 37),
        ((375, 10), 38),
        ((574, 10), 57),
        ((100, 1), 100),
    ];
    for ((num, den), want) in celsius {
        assert_eq!(n(num, den), Shown::Number(want), "{num}/{den} °C");
    }
    let fahrenheit = [
        ((0, 1), 32),
        ((372, 10), 99),
        ((57, 1), 135),
        ((100, 1), 212),
        ((120, 1), 248),
        ((537, 1), 999),
    ];
    for ((num, den), want) in fahrenheit {
        assert_eq!(f(num, den), Shown::Number(want), "{num}/{den} °C in °F");
    }
    // The readings are exact millidegrees: 37 000 m°C is exactly 37 °C, 98.6 °F.
    assert_eq!(f(37_000, 1000), Shown::Number(99));
}

// display.md, Values: no caps below 9999 (owner, 2026-10-24), and 9999 above it in the unit drawn,
// for °F after the conversion (owner, 2026-10-25).
#[test]
fn temperatures_are_drawn_in_full_up_to_9999_and_capped_there() {
    assert_eq!(n(120, 1), Shown::Number(120));
    assert_eq!(n(1000, 1), Shown::Number(1000));
    assert_eq!(f(538, 1), Shown::Number(1000), "1000.4");
    assert_eq!(f(5536, 1), Shown::Number(9997), "9996.8");
    assert_eq!(f(5537, 1), Shown::Number(9999), "9998.6");
    assert_eq!(f(55_375, 10), Shown::Number(CAP), "10 000, capped");
    for (num, den) in [(9999, 1), (10_000, 1), (i64::MAX, 1000)] {
        assert_eq!(n(num, den), Shown::Number(CAP), "{num}/{den} °C");
        assert_eq!(f(num, den), Shown::Number(CAP), "{num}/{den} °C in °F");
    }
}

// display.md S7: the rounding at the limit in each unit.
#[test]
fn the_limit_of_s7_in_each_unit() {
    assert_eq!(f(3749, 100), Shown::Number(99), "99.482 °F");
    assert_eq!(f(375, 10), Shown::Number(100), "99.5 °F");
    assert_eq!(n(9949, 100), Shown::Number(99));
    assert_eq!(n(995, 10), Shown::Number(100));
}

// display.md S19: a negative °C reading draws dashes in °F too, before any conversion, though
// −1 m°C is 31.998 °F; so does one that can't be read.
#[test]
fn negative_and_missing_temperatures_draw_dashes_in_fahrenheit() {
    for (num, den) in [(-1, 1000), (-40, 1), (i64::MIN, 1)] {
        assert_eq!(f(num, den), Shown::Dashes, "{num}/{den}");
    }
    assert_eq!(fahrenheit(None), Shown::Dashes);
}

// display.md, Values: power to the nearest, from µW, mW or µJ over µs, in full up to 9999.
#[test]
fn power_rounds_to_the_nearest_and_is_capped_at_9999() {
    let rows = [
        ((572, 10), 57),
        ((575, 10), 58),
        ((9949, 100), 99),
        ((995, 10), 100),
        ((999, 1), 999),
        ((1000, 1), 1000),
        ((1234, 1), 1234),
        ((1999, 1), 1999),
        ((9999, 1), 9999),
        ((999_949, 100), 9999),
        ((99_995, 10), CAP),
        ((123_456, 1), CAP),
    ];
    for ((num, den), want) in rows {
        assert_eq!(n(num, den), Shown::Number(want), "{num}/{den} W");
    }
    // An energy counter: 65 000 000 µJ over 1 000 000 µs is exactly 65 W.
    assert_eq!(n(65_000_000, 1_000_000), Shown::Number(65));
    // amdgpu's µW: 24.5 W rounds up.
    assert_eq!(n(24_500_000, 1_000_000), Shown::Number(25));
}

// Phase 8 plan, Task 2: for every millidegree reading m from 0 to 9 999 000, °F is the nearest
// whole number to 9/5 · m/1000 + 32, (9m + 162 500) div 5000, capped at 9999.
#[test]
fn every_millidegree_reading_draws_the_nearest_fahrenheit() {
    for m in 0..=9_999_000_i64 {
        let want = ((9 * m + 162_500) / 5000).min(i64::from(CAP));
        assert_eq!(
            f(m, 1000),
            Shown::Number(u16::try_from(want).expect("at most 9999")),
            "{m} m°C"
        );
    }
}

proptest! {
    #[test]
    fn any_temperature_of_0_or_more_draws_a_fahrenheit_number_up_to_9999(
        num in 0..=i64::MAX,
        den in 1..=u64::MAX,
    ) {
        match f(num, den) {
            Shown::Number(drawn) => prop_assert!((32..=CAP).contains(&drawn)),
            Shown::Dashes => prop_assert!(false, "{num}/{den} drew dashes"),
        }
    }

    #[test]
    fn any_negative_temperature_draws_dashes_in_fahrenheit(num in i64::MIN..0, den in 1..=u64::MAX) {
        prop_assert_eq!(f(num, den), Shown::Dashes);
    }
}
