//! How a reading becomes the number drawn (`docs/spec/display.md`, Values: OpenDeepCool's
//! rules). Every measurement rounds to the nearest whole number, halves up, in integer
//! arithmetic on the exact reading, never through floating point (owner, 2026-10-26); every
//! number is capped at 9999 (owner, 2026-10-25); a negative reading draws dashes, like one that
//! can't be read (owner, 2026-10-24). °F is rounded once, from the exact °C reading.

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

/// The number drawn for a measurement in its own unit: usage and Memory Load in %, a temperature
/// in °C, power in W. `None` is a reading that can't be read.
pub fn number(reading: Option<Exact>) -> Shown {
    let Some((num, den)) = fraction(reading) else {
        return Shown::Dashes;
    };
    // floor(num / den + 1/2), with num ≥ 0 and den > 0.
    capped((2 * num + den) / (2 * den))
}

/// The number drawn for a temperature in °F, from its exact reading in °C: 9/5 · °C + 32, rounded
/// once to the nearest, halves up (owner, 2026-10-26). A negative °C reading draws dashes, before
/// any conversion.
pub fn fahrenheit(reading: Option<Exact>) -> Shown {
    let Some((num, den)) = fraction(reading) else {
        return Shown::Dashes;
    };
    // floor((9 · num + 160 · den) / (5 · den) + 1/2), the same as (9m + 162 500) div 5000 on
    // millidegrees m (display.md, Values).
    capped((18 * num + 325 * den) / (10 * den))
}

/// The reading as a fraction of whole numbers, or `None` for dashes: a reading that can't be
/// read, or a negative one. An i64 numerator and a u64 denominator never overflow the i128 sums
/// of the rules above (solution review OV-08).
fn fraction(reading: Option<Exact>) -> Option<(i128, i128)> {
    let reading = reading?;
    let (num, den) = (i128::from(reading.num()), i128::from(reading.den()));
    (num >= 0).then_some((num, den))
}

/// Every number drawn, of every metric, is at most 9999 (owner, 2026-10-25).
fn capped(n: i128) -> Shown {
    Shown::Number(u16::try_from(n.min(i128::from(CAP))).expect("from 0 to 9999"))
}
