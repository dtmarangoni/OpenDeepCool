//! Settings v1 (Phase 7 plan, Task 5; `docs/spec/settings.md`): one TOML file in the service's
//! state directory, written only by the service. The file is checked for form only: a saved GPU
//! or fan input may be missing from the PC for now, and it's kept for when it's back (owner,
//! 2026-10-26; solution review OV-14). Fields a later build added are kept and written back
//! unchanged, so moving to an older build and back loses nothing (owner, 2026-10-27; solution
//! review P7-12).

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use odc_render::{Mode, Unit};
use odc_sensors::PciAddr;
use serde::{Deserialize, Serialize};

/// The file's name in the state directory.
pub const FILE: &str = "settings.toml";

/// This build's format.
pub const VERSION: u32 = 1;

/// How many invalid files are kept, renamed aside (solution review P7-12).
const KEPT_INVALID: usize = 3;

/// A settings file larger than this is refused unread.
const MAX_FILE: u64 = 64 * 1024;

/// The temperature unit as the file writes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

impl From<TemperatureUnit> for Unit {
    fn from(unit: TemperatureUnit) -> Unit {
        match unit {
            TemperatureUnit::Celsius => Unit::Celsius,
            TemperatureUnit::Fahrenheit => Unit::Fahrenheit,
        }
    }
}

/// A chosen fan input: its chip's name, its device path relative to the sysfs root, and its
/// number, as Phase 6's `FanInput` identifies it, never by hwmon index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedInput {
    pub chip: String,
    pub device_path: String,
    pub input: u32,
}

/// The settings (`docs/spec/settings.md`; the defaults are OpenDeepCool's, the same as
/// DeepCreative's first start).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    pub mode: u16,
    pub rotation: u8,
    pub temperature_unit: TemperatureUnit,
    /// The PCI address of the GPU the user chose; absent for Automatic, the first in the ranking.
    /// Cleared at start when the GPUs changed (owner, 2026-11-04).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_choice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooler_fan_input: Option<SavedInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pump_input: Option<SavedInput>,
    /// The fields a later build added, as they were.
    #[serde(flatten)]
    pub later: toml::Table,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            version: VERSION,
            mode: 1,
            rotation: 0,
            temperature_unit: TemperatureUnit::Celsius,
            gpu_choice: None,
            cooler_fan_input: None,
            pump_input: None,
            later: toml::Table::new(),
        }
    }
}

impl Settings {
    /// Whether the settings have the form this build accepts.
    ///
    /// # Errors
    ///
    /// The first field out of form, and why.
    pub fn check(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err(format!("version {}, not {VERSION}", self.version));
        }
        if Mode::new(self.mode).is_none() {
            return Err(format!("mode {} isn't in the mode table", self.mode));
        }
        if self.rotation > 3 {
            return Err(format!("rotation {} isn't 0 to 3", self.rotation));
        }
        if let Some(gpu) = &self.gpu_choice {
            check_pci(gpu).map_err(|why| format!("gpu_choice: {why}"))?;
        }
        for (name, input) in [
            ("cooler_fan_input", &self.cooler_fan_input),
            ("pump_input", &self.pump_input),
        ] {
            if let Some(input) = input {
                check_input(input).map_err(|why| format!("{name}: {why}"))?;
            }
        }
        Ok(())
    }
}

/// A PCI address in canonical form, `dddd:bb:dd.f` in lower-case hex.
pub fn check_pci(text: &str) -> Result<PciAddr, String> {
    let address = PciAddr::from_str(text)?;
    if address.to_string() == text {
        Ok(address)
    } else {
        Err(format!("{text:?} isn't written as {address}"))
    }
}

/// The chip name at most 64 bytes; the device path relative to the sysfs root, `devices/…`, of
/// normal components only and at most 1024 bytes; the input numbered from 1.
pub fn check_input(input: &SavedInput) -> Result<(), String> {
    if input.chip.is_empty() || input.chip.len() > 64 {
        return Err(format!("a chip name of {} bytes", input.chip.len()));
    }
    check_device_path(&input.device_path)?;
    if input.input == 0 {
        return Err("input 0; inputs are numbered from 1".into());
    }
    Ok(())
}

/// A device path as Phase 6's `FanInput` keeps it: relative to the sysfs root, under `devices/`,
/// with no `/` at the start, no empty, `.` or `..` component, at most 1024 bytes.
pub fn check_device_path(path: &str) -> Result<(), String> {
    if path.len() > 1024 {
        return Err(format!("a device path of {} bytes", path.len()));
    }
    let mut components = path.split('/');
    let normal = path
        .split('/')
        .all(|c| !c.is_empty() && c != "." && c != "..");
    if components.next() != Some("devices") || components.next().is_none() || !normal {
        return Err(format!("{path:?} isn't a device path under devices/"));
    }
    Ok(())
}

/// What loading found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// The file, read and in form.
    Read,
    /// No file: the defaults, now written.
    Created,
    /// A file that wasn't settings this build accepts: moved aside, and the defaults written.
    Replaced { moved_to: PathBuf, why: String },
}

/// The settings file in a directory, read and written only by the service.
pub struct SettingsStore {
    dir: PathBuf,
}

impl SettingsStore {
    pub fn new(dir: impl Into<PathBuf>) -> SettingsStore {
        SettingsStore { dir: dir.into() }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(FILE)
    }

    /// The saved settings. A missing file gives the defaults, which are written; a file that
    /// can't be read or isn't in form is renamed to `settings.toml.invalid-<unix time>`, the
    /// last three such copies kept, and the defaults are written in its place.
    ///
    /// # Errors
    ///
    /// When the directory can't be written.
    pub fn load(&self) -> io::Result<(Settings, Found)> {
        let path = self.path();
        let why = match read(&path) {
            Ok(None) => {
                let settings = Settings::default();
                self.store(&settings)?;
                return Ok((settings, Found::Created));
            }
            Ok(Some(text)) => match toml::from_str::<Settings>(&text) {
                Ok(settings) => match settings.check() {
                    Ok(()) => return Ok((settings, Found::Read)),
                    Err(why) => why,
                },
                Err(e) => e.message().to_owned(),
            },
            Err(why) => why,
        };
        let moved_to = self.move_aside()?;
        let settings = Settings::default();
        self.store(&settings)?;
        Ok((settings, Found::Replaced { moved_to, why }))
    }

    /// Writes `settings`: a temporary file, synced, renamed over the file, then the directory
    /// synced, so the rename itself survives a power cut (solution review P7-12).
    ///
    /// # Errors
    ///
    /// When the settings aren't in form, or a write fails; the file stays as it was.
    pub fn store(&self, settings: &Settings) -> io::Result<()> {
        settings
            .check()
            .map_err(|why| io::Error::new(ErrorKind::InvalidInput, why))?;
        let text = toml::to_string(settings)
            .map_err(|e| io::Error::new(ErrorKind::InvalidData, e.to_string()))?;
        let temp = self.dir.join(format!("{FILE}.new"));
        match fs::remove_file(&temp) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, self.path())?;
        File::open(&self.dir)?.sync_all()
    }

    /// Renames the file aside, then removes all but the last three such copies.
    fn move_aside(&self) -> io::Result<PathBuf> {
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut moved_to = self.dir.join(format!("{FILE}.invalid-{unix}"));
        let mut n = 1;
        while fs::symlink_metadata(&moved_to).is_ok() {
            moved_to = self.dir.join(format!("{FILE}.invalid-{unix}-{n}"));
            n += 1;
        }
        fs::rename(self.path(), &moved_to)?;
        let mut copies: Vec<(u64, u64, PathBuf)> = fs::read_dir(&self.dir)?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let stamp = name.strip_prefix(&format!("{FILE}.invalid-"))?;
                let (time, n) = stamp.split_once('-').unwrap_or((stamp, "0"));
                Some((time.parse().ok()?, n.parse().ok()?, entry.path()))
            })
            .collect();
        copies.sort();
        let excess = copies.len().saturating_sub(KEPT_INVALID);
        for (_, _, old) in copies.into_iter().take(excess) {
            fs::remove_file(old)?;
        }
        Ok(moved_to)
    }
}

/// The file's text, `None` when there's none. A link, anything but a plain file, a file too big
/// or one that isn't UTF-8 is refused.
fn read(path: &Path) -> Result<Option<String>, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file() {
        return Err("not a plain file".into());
    }
    if meta.len() > MAX_FILE {
        return Err(format!("{} bytes, more than {MAX_FILE}", meta.len()));
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|e| e.to_string())
}
