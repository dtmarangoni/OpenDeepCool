//! The Music frame's parts before any Music mode is drawn (Phase 9 plan, Task 2): the value area
//! of modes 18–22 in rows 2–6 over a silent spectrum, against every case of their expected
//! files. Each mode's own matrix comes with it (Tasks 6–9).

mod common;

use common::{unit, values};
use odc_render::{
    COLUMNS, Column, Kind, Marquee, Mode, Unit, draws, mode_info, value_picture, with_spectrum,
};
use odc_testkit::diff;
use odc_testkit::expected::{self, load_mode};

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
    let music_drawn: [u16; 2] = [13, 14];
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
