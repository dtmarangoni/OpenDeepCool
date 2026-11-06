//! Layer 1, the scenarios (Phase 9 plan, Tasks 6–9): every scenario of each drawn Music mode's
//! `docs/spec/frames/scenarios/mode-NNN.txt`, its sound analysed tick by tick, its Music settings
//! and resets applied, the spectrum, the emoji loop or the Dancer drawn with the renderer's own
//! pieces, in all four rotations, the expected frames turned. The service's engine replays the
//! same scenarios in `odc-service`'s `engine.rs`.

mod common;

use odc_render::Kind;

#[test]
fn every_scenario_of_each_drawn_music_mode_matches_in_every_rotation() {
    common::every_scenario_of_each_drawn_mode(Kind::Music);
}
