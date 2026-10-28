//! Rotation against the oracle (Phase 5 plan, Task 3): mode 1's upright static pictures, `3%` and
//! `42%`, turned by `Frame::rotated`, match the expected frames of rotations 1–3. These 8 cases
//! need no renderer: every picture comes from `docs/spec/frames/mode-001.txt` itself.

use odc_protocol::Rotation;
use odc_testkit::diff;
use odc_testkit::expected::{Case, Kind, Picture, load_mode};

fn static_picture(case: &Case) -> &Picture {
    match &case.kind {
        Kind::Static(picture) => picture,
        _ => panic!("{}: not a static case", case.heading),
    }
}

#[test]
fn turning_mode_1s_upright_pictures_gives_its_rotated_cases() {
    let mode = load_mode(1);
    let statics: Vec<&Case> = mode
        .cases
        .iter()
        .filter(|c| matches!(c.kind, Kind::Static(_)))
        .collect();
    assert_eq!(statics.len(), 8, "two value sets in four rotations");
    let upright: Vec<&Case> = statics
        .iter()
        .copied()
        .filter(|c| c.rotation == Rotation::default())
        .collect();
    assert_eq!(upright.len(), 2);
    for base in upright {
        let frame = static_picture(base)
            .to_frame()
            .expect("mode 1 has no spectrum pixels");
        for rotation in &Rotation::ALL[1..] {
            let case = statics
                .iter()
                .find(|c| c.rotation == *rotation && c.readings == base.readings)
                .unwrap_or_else(|| {
                    panic!("no rotation {} for {}", rotation.setting(), base.heading)
                });
            diff::assert_matches(
                static_picture(case),
                &frame.rotated(*rotation),
                &case.heading,
            );
        }
    }
}
