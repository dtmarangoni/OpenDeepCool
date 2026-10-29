//! The driver rules as data (`docs/spec/sensors.md`, Driver rules): each row names a metric and
//! where its reading is. Supporting another driver takes a row and a fake tree, not a new code
//! path (PRD, Sensor architecture).

use std::fs;
use std::path::{Path, PathBuf};

use crate::hwmon::HwmonCandidate;
use crate::model::{Metric, Unavailable};
use crate::procfs::{CpuUsageCandidate, MemoryLoadCandidate};
use crate::registry::Candidate;

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
    /// `MemTotal` and `MemAvailable` of `/proc/meminfo`.
    Meminfo,
}

/// A reading of an hwmon device found by its `name`: the channel of `kind` (`temp`, `fan`,
/// `power`) with the first of `labels` it has, and that channel's value file, the first of
/// `files` it has (`input`, `average`). The value is in the source's unit, `scale` of them per
/// unit of the metric.
pub(crate) struct HwmonRule {
    pub driver: &'static str,
    pub kind: &'static str,
    pub labels: &'static [&'static str],
    pub files: &'static [&'static str],
    pub scale: u64,
    pub applies_to: AppliesTo,
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
            labels: &["Package id 0"],
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
            labels: &["Tdie", "Tctl"],
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
        metric: Metric::MemoryLoad,
        source: RuleSource::Meminfo,
    },
];

/// `metric`'s rules as candidates looking under `roots`, in the table's order.
pub(crate) fn candidates(metric: Metric, roots: &Roots) -> Vec<Box<dyn Candidate>> {
    RULES
        .iter()
        .filter(|rule| rule.metric == metric)
        .map(|rule| -> Box<dyn Candidate> {
            match &rule.source {
                RuleSource::Hwmon(hwmon) => Box::new(HwmonCandidate::new(hwmon, roots.clone())),
                RuleSource::ProcStat => Box::new(CpuUsageCandidate::new(roots)),
                RuleSource::Meminfo => Box::new(MemoryLoadCandidate::new(roots)),
            }
        })
        .collect()
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
