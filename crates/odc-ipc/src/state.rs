//! The state the service tells its clients, as the `a{sv}` dictionary of `GetState()` and
//! `StateChanged` (`docs/spec/dbus.md`, State). Every key is optional, because `StateChanged`
//! sends only the keys that changed; a client ignores keys it doesn't know, so later phases add
//! keys without breaking it.

use std::collections::HashMap;
use std::fmt;

use zbus::wire::{OwnedVariant, Variant};

/// Every key and its D-Bus signature, as `docs/spec/dbus.md` lists them.
pub const STATE_KEYS: [(&str, &str); 20] = [
    ("Mode", "u"),
    ("Rotation", "y"),
    ("TemperatureUnit", "s"),
    ("CoolerConnected", "b"),
    ("CoolerState", "s"),
    ("AvailableModes", "au"),
    ("UnavailableReasons", "a{us}"),
    ("Gpus", "a(sssqqb)"),
    ("ChosenGpu", "s"),
    ("GpuInUse", "s"),
    ("MusicPeaks", "b"),
    ("MusicBarFall", "s"),
    ("MusicPeakFall", "s"),
    ("Readings", "a{s(xt)}"),
    ("UseSystemTheme", "b"),
    ("DarkTheme", "b"),
    ("WindowOpacity", "s"),
    ("WindowCorners", "s"),
    ("AudioConnected", "b"),
    ("AudioWanted", "b"),
];

/// A value the state's text keys take, by its text on the bus.
macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }

            pub fn parse(text: &str) -> Option<$name> {
                match text {
                    $($text => Some($name::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

text_enum! {
    /// `TemperatureUnit`.
    TemperatureUnit { Celsius => "celsius", Fahrenheit => "fahrenheit" }
}

text_enum! {
    /// `MusicBarFall` and `MusicPeakFall`: Winamp's five falloff stops, slowest first
    /// (`docs/spec/music.md` point 4).
    FallStop {
        Slower => "slower",
        Slow => "slow",
        Moderate => "moderate",
        Fast => "fast",
        Faster => "faster",
    }
}

text_enum! {
    /// `WindowOpacity`: how see-through OpenDeepCool's own theme draws the window, in Omarchy's
    /// words for its own Window Opacity (Phase 11a Task 49).
    WindowOpacity {
        Opaque => "opaque",
        Transparent => "transparent",
        Glassy => "glassy",
    }
}

text_enum! {
    /// `WindowCorners`: sharp or rounded corners of the window and what is in it, in Omarchy's
    /// words for its own Window Corners (Phase 11a Task 49).
    WindowCorners { Sharp => "sharp", Rounded => "rounded" }
}

text_enum! {
    /// `CoolerState`: from Phase 5's link and its reason (owner, 2026-10-27; solution review
    /// P5-02); `released` only on the dev copy, while a test holds the cooler (owner,
    /// 2026-10-29).
    CoolerState {
        Connected => "connected",
        NotFound => "not-found",
        NoAccess => "no-access",
        WriteFailed => "write-failed",
        Released => "released",
    }
}

text_enum! {
    /// A GPU's kind in `Gpus`; `query-failed` is ranked as unknown (owner, 2026-10-27; solution
    /// review P6-07).
    GpuKind {
        Dedicated => "dedicated",
        Integrated => "integrated",
        Unknown => "unknown",
        QueryFailed => "query-failed",
    }
}

/// One GPU of `Gpus`, in rank order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gpu {
    /// Its PCI address, `dddd:bb:dd.f`.
    pub pci: String,
    pub driver: String,
    pub kind: GpuKind,
    /// Its PCI vendor and device ID, for the system's PCI ID list; 0 when sysfs doesn't give
    /// one (Phase 10).
    pub vendor: u16,
    pub device: u16,
    /// Whether its driver has a reading rule (`docs/spec/sensors.md`); without one, no GPU
    /// metric can be read from it.
    pub has_rule: bool,
}

/// The state, each key present or not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub mode: Option<u32>,
    pub rotation: Option<u8>,
    pub temperature_unit: Option<TemperatureUnit>,
    pub cooler_connected: Option<bool>,
    pub cooler_state: Option<CoolerState>,
    pub available_modes: Option<Vec<u32>>,
    /// Mode → its one-line reason.
    pub unavailable_reasons: Option<HashMap<u32, String>>,
    pub gpus: Option<Vec<Gpu>>,
    /// Empty when the user chose none.
    pub chosen_gpu: Option<String>,
    /// The GPU the readings come from: the chosen one, or Automatic's; empty with no GPU.
    pub gpu_in_use: Option<String>,
    pub music_peaks: Option<bool>,
    pub music_bar_fall: Option<FallStop>,
    pub music_peak_fall: Option<FallStop>,
    /// Each metric that has a value now, by its spec name, as the exact fraction the display
    /// rounds from: numerator and denominator (Phase 11a Task 13). A metric with no value is
    /// left out. `StateChanged` sends it only to a watcher that also watches frames.
    pub readings: Option<HashMap<String, (i64, u64)>>,
    /// Whether the app follows the system's theme, and whether OpenDeepCool's own is the dark one:
    /// two settings of the whole PC the service only stores (Phase 11a).
    pub use_system_theme: Option<bool>,
    pub dark_theme: Option<bool>,
    /// OpenDeepCool's own theme's window opacity and corners, for the whole PC (Phase 11a
    /// Task 49): stored only, applied by the app.
    pub window_opacity: Option<WindowOpacity>,
    pub window_corners: Option<WindowCorners>,
    /// Whether the session's sound comes in: a helper is attached.
    pub audio_connected: Option<bool>,
    /// Whether the service takes sound: true exactly while a Music mode, 13–16 or 18–26, is shown.
    pub audio_wanted: Option<bool>,
}

/// Why a dictionary isn't a state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    /// A known key whose value has another type.
    WrongType(&'static str),
    /// A known key whose value its type allows, but the contract doesn't.
    BadValue(&'static str, String),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::WrongType(key) => write!(f, "{key} doesn't have the contract's type"),
            StateError::BadValue(key, value) => write!(f, "{key} can't be {value}"),
        }
    }
}

impl std::error::Error for StateError {}

/// Calls the macro `$each` with every field of [`State`], once, for the code that treats them all
/// alike ([`State::merge`] and [`State::changed_since`]).
macro_rules! every_field {
    ($each:ident) => {
        $each!(
            mode,
            rotation,
            temperature_unit,
            cooler_connected,
            cooler_state,
            available_modes,
            unavailable_reasons,
            gpus,
            chosen_gpu,
            gpu_in_use,
            music_peaks,
            music_bar_fall,
            music_peak_fall,
            readings,
            use_system_theme,
            dark_theme,
            window_opacity,
            window_corners,
            audio_connected,
            audio_wanted
        )
    };
}

/// A new field of [`State`] fails to compile here, so that every list that has to know it is
/// revisited: `STATE_KEYS`, [`State::to_dict`], [`State::from_dict`] (its literal has no `..`) and
/// `every_field!` above, and `docs/spec/dbus.md`, which the contract tests check against
/// `STATE_KEYS`.
const _: fn(State) = |State {
                          mode: _,
                          rotation: _,
                          temperature_unit: _,
                          cooler_connected: _,
                          cooler_state: _,
                          available_modes: _,
                          unavailable_reasons: _,
                          gpus: _,
                          chosen_gpu: _,
                          gpu_in_use: _,
                          music_peaks: _,
                          music_bar_fall: _,
                          music_peak_fall: _,
                          readings: _,
                          use_system_theme: _,
                          dark_theme: _,
                          window_opacity: _,
                          window_corners: _,
                          audio_connected: _,
                          audio_wanted: _,
                      }| {};

fn owned<'a>(value: impl Into<Variant<'a>>) -> OwnedVariant {
    OwnedVariant::try_from(value.into()).expect("the state holds no file descriptor")
}

impl State {
    /// The keys present, as the service sends them.
    pub fn to_dict(&self) -> HashMap<String, OwnedVariant> {
        let mut dict = HashMap::new();
        let mut put = |key: &str, value: OwnedVariant| {
            dict.insert(key.to_owned(), value);
        };
        if let Some(mode) = self.mode {
            put("Mode", owned(mode));
        }
        if let Some(rotation) = self.rotation {
            put("Rotation", owned(rotation));
        }
        if let Some(unit) = self.temperature_unit {
            put("TemperatureUnit", owned(unit.as_str()));
        }
        if let Some(connected) = self.cooler_connected {
            put("CoolerConnected", owned(connected));
        }
        if let Some(state) = self.cooler_state {
            put("CoolerState", owned(state.as_str()));
        }
        if let Some(modes) = &self.available_modes {
            put("AvailableModes", owned(modes.clone()));
        }
        if let Some(reasons) = &self.unavailable_reasons {
            put("UnavailableReasons", owned(reasons.clone()));
        }
        if let Some(gpus) = &self.gpus {
            let list: Vec<(String, String, String, u16, u16, bool)> = gpus
                .iter()
                .map(|g| {
                    (
                        g.pci.clone(),
                        g.driver.clone(),
                        g.kind.as_str().to_owned(),
                        g.vendor,
                        g.device,
                        g.has_rule,
                    )
                })
                .collect();
            put("Gpus", owned(list));
        }
        if let Some(gpu) = &self.chosen_gpu {
            put("ChosenGpu", owned(gpu.as_str()));
        }
        if let Some(gpu) = &self.gpu_in_use {
            put("GpuInUse", owned(gpu.as_str()));
        }
        if let Some(peaks) = self.music_peaks {
            put("MusicPeaks", owned(peaks));
        }
        if let Some(stop) = self.music_bar_fall {
            put("MusicBarFall", owned(stop.as_str()));
        }
        if let Some(stop) = self.music_peak_fall {
            put("MusicPeakFall", owned(stop.as_str()));
        }
        if let Some(readings) = &self.readings {
            put("Readings", owned(readings.clone()));
        }
        if let Some(system) = self.use_system_theme {
            put("UseSystemTheme", owned(system));
        }
        if let Some(dark) = self.dark_theme {
            put("DarkTheme", owned(dark));
        }
        if let Some(opacity) = self.window_opacity {
            put("WindowOpacity", owned(opacity.as_str()));
        }
        if let Some(corners) = self.window_corners {
            put("WindowCorners", owned(corners.as_str()));
        }
        if let Some(connected) = self.audio_connected {
            put("AudioConnected", owned(connected));
        }
        if let Some(wanted) = self.audio_wanted {
            put("AudioWanted", owned(wanted));
        }
        dict
    }

    /// The state in `dict`: every known key it holds, read strictly; unknown keys are ignored.
    ///
    /// # Errors
    ///
    /// When a known key has another type, or a value the contract doesn't allow.
    pub fn from_dict(dict: &HashMap<String, OwnedVariant>) -> Result<State, StateError> {
        fn get<T: TryFrom<OwnedVariant>>(
            dict: &HashMap<String, OwnedVariant>,
            key: &'static str,
        ) -> Result<Option<T>, StateError> {
            let Some(value) = dict.get(key) else {
                return Ok(None);
            };
            // Checked first: zbus 6 panics converting a structure with fewer fields than asked,
            // such as a structure key from another version of the service.
            let signature = STATE_KEYS
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, signature)| *signature);
            if signature != Some(value.value_signature().to_string().as_str()) {
                return Err(StateError::WrongType(key));
            }
            let value = value.try_clone().map_err(|_| StateError::WrongType(key))?;
            T::try_from(value)
                .map(Some)
                .map_err(|_| StateError::WrongType(key))
        }
        fn text<T>(
            dict: &HashMap<String, OwnedVariant>,
            key: &'static str,
            parse: fn(&str) -> Option<T>,
        ) -> Result<Option<T>, StateError> {
            get::<String>(dict, key)?
                .map(|t| parse(&t).ok_or(StateError::BadValue(key, t)))
                .transpose()
        }
        let rotation = get::<u8>(dict, "Rotation")?;
        if let Some(r) = rotation
            && r > 3
        {
            return Err(StateError::BadValue("Rotation", r.to_string()));
        }
        let gpus = get::<Vec<(String, String, String, u16, u16, bool)>>(dict, "Gpus")?
            .map(|list| {
                list.into_iter()
                    .map(|(pci, driver, kind, vendor, device, has_rule)| {
                        let kind =
                            GpuKind::parse(&kind).ok_or(StateError::BadValue("Gpus", kind))?;
                        Ok(Gpu {
                            pci,
                            driver,
                            kind,
                            vendor,
                            device,
                            has_rule,
                        })
                    })
                    .collect::<Result<Vec<_>, StateError>>()
            })
            .transpose()?;
        let readings = get::<HashMap<String, (i64, u64)>>(dict, "Readings")?;
        if let Some((name, (num, _))) = readings.iter().flatten().find(|(_, (_, den))| *den == 0) {
            return Err(StateError::BadValue(
                "Readings",
                format!("{name}: {num} over 0"),
            ));
        }
        Ok(State {
            mode: get(dict, "Mode")?,
            rotation,
            temperature_unit: text(dict, "TemperatureUnit", TemperatureUnit::parse)?,
            cooler_connected: get(dict, "CoolerConnected")?,
            cooler_state: text(dict, "CoolerState", CoolerState::parse)?,
            available_modes: get(dict, "AvailableModes")?,
            unavailable_reasons: get(dict, "UnavailableReasons")?,
            gpus,
            chosen_gpu: get(dict, "ChosenGpu")?,
            gpu_in_use: get(dict, "GpuInUse")?,
            music_peaks: get(dict, "MusicPeaks")?,
            music_bar_fall: text(dict, "MusicBarFall", FallStop::parse)?,
            music_peak_fall: text(dict, "MusicPeakFall", FallStop::parse)?,
            readings,
            use_system_theme: get(dict, "UseSystemTheme")?,
            dark_theme: get(dict, "DarkTheme")?,
            window_opacity: text(dict, "WindowOpacity", WindowOpacity::parse)?,
            window_corners: text(dict, "WindowCorners", WindowCorners::parse)?,
            audio_connected: get(dict, "AudioConnected")?,
            audio_wanted: get(dict, "AudioWanted")?,
        })
    }

    /// Takes every key `changes` holds, as a client does with `StateChanged`.
    pub fn merge(&mut self, changes: State) {
        macro_rules! take {
            ($($field:ident),+) => {
                $(if changes.$field.is_some() {
                    self.$field = changes.$field;
                })+
            };
        }
        every_field!(take);
    }

    /// The keys whose value differs from `older`'s, as the service sends them in `StateChanged`;
    /// merging the result into `older` gives `self` back.
    pub fn changed_since(&self, older: &State) -> State {
        let mut changes = State::default();
        macro_rules! keep {
            ($($field:ident),+) => {
                $(if self.$field != older.$field {
                    changes.$field = self.$field.clone();
                })+
            };
        }
        every_field!(keep);
        changes
    }

    /// Whether no key is present.
    pub fn is_empty(&self) -> bool {
        *self == State::default()
    }
}
