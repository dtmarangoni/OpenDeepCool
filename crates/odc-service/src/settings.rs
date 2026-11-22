//! Settings v1 (Phase 7 plan, Task 5; `docs/spec/settings.md`): one TOML file in the service's
//! state directory, written only by the service. The file is checked for form only: a saved GPU
//! may be missing from the PC for now, and it's kept for when it's back (owner, 2026-10-26;
//! solution review OV-14). Fields a later build added are kept and written back
//! unchanged, so moving to an older build and back loses nothing (owner, 2026-10-27; solution
//! review P7-12).

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use odc_render::{Mode, MusicSettings, Stop, Unit};
use odc_sensors::PciAddr;
use serde::{Deserialize, Serialize};

/// The file's name in the state directory.
pub const FILE: &str = "settings.toml";

/// This build's format.
pub const VERSION: u32 = 1;

/// Keys an older build wrote and this one no longer has (Phase 11a): dropped at load.
const RETIRED_KEYS: [&str; 2] = ["cooler_fan_input", "pump_input"];

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

/// A falloff stop as the file writes it: Winamp's five, slowest first (`docs/spec/music.md`
/// point 4). A name that isn't one of them makes the file invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fall {
    Slower,
    Slow,
    Moderate,
    Fast,
    Faster,
}

impl Fall {
    /// The stop called `name`, as `docs/spec/dbus.md`'s setters take it.
    pub fn from_name(name: &str) -> Option<Fall> {
        Stop::from_name(name).map(Fall::from)
    }

    pub fn name(self) -> &'static str {
        Stop::from(self).name()
    }
}

impl From<Fall> for Stop {
    fn from(fall: Fall) -> Stop {
        match fall {
            Fall::Slower => Stop::Slower,
            Fall::Slow => Stop::Slow,
            Fall::Moderate => Stop::Moderate,
            Fall::Fast => Stop::Fast,
            Fall::Faster => Stop::Faster,
        }
    }
}

impl From<Stop> for Fall {
    fn from(stop: Stop) -> Fall {
        match stop {
            Stop::Slower => Fall::Slower,
            Stop::Slow => Fall::Slow,
            Stop::Moderate => Fall::Moderate,
            Stop::Fast => Fall::Fast,
            Stop::Faster => Fall::Faster,
        }
    }
}

fn peaks_on() -> bool {
    true
}

fn bars_moderate() -> Fall {
    Fall::Moderate
}

fn peaks_slow() -> Fall {
    Fall::Slow
}

/// The theme settings' default, both on: follow the system's theme, and OpenDeepCool's own is the
/// dark one (owner, 2026-11-18).
fn theme_on() -> bool {
    true
}

/// How see-through OpenDeepCool's own theme draws the window, in Omarchy's words for its own
/// Window Opacity (Phase 11a Task 49). A word that isn't one of them makes the file invalid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowOpacity {
    #[default]
    Opaque,
    Transparent,
    Glassy,
}

impl WindowOpacity {
    /// The opacity called `name`, as `docs/spec/dbus.md`'s setter takes it.
    pub fn from_name(name: &str) -> Option<WindowOpacity> {
        match name {
            "opaque" => Some(WindowOpacity::Opaque),
            "transparent" => Some(WindowOpacity::Transparent),
            "glassy" => Some(WindowOpacity::Glassy),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            WindowOpacity::Opaque => "opaque",
            WindowOpacity::Transparent => "transparent",
            WindowOpacity::Glassy => "glassy",
        }
    }
}

/// Sharp or rounded corners, in Omarchy's words for its own Window Corners (Phase 11a Task 49).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowCorners {
    Sharp,
    #[default]
    Rounded,
}

impl WindowCorners {
    pub fn from_name(name: &str) -> Option<WindowCorners> {
        match name {
            "sharp" => Some(WindowCorners::Sharp),
            "rounded" => Some(WindowCorners::Rounded),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            WindowCorners::Sharp => "sharp",
            WindowCorners::Rounded => "rounded",
        }
    }
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
    /// The Music settings (`docs/spec/music.md` point 7; Phase 9): a file written before them
    /// gets Winamp's defaults, peak dots on, bars moderate, peaks slow.
    #[serde(default = "peaks_on")]
    pub music_peaks: bool,
    #[serde(default = "bars_moderate")]
    pub music_bar_fall: Fall,
    #[serde(default = "peaks_slow")]
    pub music_peak_fall: Fall,
    /// The theme (`docs/spec/settings.md`; Phase 11a): the app follows the system's, and
    /// OpenDeepCool's own is dark. The service only stores them, for the whole PC; a file
    /// written before them gets both on.
    #[serde(default = "theme_on")]
    pub use_system_theme: bool,
    #[serde(default = "theme_on")]
    pub dark_theme: bool,
    /// OpenDeepCool's own theme's window opacity (default opaque) and corners (default rounded),
    /// stored for the whole PC and applied by the app (Phase 11a Task 49): a file written before
    /// them gets the defaults.
    #[serde(default)]
    pub window_opacity: WindowOpacity,
    #[serde(default)]
    pub window_corners: WindowCorners,
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
            music_peaks: peaks_on(),
            music_bar_fall: bars_moderate(),
            music_peak_fall: peaks_slow(),
            use_system_theme: theme_on(),
            dark_theme: theme_on(),
            window_opacity: WindowOpacity::default(),
            window_corners: WindowCorners::default(),
            later: toml::Table::new(),
        }
    }
}

impl Settings {
    /// Drops the keys a retired feature left in `later` (the cooler's fan and pump inputs, which
    /// the product no longer has; owner, 2026-11-18), so the next save writes none of them. Their
    /// contents are never read, so a file with them out of form still loads.
    fn drop_retired_keys(&mut self) {
        for key in RETIRED_KEYS {
            self.later.remove(key);
        }
    }

    /// The Music settings the spectrum is drawn with.
    pub fn music(&self) -> MusicSettings {
        MusicSettings {
            peaks: self.music_peaks,
            bar_fall: self.music_bar_fall.into(),
            peak_fall: self.music_peak_fall.into(),
        }
    }

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
                Ok(mut settings) => match settings.check() {
                    Ok(()) => {
                        settings.drop_retired_keys();
                        return Ok((settings, Found::Read));
                    }
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
