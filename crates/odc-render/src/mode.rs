//! The modes of `docs/spec/display.md` (Modes, 1–12) and `docs/spec/music.md` (Modes, 13–16 and
//! 18–26): the metrics each shows, top first, and its kind. Availability and the renderers read this
//! table.

use odc_sensors::Metric;

/// A mode, by the spec's number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mode(u16);

impl Mode {
    /// The highest mode: the third emoji alone, OpenDeepCool's own (`music.md`).
    pub const LAST: u16 = 26;

    /// The number no mode has: DeepCreative's Emoji 4, which OpenDeepCool dropped when it drew
    /// its own emoji (owner, 2026-11-23). Its number stays unused, so that no mode changes its
    /// own.
    pub const RETIRED: u16 = 17;

    /// Mode `number`, 1 to 26 but 17.
    pub fn new(number: u16) -> Option<Mode> {
        ((1..=Mode::LAST).contains(&number) && number != Mode::RETIRED).then_some(Mode(number))
    }

    /// Every mode, in order.
    pub fn all() -> impl Iterator<Item = Mode> {
        (1..=Mode::LAST)
            .filter(|&number| number != Mode::RETIRED)
            .map(Mode)
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

/// What a mode draws above or instead of its spectrum (`docs/spec/music.md`): the one decision
/// the cooler's engine and the app's tiles both follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content {
    /// A value's strip: the whole grid in modes 1–12, rows 2–6 in modes 18–22.
    Value,
    /// Mode 13: the spectrum alone.
    Spectrum,
    /// An emoji's loop: in rows 1–7 over the spectrum (modes 14–16), or on the whole grid
    /// (modes 24–26).
    Emoji,
    /// Mode 23: the Dancer.
    Dancer,
}

/// A mode's metrics, top first, its kind and its content. Every Music mode takes sound (owner,
/// 2026-10-26; the emoji alone too, since they move only while sound plays, owner, 2026-11-23).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeInfo {
    pub metrics: &'static [Metric],
    pub kind: Kind,
    pub content: Content,
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
        // Full screen, the emoji modes with and without a spectrum, and Dancer show no value.
        13..=16 | 23..=26 => (&[], Kind::Music),
        18 => (&[CpuTemperature], Kind::Music),
        19 => (&[GpuTemperature], Kind::Music),
        20 => (&[CpuUsage], Kind::Music),
        21 => (&[GpuUsage], Kind::Music),
        22 => (&[MemoryLoad], Kind::Music),
        n => unreachable!("mode {n}: a Mode is 1 to 26 but 17"),
    };
    let content = match mode.0 {
        13 => Content::Spectrum,
        14..=16 | 24..=26 => Content::Emoji,
        23 => Content::Dancer,
        _ => Content::Value,
    };
    ModeInfo {
        metrics,
        kind,
        content,
    }
}
