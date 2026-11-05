//! Layer 1, the scenarios (Phase 8 plan, Tasks 7–8): every scenario of each drawn Power mode's
//! `docs/spec/frames/scenarios/mode-NNN.txt`, driven through `picture` and the `Marquee` in all
//! four rotations, the expected frames turned: every tick's frame must match, a unit change
//! leaving every frame as it was (S15).

mod common;

use odc_render::Kind;

#[test]
fn every_scenario_of_each_drawn_power_mode_matches_in_every_rotation() {
    common::every_scenario_of_each_drawn_mode(Kind::Power);
}
