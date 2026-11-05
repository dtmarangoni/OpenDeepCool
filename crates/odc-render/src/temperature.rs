//! Temperature, modes 7–9 (`docs/spec/display.md`, Modes): the CPU's, the GPU's, or the CPU's over
//! the GPU's, each followed by `°C` or `°F` in the saved unit. Each mode joins `DRAWN` in its own
//! commit, with all its tests (Phase 8 plan, Tasks 4–6; AGENTS.md rule 4).

use odc_sensors::Exact;

use crate::glyph::{CELSIUS, FAHRENHEIT, Glyph};
use crate::mode::{Mode, mode_info};
use crate::picture::{Band, Picture, layout};
use crate::value::{Shown, fahrenheit, number};
use crate::{Unit, Values};

/// The Temperature modes this build draws.
const DRAWN: [u16; 1] = [7];

pub(crate) fn draws(mode: Mode) -> bool {
    DRAWN.contains(&mode.number())
}

/// The picture of a Temperature mode this build draws, in `unit`: each temperature the mode
/// shows, top first, from its own reading only (AGENTS.md rule 11).
pub(crate) fn picture(mode: Mode, values: &Values, unit: Unit) -> Option<Picture> {
    draws(mode).then(|| {
        let (rule, symbol): (fn(Option<Exact>) -> Shown, &'static Glyph) = match unit {
            Unit::Celsius => (number, &CELSIUS),
            Unit::Fahrenheit => (fahrenheit, &FAHRENHEIT),
        };
        let bands: Vec<Band> = mode_info(mode)
            .metrics
            .iter()
            .map(|&metric| Band {
                shown: rule(values.get(metric)),
                symbol,
            })
            .collect();
        layout(&bands)
    })
}
