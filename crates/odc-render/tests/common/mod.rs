//! What the Load tests share: the modes drawn so far, and a case's readings as `Values`. Each
//! test file uses part of it.
#![allow(dead_code)]

use odc_protocol::{Frame, Rotation};
use odc_render::{Kind, Mode, Values, draws, mode_info};
use odc_sensors::Exact;
use odc_testkit::expected::{Picture, every_metric};

/// The Load modes this build draws (Phase 7 plan, Task 3: one per commit).
pub fn drawn_load_modes() -> Vec<Mode> {
    Mode::all()
        .filter(|&mode| mode_info(mode).kind == Kind::Load && draws(mode))
        .collect()
}

/// Every metric's value: the readings named, and a filler of its own for every other metric, so
/// drawing the wrong metric shows (AGENTS.md rule 5).
pub fn values(readings: &[(String, Option<Exact>)]) -> Values {
    let mut values = Values::new();
    for (metric, value) in every_metric(readings) {
        values.set(metric, value);
    }
    values
}

/// An expected picture of the Load files, which hold no spectrum pixel, turned.
pub fn turned(picture: &Picture, rotation: Rotation) -> Frame {
    picture
        .to_frame()
        .expect("Load pictures have no spectrum pixel")
        .rotated(rotation)
}
