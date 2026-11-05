//! Power, modes 10–12 (`docs/spec/display.md`, Modes): the CPU's, the GPU's, or the CPU's over the
//! GPU's, each followed by `W`. Each mode joins `DRAWN` in its own commit, with all its tests
//! (Phase 8 plan, Tasks 7–8; AGENTS.md rule 4).

use crate::Values;
use crate::glyph::WATT;
use crate::mode::{Mode, mode_info};
use crate::picture::{Band, Picture, layout};
use crate::value::number;

/// The Power modes this build draws.
const DRAWN: [u16; 1] = [10];

pub(crate) fn draws(mode: Mode) -> bool {
    DRAWN.contains(&mode.number())
}

/// The picture of a Power mode this build draws: each power the mode shows, top first, from its
/// own reading only (AGENTS.md rule 11).
pub(crate) fn picture(mode: Mode, values: &Values) -> Option<Picture> {
    draws(mode).then(|| {
        let bands: Vec<Band> = mode_info(mode)
            .metrics
            .iter()
            .map(|&metric| Band {
                shown: number(values.get(metric)),
                symbol: &WATT,
            })
            .collect();
        layout(&bands)
    })
}
