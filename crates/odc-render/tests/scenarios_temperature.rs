//! Layer 1, the scenarios (Phase 8 plan, Tasks 4–6): every scenario of each drawn Temperature
//! mode's `docs/spec/frames/scenarios/mode-NNN.txt`, in °C and in °F, driven through `picture` and
//! the `Marquee` in all four rotations, the expected frames turned: every tick's frame must match,
//! a unit change resetting to home in the new unit (S15).

mod common;

use odc_render::Kind;

#[test]
fn every_scenario_of_each_drawn_temperature_mode_matches_in_every_rotation() {
    common::every_scenario_of_each_drawn_mode(Kind::Temperature);
}
