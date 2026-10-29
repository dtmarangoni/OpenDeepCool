//! CPU Power from the RAPL package energy counters (`Documentation/power/powercap/powercap.rst`;
//! `docs/spec/sensors.md`, CPU power): the energy every package zone used over the interval,
//! divided by the interval, kept as whole microjoules over whole microseconds, which is watts.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::model::{Exact, Reading, Unavailable};
use crate::registry::{Candidate, Source};
use crate::rules::Roots;
use crate::sysfs::read_integer;

/// The CPU's package zones under `<sys>/class/powercap`, resolved and sorted: the zones directly
/// under the control type `intel-rapl`, which AMD and Hygon CPUs use too, whose `name` starts with
/// `package-` (`package-N`, or `package-N-die-M`). Never a sub-zone, never an `intel-rapl-mmio`
/// zone, and never the control type itself. `odc-dev` and the hardware probe find them this way
/// too.
pub fn rapl_package_zones(sys: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(sys.join("class/powercap")) else {
        return Vec::new();
    };
    let mut zones: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| entry.path().canonicalize().ok())
        .filter(|zone| {
            let under_rapl = zone
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|control| control == "intel-rapl");
            let package = fs::read_to_string(zone.join("name"))
                .is_ok_and(|name| name.trim().starts_with("package-"));
            under_rapl && package
        })
        .collect();
    zones.sort();
    zones
}

/// CPU Power's rule: every package zone.
pub(crate) struct CpuPowerCandidate {
    sys: PathBuf,
}

impl CpuPowerCandidate {
    pub(crate) fn new(roots: &Roots) -> Self {
        CpuPowerCandidate {
            sys: roots.sys.clone(),
        }
    }
}

impl Candidate for CpuPowerCandidate {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        let zones = rapl_package_zones(&self.sys);
        if zones.is_empty() {
            return Err(Unavailable::NoDevice);
        }
        let zones = zones
            .into_iter()
            .map(|dir| {
                let energy = dir.join("energy_uj");
                // A first read proves this account may read it.
                read_integer::<u64>(&energy)?;
                let max = read_integer(&dir.join("max_energy_range_uj"))?;
                Ok(Zone { energy, max })
            })
            .collect::<Result<Vec<_>, Unavailable>>()?;
        Ok(Box::new(CpuPower {
            zones,
            last: None,
            previous: None,
        }))
    }
}

/// One package zone's counter, which wraps to 0 after `max` µJ.
struct Zone {
    energy: PathBuf,
    max: u64,
}

/// Σ ΔµJ ÷ Δµs over the package zones.
struct CpuPower {
    zones: Vec<Zone>,
    /// The last sample: when, and each zone's counter.
    last: Option<(Instant, Vec<u64>)>,
    /// The last value, kept when an interval has no time.
    previous: Option<Exact>,
}

impl Source for CpuPower {
    fn read(&mut self, now: Instant) -> Result<Reading, Unavailable> {
        let counters = self
            .zones
            .iter()
            .map(|zone| read_integer::<u64>(&zone.energy))
            .collect::<Result<Vec<u64>, Unavailable>>()?;
        let Some((then, before)) = self.last.replace((now, counters.clone())) else {
            return Ok(Reading::Unavailable(Unavailable::NotYet));
        };
        let micros = now.saturating_duration_since(then).as_micros();
        if micros == 0 {
            return Ok(self
                .previous
                .map_or(Reading::Unavailable(Unavailable::NotYet), Reading::Value));
        }
        let mut energy: u128 = 0;
        for ((zone, after), before) in self.zones.iter().zip(&counters).zip(&before) {
            // A counter lower than before has wrapped (powercap.rst, `max_energy_range_uj`). A
            // sleep can lower it too, but a wake-up restarts the interval first.
            energy += if after >= before {
                u128::from(after - before)
            } else {
                u128::from(zone.max) - u128::from(*before) + u128::from(*after)
            };
        }
        let value = i64::try_from(energy)
            .ok()
            .zip(u64::try_from(micros).ok())
            .and_then(|(energy, micros)| Exact::new(energy, micros));
        self.previous = value;
        Ok(value.map_or(Reading::Unavailable(Unavailable::NotYet), Reading::Value))
    }

    fn restart(&mut self) {
        self.last = None;
        self.previous = None;
    }

    fn describe(&self) -> String {
        let zones: Vec<String> = self
            .zones
            .iter()
            .map(|zone| zone.energy.display().to_string())
            .collect();
        format!("RAPL package energy, {}", zones.join(", "))
    }
}
