//! The generic hwmon adapter (`Documentation/hwmon/sysfs-interface.rst`): it finds a device by
//! its `name`, and for a GPU only the chosen GPU's own device, never by an index; it finds the
//! channel by its label or by the number its documentation names, and reads the value as an exact
//! fraction (`docs/spec/sensors.md`, Driver rules).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::model::{Exact, Reading, Unavailable};
use crate::registry::{Candidate, Source};
use crate::rules::{Channel, HwmonRule, Roots};
use crate::sysfs::read_integer;

/// One rule over the hwmon devices under a root.
pub(crate) struct HwmonCandidate {
    rule: &'static HwmonRule,
    roots: Roots,
    /// For a GPU, the GPU's own device folder: only an hwmon device whose `device` is it counts.
    device: Option<PathBuf>,
}

impl HwmonCandidate {
    pub(crate) fn new(rule: &'static HwmonRule, roots: Roots) -> Self {
        HwmonCandidate {
            rule,
            roots,
            device: None,
        }
    }

    pub(crate) fn for_device(rule: &'static HwmonRule, roots: Roots, device: PathBuf) -> Self {
        HwmonCandidate {
            rule,
            roots,
            device: Some(device),
        }
    }
}

impl Candidate for HwmonCandidate {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        let rule = self.rule;
        let devices = matching(&self.roots.sys, rule.driver, self.device.as_deref());
        if devices.is_empty() {
            return Err(Unavailable::NoDevice);
        }
        rule.applies_to.check(&self.roots, rule.driver)?;
        for dir in &devices {
            let channels: Vec<(u32, String)> = match &rule.channel {
                Channel::Labelled(labels) => labels
                    .iter()
                    .filter_map(|label| {
                        channel_labelled(dir, rule.kind, label).map(|n| (n, format!(" ({label})")))
                    })
                    .collect(),
                Channel::Numbered(n) => vec![(*n, String::new())],
            };
            for (n, label) in channels {
                for file in rule.files {
                    let value = dir.join(format!("{}{n}_{file}", rule.kind));
                    // Pinned to the device's own folder, so a number another device takes later
                    // fails instead of reading that device (AGENTS.md rule 11).
                    let Ok(path) = value.canonicalize() else {
                        continue;
                    };
                    let source = HwmonSource {
                        path,
                        scale: rule.scale,
                        name: format!("{} {}{n}_{file}{label}", rule.driver, rule.kind),
                    };
                    source.value()?;
                    return Ok(Box::new(source));
                }
            }
        }
        Err(Unavailable::NoRule {
            driver: rule.driver.to_owned(),
        })
    }
}

/// Every hwmon device under `<sys>/class/hwmon`, as (folder, name), in the order of their
/// numbers.
pub(crate) fn devices(sys: &Path) -> Vec<(PathBuf, String)> {
    let Ok(entries) = fs::read_dir(sys.join("class/hwmon")) else {
        return Vec::new();
    };
    let mut devices: Vec<(u32, PathBuf, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let n = entry
                .file_name()
                .to_str()?
                .strip_prefix("hwmon")?
                .parse()
                .ok()?;
            let dir = entry.path();
            let name = fs::read_to_string(dir.join("name")).ok()?;
            Some((n, dir, name.trim().to_owned()))
        })
        .collect();
    devices.sort_by_key(|(n, _, _)| *n);
    devices
        .into_iter()
        .map(|(_, dir, name)| (dir, name))
        .collect()
}

/// The hwmon devices named `name`, and when `device` is given, only those whose `device` link
/// leads to it.
fn matching(sys: &Path, name: &str, device: Option<&Path>) -> Vec<PathBuf> {
    let wanted = device.map(Path::canonicalize);
    devices(sys)
        .into_iter()
        .filter(|(dir, found)| {
            found == name
                && match &wanted {
                    None => true,
                    Some(Ok(wanted)) => {
                        dir.join("device").canonicalize().ok().as_ref() == Some(wanted)
                    }
                    Some(Err(_)) => false,
                }
        })
        .map(|(dir, _)| dir)
        .collect()
}

/// The lowest N whose `<kind>N_label` in `dir` reads `label`.
fn channel_labelled(dir: &Path, kind: &str, label: &str) -> Option<u32> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let n: u32 = name
                .strip_prefix(kind)?
                .strip_suffix("_label")?
                .parse()
                .ok()?;
            (fs::read_to_string(entry.path()).ok()?.trim() == label).then_some(n)
        })
        .min()
}

/// An hwmon channel's value file, read as an integer and divided by `scale`.
struct HwmonSource {
    path: PathBuf,
    scale: u64,
    name: String,
}

impl HwmonSource {
    fn value(&self) -> Result<Reading, Unavailable> {
        let raw = read_integer(&self.path)?;
        Exact::new(raw, self.scale)
            .map(Reading::Value)
            .ok_or_else(|| Unavailable::ReadFailed {
                path: self.path.clone(),
            })
    }
}

impl Source for HwmonSource {
    fn read(&mut self, _now: Instant) -> Result<Reading, Unavailable> {
        self.value()
    }

    fn describe(&self) -> String {
        format!("{}, {}", self.name, self.path.display())
    }
}

/// A GPU's power from its hwmon energy counter, `energyN_input` in microjoules
/// (`Documentation/ABI/testing/sysfs-driver-intel-i915-hwmon`, `…-xe-hwmon`).
pub(crate) struct EnergyCandidate {
    hwmon: &'static str,
    channel: u32,
    roots: Roots,
    device: PathBuf,
}

impl EnergyCandidate {
    pub(crate) fn new(hwmon: &'static str, channel: u32, roots: Roots, device: PathBuf) -> Self {
        EnergyCandidate {
            hwmon,
            channel,
            roots,
            device,
        }
    }
}

impl Candidate for EnergyCandidate {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        let devices = matching(&self.roots.sys, self.hwmon, Some(&self.device));
        let Some(dir) = devices.first() else {
            return Err(Unavailable::NoDevice);
        };
        let file = dir.join(format!("energy{}_input", self.channel));
        let Ok(path) = file.canonicalize() else {
            return Err(Unavailable::NoRule {
                driver: self.hwmon.to_owned(),
            });
        };
        read_integer::<u64>(&path)?;
        Ok(Box::new(Energy {
            path,
            last: None,
            previous: None,
        }))
    }
}

/// ΔµJ ÷ Δµs between two samples of an energy counter: watts.
struct Energy {
    path: PathBuf,
    last: Option<(Instant, u64)>,
    previous: Option<Exact>,
}

impl Source for Energy {
    fn read(&mut self, now: Instant) -> Result<Reading, Unavailable> {
        let counter: u64 = read_integer(&self.path)?;
        let Some((then, before)) = self.last.replace((now, counter)) else {
            return Ok(Reading::Unavailable(Unavailable::NotYet));
        };
        let micros = now.saturating_duration_since(then).as_micros();
        let value = match counter.checked_sub(before) {
            // The counter went back: start a new interval rather than guess a wrap, which the
            // documents don't describe.
            None => None,
            Some(_) if micros == 0 => self.previous,
            Some(energy) => i64::try_from(energy)
                .ok()
                .zip(u64::try_from(micros).ok())
                .and_then(|(energy, micros)| Exact::new(energy, micros)),
        };
        self.previous = value;
        Ok(value.map_or(Reading::Unavailable(Unavailable::NotYet), Reading::Value))
    }

    fn restart(&mut self) {
        self.last = None;
        self.previous = None;
    }

    fn describe(&self) -> String {
        format!("energy over the interval, {}", self.path.display())
    }
}
