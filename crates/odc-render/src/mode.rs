//! The modes of `docs/spec/display.md` (Modes, 1–12) and `docs/spec/music.md` (Modes, 13–23):
//! the metrics each shows, top first, and its kind. Availability and the later phases'
//! renderers read this table.

use odc_sensors::Metric;

/// A mode, by the spec's number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mode(u16);

impl Mode {
    /// The highest mode: Dancer, OpenDeepCool's own (`music.md`).
    pub const LAST: u16 = 23;

    /// Mode `number`, 1 to 23.
    pub fn new(number: u16) -> Option<Mode> {
        (1..=Mode::LAST).contains(&number).then_some(Mode(number))
    }

    /// Every mode, in order.
    pub fn all() -> impl Iterator<Item = Mode> {
        (1..=Mode::LAST).map(Mode)
    }

    pub fn number(self) -> u16 {
        self.0
    }
}

/// What a mode shows (owner, 2026-10-24: Load, Temperature, Power and Music, never "Guardian").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Load,
    Temperature,
    Power,
    Music,
}

/// A mode's metrics, top first, and its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeInfo {
    pub metrics: &'static [Metric],
    pub kind: Kind,
}

/// The table, for every mode.
pub fn mode_info(mode: Mode) -> ModeInfo {
    use Metric::{
        CpuPower, CpuTemperature, CpuUsage, GpuPower, GpuTemperature, GpuUsage, MemoryLoad,
    };
    let (metrics, kind): (&'static [Metric], Kind) = match mode.0 {
        1 => (&[CpuUsage], Kind::Load),
        2 => (&[GpuUsage], Kind::Load),
        3 => (&[MemoryLoad], Kind::Load),
        4 => (&[CpuUsage, GpuUsage], Kind::Load),
        5 => (&[CpuUsage, MemoryLoad], Kind::Load),
        6 => (&[GpuUsage, MemoryLoad], Kind::Load),
        7 => (&[CpuTemperature], Kind::Temperature),
        8 => (&[GpuTemperature], Kind::Temperature),
        9 => (&[CpuTemperature, GpuTemperature], Kind::Temperature),
        10 => (&[CpuPower], Kind::Power),
        11 => (&[GpuPower], Kind::Power),
        12 => (&[CpuPower, GpuPower], Kind::Power),
        // Full screen, the four emoji modes, and Dancer show no value.
        13..=17 | 23 => (&[], Kind::Music),
        18 => (&[CpuTemperature], Kind::Music),
        19 => (&[GpuTemperature], Kind::Music),
        20 => (&[CpuUsage], Kind::Music),
        21 => (&[GpuUsage], Kind::Music),
        22 => (&[MemoryLoad], Kind::Music),
        n => unreachable!("mode {n}: a Mode is 1 to 23"),
    };
    ModeInfo { metrics, kind }
}
