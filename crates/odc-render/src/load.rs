//! Load, modes 1–6 (`docs/spec/display.md`, Modes): usage and Memory Load, each followed by `%`.
//! Each mode joins `DRAWN` in its own commit, with all its tests (Phase 7 plan, Task 3;
//! AGENTS.md rule 4).

use crate::Values;
use crate::glyph::PERCENT;
use crate::mode::{Mode, mode_info};
use crate::picture::{Band, Picture, layout};
use crate::value::number;

/// The Load modes this build draws.
const DRAWN: [u16; 0] = [];

pub(crate) fn draws(mode: Mode) -> bool {
    DRAWN.contains(&mode.number())
}

/// The picture of a Load mode this build draws: each metric the mode shows, top first, from its
/// own reading only (AGENTS.md rule 11).
pub(crate) fn picture(mode: Mode, values: &Values) -> Option<Picture> {
    draws(mode).then(|| {
        let bands: Vec<Band> = mode_info(mode)
            .metrics
            .iter()
            .map(|&metric| Band {
                shown: number(values.get(metric)),
                symbol: &PERCENT,
            })
            .collect();
        layout(&bands)
    })
}
