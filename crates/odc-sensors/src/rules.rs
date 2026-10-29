//! The driver rules as data (`docs/spec/sensors.md`, Driver rules): each row names a metric and
//! where its reading is. Supporting another driver takes a row and a fake tree, not a new code
//! path (PRD, Sensor architecture).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::drm::BusyPercentCandidate;
use crate::gpu::{Gpu, GpuKind};
use crate::hwmon::{EnergyCandidate, HwmonCandidate};
use crate::model::{Metric, Unavailable};
use crate::nvml::{NvmlCandidate, NvmlReading, NvmlSource};
use crate::powercap::CpuPowerCandidate;
use crate::procfs::{CpuUsageCandidate, MemoryLoadCandidate};
use crate::registry::{Candidate, Source};

/// Where the rules look: `/sys` and `/proc`, or a fake tree's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roots {
    pub sys: PathBuf,
    pub proc: PathBuf,
}

impl Roots {
    pub fn system() -> Roots {
        Roots {
            sys: PathBuf::from("/sys"),
            proc: PathBuf::from("/proc"),
        }
    }
}

pub(crate) struct Rule {
    metric: Metric,
    source: RuleSource,
}

enum RuleSource {
    Hwmon(HwmonRule),
    /// The aggregate `cpu` line of `/proc/stat`.
    ProcStat,
    /// The RAPL package zones' energy counters.
    RaplPackages,
    /// `MemTotal` and `MemAvailable` of `/proc/meminfo`.
    Meminfo,
}

/// A reading of an hwmon device found by its `name` (`driver`): a channel of `kind` (`temp`,
/// `power`), and that channel's value file, the first of `files` it has (`input`, `average`). The
/// value is in the source's unit, `scale` of them per unit of the metric.
pub(crate) struct HwmonRule {
    pub driver: &'static str,
    pub kind: &'static str,
    pub channel: Channel,
    pub files: &'static [&'static str],
    pub scale: u64,
    pub applies_to: AppliesTo,
}

/// Which channel of an hwmon device a rule reads.
pub(crate) enum Channel {
    /// The channel with the first of these labels it has.
    Labelled(&'static [&'static str]),
    /// The channel with this number, for drivers whose documentation names the file itself.
    Numbered(u32),
}

/// When a rule's readings mean what the metric says.
pub(crate) enum AppliesTo {
    Always,
    /// On AMD CPU families from this one on: before family 17h, k10temp's `Tctl` isn't a
    /// temperature (the k10temp documentation; `docs/spec/sensors.md`).
    CpuFamilyFrom(u32),
}

static RULES: &[Rule] = &[
    Rule {
        metric: Metric::CpuTemperature,
        source: RuleSource::Hwmon(HwmonRule {
            driver: "coretemp",
            kind: "temp",
            channel: Channel::Labelled(&["Package id 0"]),
            files: &["input"],
            scale: 1_000,
            applies_to: AppliesTo::Always,
        }),
    },
    Rule {
        metric: Metric::CpuTemperature,
        source: RuleSource::Hwmon(HwmonRule {
            driver: "k10temp",
            kind: "temp",
            channel: Channel::Labelled(&["Tdie", "Tctl"]),
            files: &["input"],
            scale: 1_000,
            applies_to: AppliesTo::CpuFamilyFrom(0x17),
        }),
    },
    Rule {
        metric: Metric::CpuUsage,
        source: RuleSource::ProcStat,
    },
    Rule {
        metric: Metric::CpuPower,
        source: RuleSource::RaplPackages,
    },
    Rule {
        metric: Metric::MemoryLoad,
        source: RuleSource::Meminfo,
    },
];

/// `metric`'s rules as candidates looking under `roots`, in the table's order. The GPU metrics
/// have none here: they follow the chosen GPU ([`gpu_candidates`]).
pub(crate) fn candidates(metric: Metric, roots: &Roots) -> Vec<Box<dyn Candidate>> {
    RULES
        .iter()
        .filter(|rule| rule.metric == metric)
        .map(|rule| -> Box<dyn Candidate> {
            match &rule.source {
                RuleSource::Hwmon(hwmon) => Box::new(HwmonCandidate::new(hwmon, roots.clone())),
                RuleSource::ProcStat => Box::new(CpuUsageCandidate::new(roots)),
                RuleSource::RaplPackages => Box::new(CpuPowerCandidate::new(roots)),
                RuleSource::Meminfo => Box::new(MemoryLoadCandidate::new(roots)),
            }
        })
        .collect()
}

/// What a GPU rule reads, always of the chosen GPU.
enum GpuSource {
    /// A channel of the GPU's own hwmon device.
    Hwmon(HwmonRule),
    /// The PCI device's `gpu_busy_percent`, a whole percent.
    BusyPercent,
    /// Power from the GPU's hwmon energy counter `energyN_input` over the interval.
    Energy { hwmon: &'static str, channel: u32 },
    /// One of NVML's readings.
    Nvml(NvmlReading),
    /// Nothing: the driver documents no such reading.
    Nothing,
}

struct GpuRule {
    driver: &'static str,
    metric: Metric,
    source: GpuSource,
    /// Shown only for a GPU ranked dedicated: integrated AMD graphics count the CPU in their power
    /// (owner, 2026-10-27; solution review P8-06).
    dedicated_only: bool,
}

static GPU_RULES: &[GpuRule] = &[
    GpuRule {
        driver: "amdgpu",
        metric: Metric::GpuTemperature,
        source: GpuSource::Hwmon(HwmonRule {
            driver: "amdgpu",
            kind: "temp",
            channel: Channel::Labelled(&["edge"]),
            files: &["input"],
            scale: 1_000,
            applies_to: AppliesTo::Always,
        }),
        dedicated_only: false,
    },
    GpuRule {
        driver: "amdgpu",
        metric: Metric::GpuUsage,
        source: GpuSource::BusyPercent,
        dedicated_only: false,
    },
    GpuRule {
        driver: "amdgpu",
        metric: Metric::GpuPower,
        source: GpuSource::Hwmon(HwmonRule {
            driver: "amdgpu",
            kind: "power",
            channel: Channel::Numbered(1),
            files: &["average", "input"],
            scale: 1_000_000,
            applies_to: AppliesTo::Always,
        }),
        dedicated_only: true,
    },
    GpuRule {
        driver: "i915",
        metric: Metric::GpuTemperature,
        source: GpuSource::Hwmon(HwmonRule {
            driver: "i915",
            kind: "temp",
            channel: Channel::Numbered(1),
            files: &["input"],
            scale: 1_000,
            applies_to: AppliesTo::Always,
        }),
        dedicated_only: false,
    },
    GpuRule {
        driver: "i915",
        metric: Metric::GpuUsage,
        source: GpuSource::Nothing,
        dedicated_only: false,
    },
    GpuRule {
        driver: "i915",
        metric: Metric::GpuPower,
        source: GpuSource::Energy {
            hwmon: "i915",
            channel: 1,
        },
        dedicated_only: false,
    },
    GpuRule {
        driver: "xe",
        metric: Metric::GpuTemperature,
        source: GpuSource::Hwmon(HwmonRule {
            driver: "xe",
            kind: "temp",
            channel: Channel::Numbered(2),
            files: &["input"],
            scale: 1_000,
            applies_to: AppliesTo::Always,
        }),
        dedicated_only: false,
    },
    GpuRule {
        driver: "xe",
        metric: Metric::GpuUsage,
        source: GpuSource::Nothing,
        dedicated_only: false,
    },
    GpuRule {
        driver: "xe",
        metric: Metric::GpuPower,
        source: GpuSource::Energy {
            hwmon: "xe",
            channel: 1,
        },
        dedicated_only: false,
    },
    GpuRule {
        driver: "nvidia",
        metric: Metric::GpuTemperature,
        source: GpuSource::Nvml(NvmlReading::Temperature),
        dedicated_only: false,
    },
    GpuRule {
        driver: "nvidia",
        metric: Metric::GpuUsage,
        source: GpuSource::Nvml(NvmlReading::Usage),
        dedicated_only: false,
    },
    GpuRule {
        driver: "nvidia",
        metric: Metric::GpuPower,
        source: GpuSource::Nvml(NvmlReading::Power),
        dedicated_only: false,
    },
];

/// The chosen GPU's rules for `metric`, as candidates looking under `roots`. A driver without
/// rows, such as nouveau, has no reading for any GPU metric.
pub(crate) fn gpu_candidates(
    metric: Metric,
    gpu: &Gpu,
    roots: &Roots,
    nvml: &Arc<Mutex<Box<dyn NvmlSource>>>,
) -> Vec<Box<dyn Candidate>> {
    let device = roots.sys.join("bus/pci/devices").join(gpu.pci.to_string());
    let no_rule = || Unavailable::NoRule {
        driver: gpu.driver.clone(),
    };
    let rows: Vec<&GpuRule> = GPU_RULES
        .iter()
        .filter(|rule| rule.driver == gpu.driver && rule.metric == metric)
        .collect();
    if rows.is_empty() {
        return vec![Box::new(Refuse(no_rule()))];
    }
    rows.into_iter()
        .map(|rule| -> Box<dyn Candidate> {
            if rule.dedicated_only {
                match gpu.kind {
                    GpuKind::Dedicated => {}
                    GpuKind::Integrated => {
                        return Box::new(Refuse(Unavailable::IntegratedGpuPower));
                    }
                    GpuKind::Unknown(_) => return Box::new(Refuse(Unavailable::UnknownGpuKind)),
                }
            }
            match &rule.source {
                GpuSource::Hwmon(hwmon) => Box::new(HwmonCandidate::for_device(
                    hwmon,
                    roots.clone(),
                    device.clone(),
                )),
                GpuSource::BusyPercent => Box::new(BusyPercentCandidate::new(device.clone())),
                GpuSource::Energy { hwmon, channel } => Box::new(EnergyCandidate::new(
                    hwmon,
                    *channel,
                    roots.clone(),
                    device.clone(),
                )),
                GpuSource::Nvml(reading) => Box::new(NvmlCandidate::new(
                    *reading,
                    gpu.pci.to_string(),
                    Arc::clone(nvml),
                )),
                GpuSource::Nothing => Box::new(Refuse(no_rule())),
            }
        })
        .collect()
}

/// A rule that never binds, for a reading the rules exclude, with the reason.
struct Refuse(Unavailable);

impl Candidate for Refuse {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        Err(self.0.clone())
    }
}

impl AppliesTo {
    /// `Ok` when the rule's readings apply on this PC; else why not, for a device of `driver`
    /// that's there.
    pub(crate) fn check(&self, roots: &Roots, driver: &str) -> Result<(), Unavailable> {
        match self {
            AppliesTo::Always => Ok(()),
            AppliesTo::CpuFamilyFrom(first) => {
                if cpu_family(&roots.proc.join("cpuinfo"))? >= *first {
                    Ok(())
                } else {
                    Err(Unavailable::NoRule {
                        driver: driver.to_owned(),
                    })
                }
            }
        }
    }
}

/// The `cpu family` of the first CPU in `/proc/cpuinfo` (proc_cpuinfo(5)), written in decimal.
fn cpu_family(path: &Path) -> Result<u32, Unavailable> {
    let failed = || Unavailable::ReadFailed {
        path: path.to_owned(),
    };
    let text = fs::read_to_string(path).map_err(|_| failed())?;
    text.lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == "cpu family").then(|| value.trim().parse().ok())?
        })
        .ok_or_else(failed)
}
