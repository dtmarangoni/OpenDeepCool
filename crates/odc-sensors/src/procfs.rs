//! CPU Usage from `/proc/stat` and Memory Load from `/proc/meminfo` (`docs/spec/sensors.md`,
//! Driver rules), kept as exact fractions of the kernel's whole numbers.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::model::{Exact, Reading, Unavailable};
use crate::registry::{Candidate, Source};
use crate::rules::Roots;

fn read_failed(path: &Path) -> Unavailable {
    Unavailable::ReadFailed {
        path: path.to_owned(),
    }
}

fn read(path: &Path) -> Result<String, Unavailable> {
    fs::read_to_string(path).map_err(|_| read_failed(path))
}

/// The aggregate `cpu` line's counters, in jiffies: the total of its first eight fields, user to
/// steal, and the idle part, idle plus iowait (proc_stat(5)). Guest time is already counted in
/// user and nice (the kernel's `account_guest_time`), so it isn't added again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CpuTimes {
    total: u64,
    idle: u64,
}

fn cpu_times(path: &Path) -> Result<CpuTimes, Unavailable> {
    let text = read(path)?;
    let fields: Vec<u64> = text
        .lines()
        .find_map(|line| {
            let mut words = line.split_whitespace();
            (words.next() == Some("cpu")).then(|| words.map(|w| w.parse::<u64>()).collect())
        })
        .and_then(Result::ok)
        .filter(|fields: &Vec<u64>| fields.len() >= 8)
        .ok_or_else(|| read_failed(path))?;
    let total = fields[..8]
        .iter()
        .try_fold(0u64, |sum, n| sum.checked_add(*n))
        .ok_or_else(|| read_failed(path))?;
    let idle = fields[3]
        .checked_add(fields[4])
        .ok_or_else(|| read_failed(path))?;
    Ok(CpuTimes { total, idle })
}

/// CPU Usage's rule: the aggregate `cpu` line of `/proc/stat`.
pub(crate) struct CpuUsageCandidate {
    path: PathBuf,
}

impl CpuUsageCandidate {
    pub(crate) fn new(roots: &Roots) -> Self {
        CpuUsageCandidate {
            path: roots.proc.join("stat"),
        }
    }
}

impl Candidate for CpuUsageCandidate {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        cpu_times(&self.path)?;
        Ok(Box::new(CpuUsage {
            path: self.path.clone(),
            last: None,
            previous: None,
        }))
    }
}

/// The busy share of the interval between two samples: 100 · busy ÷ total, in whole jiffies.
struct CpuUsage {
    path: PathBuf,
    last: Option<CpuTimes>,
    /// The last value, kept when an interval has no jiffies.
    previous: Option<Exact>,
}

impl Source for CpuUsage {
    fn read(&mut self, _now: Instant) -> Result<Reading, Unavailable> {
        let now = cpu_times(&self.path)?;
        let Some(last) = self.last.replace(now) else {
            return Ok(Reading::Unavailable(Unavailable::NotYet));
        };
        let total = now.total.checked_sub(last.total);
        let idle = i128::from(now.idle) - i128::from(last.idle);
        let value = match total {
            // The counters went back, which they don't; start a new interval.
            None => {
                self.previous = None;
                None
            }
            Some(0) => self.previous,
            Some(total) => {
                // iowait "is not reliable" and can go down (proc_stat(5)): keep busy within the
                // interval.
                let busy = (i128::from(total) - idle).clamp(0, i128::from(total));
                let value = i64::try_from(100 * busy)
                    .ok()
                    .and_then(|busy| Exact::new(busy, total));
                self.previous = value;
                value
            }
        };
        Ok(value.map_or(Reading::Unavailable(Unavailable::NotYet), Reading::Value))
    }

    fn restart(&mut self) {
        self.last = None;
        self.previous = None;
    }

    fn describe(&self) -> String {
        format!("the aggregate cpu line, {}", self.path.display())
    }
}

/// `MemTotal` and `MemAvailable` in kB (proc_meminfo(5)).
fn memory(path: &Path) -> Result<(u64, u64), Unavailable> {
    let text = read(path)?;
    let field = |key: &str| {
        text.lines().find_map(|line| {
            let rest = line.strip_prefix(key)?.strip_prefix(':')?;
            rest.trim().strip_suffix("kB")?.trim().parse::<u64>().ok()
        })
    };
    match (field("MemTotal"), field("MemAvailable")) {
        (Some(total), Some(available)) if total > 0 => Ok((total, available)),
        _ => Err(read_failed(path)),
    }
}

/// Memory Load's rule: `/proc/meminfo`.
pub(crate) struct MemoryLoadCandidate {
    path: PathBuf,
}

impl MemoryLoadCandidate {
    pub(crate) fn new(roots: &Roots) -> Self {
        MemoryLoadCandidate {
            path: roots.proc.join("meminfo"),
        }
    }
}

impl Candidate for MemoryLoadCandidate {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        memory(&self.path)?;
        Ok(Box::new(MemoryLoad {
            path: self.path.clone(),
        }))
    }
}

/// 100 · (MemTotal − MemAvailable) ÷ MemTotal.
struct MemoryLoad {
    path: PathBuf,
}

impl Source for MemoryLoad {
    fn read(&mut self, _now: Instant) -> Result<Reading, Unavailable> {
        let (total, available) = memory(&self.path)?;
        let used = i128::from(total.saturating_sub(available));
        i64::try_from(100 * used)
            .ok()
            .and_then(|used| Exact::new(used, total))
            .map(Reading::Value)
            .ok_or_else(|| read_failed(&self.path))
    }

    fn describe(&self) -> String {
        format!("MemTotal and MemAvailable, {}", self.path.display())
    }
}
