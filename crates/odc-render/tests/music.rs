//! The Music frame's parts before any Music mode is drawn (Phase 9 plan, Task 2): the value area
//! of modes 18–22 in rows 2–6 over a silent spectrum, against every case of their expected
//! files, and with any spectrum under any value (Task 8). Each mode's own matrix comes with it
//! (Tasks 6–9).

mod common;

use common::{unit, values};
use odc_protocol::Frame;
use odc_render::{
    COLUMNS, Column, Kind, Marquee, Mode, Unit, draws, mode_info, value_picture, with_spectrum,
};
use odc_sensors::Exact;
use odc_testkit::diff;
use odc_testkit::expected::{self, load_mode};
use proptest::prelude::*;

/// Silence: every bar 1 row high, the floor.
const FLOOR: [Column; COLUMNS] = [Column { bar: 1, dot: None }; COLUMNS];

#[test]
fn the_value_area_of_modes_18_to_22_draws_every_case_over_a_silent_spectrum() {
    for number in 18..=22 {
        let mode = Mode::new(number).expect("a mode");
        let file = load_mode(number);
        let name = format!("mode-{number:03}.txt");
        for case in &file.cases {
            let shown = case.unit.map_or(Unit::Celsius, unit);
            let picture = value_picture(mode, &values(&case.readings), shown).expect("a value");
            let mut marquee = Marquee::new(picture);
            let pictures = case.kind.pictures();
            assert!(
                !matches!(case.kind, expected::Kind::Loop(_)),
                "{name}, {}: a value is still or scrolls",
                case.heading
            );
            // The whole cycle from the reset, then its first frame again.
            for (k, expected) in pictures.iter().chain(pictures.first()).enumerate() {
                let frame = with_spectrum(&marquee.tick(), &FLOOR, 6).rotated(case.rotation);
                let context = format!("{name}, {}, frame {k}", case.heading);
                diff::assert_matches(expected, &frame, &context);
            }
        }
    }
}

#[test]
fn only_modes_18_to_22_have_a_value_area_and_the_music_modes_drawn_so_far_are_drawn() {
    // Phase 9 plan, Tasks 6–9: each Music mode is drawn in its own commit.
    let music_drawn: [u16; 11] = [13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23];
    for mode in Mode::all() {
        let area = value_picture(mode, &values(&[]), Unit::Celsius);
        assert_eq!(
            area.is_some(),
            (18..=22).contains(&mode.number()),
            "mode {}",
            mode.number()
        );
        if mode_info(mode).kind == Kind::Music {
            assert_eq!(
                draws(mode),
                music_drawn.contains(&mode.number()),
                "mode {}",
                mode.number()
            );
        }
    }
}

/// Any column of a spectrum area 6 rows high: a bar of 1 to 6 rows, and a dot above it or none.
fn column() -> impl Strategy<Value = Column> {
    (1u8..=6, 0u8..=6).prop_map(|(bar, dot)| Column {
        bar,
        dot: (dot > bar).then_some(dot),
    })
}

/// Any reading: none, negative, past the cap, fractional.
fn reading() -> impl Strategy<Value = Option<Exact>> {
    prop_oneof![
        Just(None),
        (-1_000i64..=2_000_000, 1u64..=4).prop_map(|(num, den)| Exact::new(num, den)),
    ]
}

proptest! {
    // Phase 9 plan, Task 8 (`music.md`, Value area): whatever the value area does, still,
    // scrolling or dashes, in either unit and at any point of its cycle, rows 9–14 are exactly the
    // spectrum's rows and rows 1–8 the value's: only the value moves.
    #[test]
    fn rows_9_to_14_are_always_the_spectrum_s_whatever_the_value_does(
        number in 18u16..=22,
        value in reading(),
        fahrenheit in any::<bool>(),
        ticks in 0usize..60,
        columns in proptest::array::uniform14(column()),
    ) {
        let mode = Mode::new(number).expect("a mode");
        let [metric] = mode_info(mode).metrics else {
            unreachable!("modes 18–22 show one value each")
        };
        let shown = if fahrenheit { Unit::Fahrenheit } else { Unit::Celsius };
        let named = [(metric.spec_name().to_owned(), value)];
        let picture = value_picture(mode, &values(&named), shown).expect("a value");
        let mut marquee = Marquee::new(picture);
        for _ in 0..ticks {
            marquee.tick();
        }
        let top = marquee.tick();
        let frame = with_spectrum(&top, &columns, 6);
        let alone = with_spectrum(&Frame::blank(), &columns, 6);
        for c in 1..=14 {
            for r in 1..=8 {
                prop_assert_eq!(frame.get(r, c), top.get(r, c), "row {}, column {}", r, c);
            }
            for r in 9..=14 {
                prop_assert!(!top.get(r, c), "the value reaches row {}, column {}", r, c);
                prop_assert_eq!(frame.get(r, c), alone.get(r, c), "row {}, column {}", r, c);
            }
        }
    }
}
