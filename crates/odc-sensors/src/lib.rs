//! The display's readings on any PC (PRD, Architecture Notes: Sensor architecture): the registry
//! binds one source per metric from built-in driver rules and reads them all once per second
//! through the [`Readings`] port (`docs/spec/sensors.md`).

pub mod fake;
mod fans;
mod gpu;
mod hwmon;
mod json;
mod model;
mod powercap;
mod procfs;
mod registry;
mod rules;
mod sysfs;

pub use fans::{FanInput, fan_inputs, fan_rpm};
pub use gpu::{
    Gpu, GpuKind, PciAddr, RANK_FILE, Ranked, RankedKind, Ranking, Review, Unknown, choose,
    display_devices, gpus, rank_is_stale, review_ranking,
};
pub use model::{Clock, Exact, Fan, Metric, Reading, Readings, Snapshot, SystemClock, Unavailable};
pub use powercap::rapl_package_zones;
pub use registry::{Binding, Candidate, Registry, Source};
pub use rules::Roots;
