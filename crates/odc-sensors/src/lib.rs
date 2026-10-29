//! The display's readings on any PC (PRD, Architecture Notes: Sensor architecture): the registry
//! binds one source per metric from built-in driver rules and reads them all once per second
//! through the [`Readings`] port (`docs/spec/sensors.md`).

pub mod fake;
mod hwmon;
mod model;
mod procfs;
mod registry;
mod rules;

pub use model::{Clock, Exact, Fan, Metric, Reading, Readings, Snapshot, SystemClock, Unavailable};
pub use registry::{Binding, Candidate, Registry, Source};
pub use rules::Roots;
