//! The generic hwmon adapter (`Documentation/hwmon/sysfs-interface.rst`): it finds a device by
//! its `name` and a channel by its label, never by an index, and reads the channel's value as an
//! exact fraction (`docs/spec/sensors.md`, Driver rules).

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::model::{Exact, Reading, Unavailable};
use crate::registry::{Candidate, Source};
use crate::rules::{HwmonRule, Roots};

/// One rule over the hwmon devices under a root.
pub(crate) struct HwmonCandidate {
    rule: &'static HwmonRule,
    roots: Roots,
}

impl HwmonCandidate {
    pub(crate) fn new(rule: &'static HwmonRule, roots: Roots) -> Self {
        HwmonCandidate { rule, roots }
    }
}

impl Candidate for HwmonCandidate {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        let rule = self.rule;
        let devices: Vec<PathBuf> = devices(&self.roots.sys)
            .into_iter()
            .filter(|(_, name)| name == rule.driver)
            .map(|(dir, _)| dir)
            .collect();
        if devices.is_empty() {
            return Err(Unavailable::NoDevice);
        }
        rule.applies_to.check(&self.roots, rule.driver)?;
        for dir in &devices {
            for label in rule.labels {
                let Some(n) = channel_labelled(dir, rule.kind, label) else {
                    continue;
                };
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
                        name: format!("{} {}{n} ({label})", rule.driver, rule.kind),
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
fn devices(sys: &Path) -> Vec<(PathBuf, String)> {
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

/// A sysfs file holding one integer, as the kernel writes it.
pub(crate) fn read_integer(path: &Path) -> Result<i64, Unavailable> {
    let text = fs::read_to_string(path).map_err(|e| match e.kind() {
        ErrorKind::PermissionDenied => Unavailable::PermissionDenied {
            path: path.to_owned(),
        },
        _ => Unavailable::ReadFailed {
            path: path.to_owned(),
        },
    })?;
    text.trim().parse().map_err(|_| Unavailable::ReadFailed {
        path: path.to_owned(),
    })
}
