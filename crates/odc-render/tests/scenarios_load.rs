//! Layer 1, the scenarios (Phase 7 plan, Task 3): every scenario of each drawn Load mode's
//! `docs/spec/frames/scenarios/mode-NNN.txt`, driven through `picture` and the `Marquee` in all
//! four rotations, the expected frames turned: every tick's frame must match. On one tick the
//! readings change first, then a reset applies (frames README). The service's engine replays the
//! same scenarios with its threads in Task 6.

mod common;

use odc_render::Kind;

#[test]
fn every_scenario_of_each_drawn_load_mode_matches_in_every_rotation() {
    common::every_scenario_of_each_drawn_mode(Kind::Load);
}
