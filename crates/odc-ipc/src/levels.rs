//! The `Levels` signal's values (`docs/spec/dbus.md`, Signals; Phase 11a Task 13): the 14 column
//! levels of the spectrum the engine analysed for a tick, each finite and within 0 to 1.

/// The columns of the spectrum.
pub const LEVELS: usize = 14;

/// The levels in a signal's `levels`; `None` unless they're 14 values, each finite and within 0
/// to 1. A client that gets `None` ignores the signal; the service never sends one.
pub fn levels_from(levels: &[f64]) -> Option<[f64; LEVELS]> {
    let levels: [f64; LEVELS] = levels.try_into().ok()?;
    levels
        .iter()
        .all(|level| level.is_finite() && (0.0..=1.0).contains(level))
        .then_some(levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourteen_levels_within_zero_to_one_are_taken() {
        let levels: Vec<f64> = (0..14).map(|i| f64::from(i) / 13.0).collect();
        assert_eq!(levels_from(&levels).map(|l| l.len()), Some(14));
        assert!(levels_from(&[0.0; 14]).is_some());
        assert!(levels_from(&[1.0; 14]).is_some());
    }

    #[test]
    fn another_length_or_a_value_outside_zero_to_one_is_refused() {
        assert_eq!(levels_from(&[0.5; 13]), None);
        assert_eq!(levels_from(&[0.5; 15]), None);
        assert_eq!(levels_from(&[]), None);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.001, 1.001] {
            for at in [0, 7, 13] {
                let mut levels = [0.5; 14];
                levels[at] = bad;
                assert_eq!(levels_from(&levels), None, "{bad} at {at}");
            }
        }
    }
}
