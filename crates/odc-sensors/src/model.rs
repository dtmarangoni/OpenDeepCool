//! The readings port and the clock (`docs/spec/sensors.md`). The vocabulary they speak, the metrics,
//! their values and snapshots, is the `odc-metrics` crate's, re-exported here so that this crate's
//! users keep their paths.

use std::time::Instant;

pub use odc_metrics::{Exact, Metric, Reading, Snapshot, Unavailable};

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
