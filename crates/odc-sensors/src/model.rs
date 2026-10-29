//! What the display reads: the seven metrics, their exact values or the reason they have none,
//! and the readings port (`docs/spec/sensors.md`, Metrics; `docs/spec/display.md`, Values).

use std::cmp::Ordering;
use std::fmt;
use std::path::PathBuf;
use std::time::Instant;

/// The seven values the display shows (`docs/spec/sensors.md`, Metrics). Each is read from its
/// own source, never from another's (AGENTS.md rule 11).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Metric {
    CpuTemperature,
    CpuUsage,
    CpuPower,
    GpuTemperature,
    GpuUsage,
    GpuPower,
    MemoryLoad,
}

impl Metric {
    /// Every metric, in declaration order.
    pub const ALL: [Metric; 7] = [
        Metric::CpuTemperature,
        Metric::CpuUsage,
        Metric::CpuPower,
        Metric::GpuTemperature,
        Metric::GpuUsage,
        Metric::GpuPower,
        Metric::MemoryLoad,
    ];

    /// DeepCreative's name for the metric, which the expected frames' headings use.
    pub fn spec_name(self) -> &'static str {
        match self {
            Metric::CpuTemperature => "CPU Temperature",
            Metric::CpuUsage => "CPU Usage",
            Metric::CpuPower => "CPU Power",
            Metric::GpuTemperature => "Gpu temperature",
            Metric::GpuUsage => "Gpu usage",
            Metric::GpuPower => "Gpu power",
            Metric::MemoryLoad => "Memory Load",
        }
    }

    pub fn from_spec_name(name: &str) -> Option<Metric> {
        Metric::ALL.into_iter().find(|m| m.spec_name() == name)
    }

    /// The metric's place in [`Metric::ALL`].
    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// A reading's exact value: a fraction of whole numbers in the metric's unit, °C, %, W or RPM.
/// It never goes through floating point, so the display's rounding works on whole numbers
/// (owner, 2026-10-26; `docs/spec/display.md`, Values). It keeps the fraction as its source gave
/// it, and compares by value: 57 000 / 1000 equals 57 / 1.
#[derive(Clone, Copy, Debug)]
pub struct Exact {
    num: i64,
    den: u64,
}

impl Exact {
    /// `num / den`, or `None` when `den` is 0.
    pub fn new(num: i64, den: u64) -> Option<Exact> {
        (den > 0).then_some(Exact { num, den })
    }

    /// A whole number, `n / 1`.
    pub fn whole(n: i64) -> Exact {
        Exact { num: n, den: 1 }
    }

    pub fn num(self) -> i64 {
        self.num
    }

    pub fn den(self) -> u64 {
        self.den
    }
}

impl PartialEq for Exact {
    fn eq(&self, other: &Exact) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Exact {}

impl Ord for Exact {
    fn cmp(&self, other: &Exact) -> Ordering {
        // Both denominators are positive, so cross-multiplying keeps the order, and no product of
        // an i64 and a u64 overflows an i128.
        let left = i128::from(self.num) * i128::from(other.den);
        let right = i128::from(other.num) * i128::from(self.den);
        left.cmp(&right)
    }
}

impl PartialOrd for Exact {
    fn partial_cmp(&self, other: &Exact) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Why a metric has no value. A metric without a source is unavailable, never another source's
/// number (AGENTS.md rule 11). [`fmt::Display`] gives the one-line reason the settings app shows
/// (`docs/spec/settings.md`, What the settings app must offer); paths and messages stay in the
/// fields, for the journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unavailable {
    /// The device is there, but its driver has no rule for this metric.
    NoRule { driver: String },
    /// No device provides the metric.
    NoDevice,
    /// The source is there, but this account may not read it.
    PermissionDenied { path: PathBuf },
    /// The source is there, but reading it failed.
    ReadFailed { path: PathBuf },
    /// The first sample of a reading averaged over the interval: at start-up, and after
    /// [`Readings::restart_intervals`].
    NotYet,
    /// An integrated AMD GPU's power reading includes the CPU's (`docs/spec/sensors.md`, Driver
    /// rules).
    IntegratedGpuPower,
    /// An AMD GPU whose kind is unknown might be integrated, so its power might include the CPU's
    /// (owner, 2026-10-27; solution review P8-06).
    UnknownGpuKind,
    /// The GPU driver was updated since boot; a reboot fixes it (solution review P6-12).
    DriverUpdated,
    /// A driver's library can't be loaded or started, with the loader's or the library's own
    /// message (NVML; overview review's cross-check, 2026-10-27).
    LibraryFailed { library: String, error: String },
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unavailable::NoRule { .. } => f.write_str("no reading for this driver"),
            Unavailable::NoDevice => f.write_str("no device found"),
            Unavailable::PermissionDenied { .. } => f.write_str("no permission to read it"),
            Unavailable::ReadFailed { .. } => f.write_str("reading it failed"),
            Unavailable::NotYet => f.write_str("waiting for the first value"),
            Unavailable::IntegratedGpuPower => {
                f.write_str("an integrated GPU's power includes the CPU's")
            }
            Unavailable::UnknownGpuKind => {
                f.write_str("the GPU's kind is unknown, so its power might include the CPU's")
            }
            Unavailable::DriverUpdated => f.write_str("the GPU driver was updated: restart the PC"),
            Unavailable::LibraryFailed { library, .. } => {
                // One line, even if the name came from elsewhere.
                let library: String = library
                    .chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect();
                write!(f, "{library} can't be loaded or started")
            }
        }
    }
}

/// A metric's value, or the reason it has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading {
    Value(Exact),
    Unavailable(Unavailable),
}

/// The cooler's fans or its pump: the reading, in RPM, of the fan input the user chose for them,
/// or none chosen, as by default (`docs/spec/sensors.md`, Fan and pump speed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fan {
    NotChosen,
    Chosen(Reading),
}

/// Every metric's reading at one moment, and the speeds of the cooler's fans and pump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    metrics: [Reading; Metric::ALL.len()],
    pub cooler_fans: Fan,
    pub pump: Fan,
}

impl Snapshot {
    /// A snapshot with `reading(metric)` for each metric, asked in the order of [`Metric::ALL`].
    pub fn new(reading: impl FnMut(Metric) -> Reading, cooler_fans: Fan, pump: Fan) -> Snapshot {
        Snapshot {
            metrics: Metric::ALL.map(reading),
            cooler_fans,
            pump,
        }
    }

    pub fn get(&self, metric: Metric) -> &Reading {
        &self.metrics[metric.index()]
    }
}

/// The readings port: what Phase 7's scheduler asks once per second (PRD, Sensor architecture).
pub trait Readings {
    /// Every metric's reading now.
    fn snapshot(&mut self) -> Snapshot;

    /// Starts every reading averaged over the interval again, as at start-up: CPU Usage, and
    /// power from an energy counter. Their next reading is [`Unavailable::NotYet`]. Phase 7 calls
    /// it after every wake-up from standby, so no average spans the sleep (owner, 2026-10-26;
    /// `docs/spec/sensors.md`, Updates and rounding).
    fn restart_intervals(&mut self);
}

/// Where the registry and the readings averaged over the interval get the time. Tests use
/// [`crate::fake::FakeClock`].
pub trait Clock {
    fn now(&self) -> Instant;
}

/// The system's monotonic clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_metric_indexes_its_own_place() {
        for (i, metric) in Metric::ALL.into_iter().enumerate() {
            assert_eq!(metric.index(), i, "{metric:?}");
        }
    }
}
