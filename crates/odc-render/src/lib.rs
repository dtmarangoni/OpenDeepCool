//! What the LP360's display shows, as pure code (Phase 7 plan, Task 2): the glyphs, the value
//! rules, one or two values as a strip, the movement, and the mode table, all from
//! `docs/spec/display.md` and `docs/spec/music.md`. No clock and no I/O: the service's engine
//! turns each frame last, and the settings app (Phase 10) draws its tiles with the same code.

mod glyph;
mod load;
mod marquee;
mod mode;
mod picture;
mod power;
mod temperature;
mod value;

pub use glyph::{CELSIUS, DASH, DIGITS, FAHRENHEIT, Glyph, PERCENT, WATT};
pub use marquee::Marquee;
pub use mode::{Kind, Mode, ModeInfo, mode_info};
pub use picture::{Band, Picture, layout};
pub use value::{CAP, Shown, fahrenheit, number};

use odc_sensors::{Exact, Metric, Reading, Snapshot};

/// The temperature unit, for the modes that show a temperature (`docs/spec/settings.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Unit {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Every metric's value for one picture: its exact reading, or `None` when it can't be read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Values {
    readings: [Option<Exact>; Metric::ALL.len()],
}

impl Values {
    /// Every metric without a value.
    pub fn new() -> Values {
        Values::default()
    }

    /// The same values, with `metric` at `value`.
    pub fn with(mut self, metric: Metric, value: Exact) -> Values {
        self.set(metric, Some(value));
        self
    }

    pub fn set(&mut self, metric: Metric, value: Option<Exact>) {
        self.readings[index(metric)] = value;
    }

    pub fn get(&self, metric: Metric) -> Option<Exact> {
        self.readings[index(metric)]
    }

    /// A snapshot's values: each reading's value, `None` where it has a reason instead.
    pub fn of(snapshot: &Snapshot) -> Values {
        let mut values = Values::new();
        for metric in Metric::ALL {
            if let Reading::Value(value) = snapshot.get(metric) {
                values.set(metric, Some(*value));
            }
        }
        values
    }
}

fn index(metric: Metric) -> usize {
    Metric::ALL
        .iter()
        .position(|&m| m == metric)
        .expect("Metric::ALL holds every metric")
}

/// A mode this build doesn't draw yet: each Temperature and Power mode comes in its own commit of
/// Phase 8, Music in Phase 9.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotYetImplemented(pub Mode);

/// Whether [`picture`] draws `mode`. It's the one source for both the service's refusal and its
/// `AvailableModes` (owner, 2026-10-27; solution review P8-03).
pub fn draws(mode: Mode) -> bool {
    load::draws(mode) || temperature::draws(mode) || power::draws(mode)
}

/// The picture of `mode` for `values`: its strip, its home and whether it must scroll. The
/// movement ([`Marquee`]) and the rotation (`Frame::rotated`) come after. Only the temperature
/// modes read `unit`.
pub fn picture(mode: Mode, values: &Values, unit: Unit) -> Result<Picture, NotYetImplemented> {
    match mode_info(mode).kind {
        Kind::Load => load::picture(mode, values),
        Kind::Temperature => temperature::picture(mode, values, unit),
        Kind::Power => power::picture(mode, values),
        Kind::Music => None,
    }
    .ok_or(NotYetImplemented(mode))
}
