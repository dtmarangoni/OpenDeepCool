//! Power, modes 10–12 (`docs/spec/display.md`, Modes): the CPU's, the GPU's, or the CPU's over the
//! GPU's, each followed by `W`.

use crate::Values;
use crate::glyph::WATT;
use crate::mode::{Mode, mode_info};
use crate::picture::{Band, Picture, layout};
use crate::value::number;

/// The picture of a Power mode: each power the mode shows, top first, from its
/// own reading only (AGENTS.md rule 11).
pub(crate) fn picture(mode: Mode, values: &Values) -> Picture {
    let bands: Vec<Band> = mode_info(mode)
        .metrics
        .iter()
        .map(|&metric| Band {
            shown: number(values.get(metric)),
            symbol: &WATT,
        })
        .collect();
    layout(&bands)
}
