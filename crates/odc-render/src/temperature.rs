//! Temperature, modes 7–9 (`docs/spec/display.md`, Modes): the CPU's, the GPU's, or the CPU's over
//! the GPU's, each followed by `°C` or `°F` in the saved unit.

use odc_metrics::Exact;

use crate::glyph::{CELSIUS, FAHRENHEIT, Glyph};
use crate::mode::{Mode, mode_info};
use crate::picture::{Band, Picture, layout};
use crate::value::{Shown, fahrenheit, number};
use crate::{Unit, Values};

/// The picture of a Temperature mode, in `unit`: each temperature the mode
/// shows, top first, from its own reading only (AGENTS.md rule 11).
pub(crate) fn picture(mode: Mode, values: &Values, unit: Unit) -> Picture {
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
}
