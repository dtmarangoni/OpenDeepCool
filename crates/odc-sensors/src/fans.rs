//! The fan inputs the user picks for the cooler's fans and pump: every hwmon fan input except a
//! graphics card's, found again on every read by chip name, device path and number, never by hwmon
//! index (`docs/spec/sensors.md`, Fan and pump speed).

use std::fs;
use std::path::{Path, PathBuf};

use crate::gpu::is_display;
use crate::hwmon::devices;
use crate::model::{Exact, Reading, Unavailable};
use crate::sysfs::read_integer;

/// One fan input, saved as the chip's name, its device's path and the input's number.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FanInput {
    /// The hwmon device's `name`.
    pub chip: String,
    /// Its parent device, relative to the sysfs root, for example `devices/platform/chip.656`.
    pub device: PathBuf,
    /// N of `fanN_input`.
    pub input: u32,
    /// `fanN_label`, when the driver gives a label.
    pub label: Option<String>,
}

/// An hwmon device's identity, relative to the sysfs root: its parent device's path, or its own
/// when it has no parent. `None` for a graphics card's device, whose fans are the card's own.
fn identity(sys: &Path, dir: &Path) -> Option<PathBuf> {
    let parent = dir.join("device").canonicalize().ok();
    if parent.as_deref().is_some_and(is_display) {
        return None;
    }
    let path = match parent {
        Some(parent) => parent,
        None => dir.canonicalize().ok()?,
    };
    path.strip_prefix(sys.canonicalize().ok()?)
        .ok()
        .map(Path::to_path_buf)
}

/// Every hwmon fan input except a graphics card's, by chip, device and number.
pub fn fan_inputs(sys: &Path) -> Vec<FanInput> {
    let mut inputs = Vec::new();
    for (dir, chip) in devices(sys) {
        let Some(device) = identity(sys, &dir) else {
            continue;
        };
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Some(input) = entry.file_name().to_str().and_then(|name| {
                name.strip_prefix("fan")?
                    .strip_suffix("_input")?
                    .parse::<u32>()
                    .ok()
            }) else {
                continue;
            };
            let label = fs::read_to_string(dir.join(format!("fan{input}_label")))
                .ok()
                .map(|label| label.trim().to_owned())
                .filter(|label| !label.is_empty());
            inputs.push(FanInput {
                chip: chip.clone(),
                device: device.clone(),
                input,
                label,
            });
        }
    }
    inputs.sort();
    inputs
}

/// The input's speed in RPM, found again by chip, device and number. When that input isn't there,
/// it's `NoDevice`, never another input's speed (AGENTS.md rule 11).
pub fn fan_rpm(sys: &Path, fan: &FanInput) -> Reading {
    let found = devices(sys)
        .into_iter()
        .find(|(dir, chip)| *chip == fan.chip && identity(sys, dir).as_ref() == Some(&fan.device));
    let Some((dir, _)) = found else {
        return Reading::Unavailable(Unavailable::NoDevice);
    };
    let path = dir.join(format!("fan{}_input", fan.input));
    if !path.exists() {
        return Reading::Unavailable(Unavailable::NoDevice);
    }
    match read_integer::<i64>(&path) {
        Ok(rpm) => Reading::Value(Exact::whole(rpm)),
        Err(reason) => Reading::Unavailable(reason),
    }
}
