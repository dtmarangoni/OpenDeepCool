//! The display's readings on any PC (PRD, Architecture Notes: Sensor architecture): the registry
//! binds one source per metric from built-in driver rules and reads them all once per second
//! through the [`Readings`] port (`docs/spec/sensors.md`).

mod drm;
pub mod fake;
mod gpu;
mod hwmon;
pub mod json;
mod model;
mod nvml;
mod powercap;
mod procfs;
mod registry;
mod rules;
mod sysfs;

pub use gpu::{
    Gpu, GpuKind, PciAddr, RANK_FILE, Ranked, RankedKind, Ranking, Review, Unknown, choose,
    delete_stale, display_devices, gpus, rank_is_stale, review_ranking,
};
pub use model::{Clock, Exact, Metric, Reading, Readings, Snapshot, SystemClock, Unavailable};
pub use nvml::{NvmlReading, NvmlSource, RealNvml};
pub use powercap::rapl_package_zones;
pub use registry::{Binding, Candidate, Registry, Source};
pub use rules::{Roots, has_gpu_rule};
