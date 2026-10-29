//! How a reading becomes the number drawn (`docs/spec/display.md`, Values: OpenDeepCool's
//! rules). Every measurement rounds to the nearest whole number, halves up, in integer
//! arithmetic on the exact reading, never through floating point (owner, 2026-10-26); every
//! number is capped at 9999 (owner, 2026-10-25); a negative reading draws dashes, like one that
//! can't be read (owner, 2026-10-24). Phase 8 adds °F, converted from the exact °C first.

use odc_sensors::Exact;

/// The largest number drawn, everywhere in OpenDeepCool.
pub const CAP: u16 = 9999;

/// What a value shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Number(u16),
    /// The reading can't be read, or no sensor can give it.
    Dashes,
}

/// The number drawn for a measurement in its own unit: usage and Memory Load in %, so far.
/// `None` is a reading that can't be read.
pub fn number(reading: Option<Exact>) -> Shown {
    let Some(reading) = reading else {
        return Shown::Dashes;
    };
    // An i64 numerator and a u64 denominator never overflow these i128 sums (solution review
    // OV-08).
    let (num, den) = (i128::from(reading.num()), i128::from(reading.den()));
    if num < 0 {
        return Shown::Dashes;
    }
    // floor(num / den + 1/2), with num ≥ 0 and den > 0.
    let nearest = (2 * num + den) / (2 * den);
    Shown::Number(u16::try_from(nearest.min(i128::from(CAP))).expect("at most 9999"))
}
