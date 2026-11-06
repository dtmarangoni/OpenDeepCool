//! The state the service tells its clients, as the `a{sv}` dictionary of `GetState()` and
//! `StateChanged` (`docs/spec/dbus.md`, State). Every key is optional, because `StateChanged`
//! sends only the keys that changed; a client ignores keys it doesn't know, so later phases add
//! keys without breaking it.

use std::collections::HashMap;
use std::fmt;

use zbus::wire::{OwnedVariant, Variant};

/// Every key and its D-Bus signature, as `docs/spec/dbus.md` lists them.
pub const STATE_KEYS: [(&str, &str); 19] = [
    ("Mode", "u"),
    ("Rotation", "y"),
    ("TemperatureUnit", "s"),
    ("CoolerConnected", "b"),
    ("CoolerState", "s"),
    ("AvailableModes", "au"),
    ("UnavailableReasons", "a{us}"),
    ("Gpus", "a(sss)"),
    ("ChosenGpu", "s"),
    ("FanInputs", "a(ssus)"),
    ("CoolerFanInput", "(ssu)"),
    ("PumpInput", "(ssu)"),
    ("CoolerFanRpm", "d"),
    ("PumpRpm", "d"),
    ("MusicPeaks", "b"),
    ("MusicBarFall", "s"),
    ("MusicPeakFall", "s"),
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
}

/// One fan input of `FanInputs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FanInput {
    pub chip: String,
    /// Relative to the sysfs root, `devices/…`, as Phase 6's `FanInput` keeps it.
    pub device: String,
    pub input: u32,
    /// The driver's label, empty when it gives none.
    pub label: String,
}

/// A chosen fan input, `CoolerFanInput` or `PumpInput`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Input {
    pub chip: String,
    pub device: String,
    pub input: u32,
}

/// A fan speed, `CoolerFanRpm` or `PumpRpm`: whole RPM capped at 9999 (owner, 2026-10-25), NaN
/// on the bus when it can't be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FanSpeed {
    Rpm(u16),
    Unavailable,
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
    pub fan_inputs: Option<Vec<FanInput>>,
    /// `None` inside: no input chosen, sent as empty strings and 0.
    pub cooler_fan_input: Option<Option<Input>>,
    pub pump_input: Option<Option<Input>>,
    pub cooler_fan_rpm: Option<FanSpeed>,
    pub pump_rpm: Option<FanSpeed>,
    pub music_peaks: Option<bool>,
    pub music_bar_fall: Option<FallStop>,
    pub music_peak_fall: Option<FallStop>,
    /// Whether the session's sound comes in: a helper is attached.
    pub audio_connected: Option<bool>,
    /// Whether the service takes sound: true exactly while a Music mode, 13–23, is shown.
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

fn owned<'a>(value: impl Into<Variant<'a>>) -> OwnedVariant {
    OwnedVariant::try_from(value.into()).expect("the state holds no file descriptor")
}

fn input_tuple(input: &Option<Input>) -> (String, String, u32) {
    match input {
        Some(i) => (i.chip.clone(), i.device.clone(), i.input),
        None => (String::new(), String::new(), 0),
    }
}

fn speed_number(speed: FanSpeed) -> f64 {
    match speed {
        FanSpeed::Rpm(rpm) => f64::from(rpm),
        FanSpeed::Unavailable => f64::NAN,
    }
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
            let list: Vec<(String, String, String)> = gpus
                .iter()
                .map(|g| (g.pci.clone(), g.driver.clone(), g.kind.as_str().to_owned()))
                .collect();
            put("Gpus", owned(list));
        }
        if let Some(gpu) = &self.chosen_gpu {
            put("ChosenGpu", owned(gpu.as_str()));
        }
        if let Some(inputs) = &self.fan_inputs {
            let list: Vec<(String, String, u32, String)> = inputs
                .iter()
                .map(|i| (i.chip.clone(), i.device.clone(), i.input, i.label.clone()))
                .collect();
            put("FanInputs", owned(list));
        }
        if let Some(input) = &self.cooler_fan_input {
            put("CoolerFanInput", owned(input_tuple(input)));
        }
        if let Some(input) = &self.pump_input {
            put("PumpInput", owned(input_tuple(input)));
        }
        if let Some(speed) = self.cooler_fan_rpm {
            put("CoolerFanRpm", owned(speed_number(speed)));
        }
        if let Some(speed) = self.pump_rpm {
            put("PumpRpm", owned(speed_number(speed)));
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
        fn input(
            dict: &HashMap<String, OwnedVariant>,
            key: &'static str,
        ) -> Result<Option<Option<Input>>, StateError> {
            Ok(
                get::<(String, String, u32)>(dict, key)?.map(|(chip, device, input)| {
                    (!(chip.is_empty() && device.is_empty() && input == 0)).then_some(Input {
                        chip,
                        device,
                        input,
                    })
                }),
            )
        }
        fn speed(
            dict: &HashMap<String, OwnedVariant>,
            key: &'static str,
        ) -> Result<Option<FanSpeed>, StateError> {
            get::<f64>(dict, key)?
                .map(|n| {
                    if n.is_nan() {
                        Ok(FanSpeed::Unavailable)
                    } else if (0.0..=9999.0).contains(&n) && n.fract() == 0.0 {
                        Ok(FanSpeed::Rpm(n as u16))
                    } else {
                        Err(StateError::BadValue(key, n.to_string()))
                    }
                })
                .transpose()
        }
        let rotation = get::<u8>(dict, "Rotation")?;
        if let Some(r) = rotation
            && r > 3
        {
            return Err(StateError::BadValue("Rotation", r.to_string()));
        }
        let gpus = get::<Vec<(String, String, String)>>(dict, "Gpus")?
            .map(|list| {
                list.into_iter()
                    .map(|(pci, driver, kind)| {
                        let kind =
                            GpuKind::parse(&kind).ok_or(StateError::BadValue("Gpus", kind))?;
                        Ok(Gpu { pci, driver, kind })
                    })
                    .collect::<Result<Vec<_>, StateError>>()
            })
            .transpose()?;
        let fan_inputs =
            get::<Vec<(String, String, u32, String)>>(dict, "FanInputs")?.map(|list| {
                list.into_iter()
                    .map(|(chip, device, input, label)| FanInput {
                        chip,
                        device,
                        input,
                        label,
                    })
                    .collect()
            });
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
            fan_inputs,
            cooler_fan_input: input(dict, "CoolerFanInput")?,
            pump_input: input(dict, "PumpInput")?,
            cooler_fan_rpm: speed(dict, "CoolerFanRpm")?,
            pump_rpm: speed(dict, "PumpRpm")?,
            music_peaks: get(dict, "MusicPeaks")?,
            music_bar_fall: text(dict, "MusicBarFall", FallStop::parse)?,
            music_peak_fall: text(dict, "MusicPeakFall", FallStop::parse)?,
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
        take!(
            mode,
            rotation,
            temperature_unit,
            cooler_connected,
            cooler_state,
            available_modes,
            unavailable_reasons,
            gpus,
            chosen_gpu,
            fan_inputs,
            cooler_fan_input,
            pump_input,
            cooler_fan_rpm,
            pump_rpm,
            music_peaks,
            music_bar_fall,
            music_peak_fall,
            audio_connected,
            audio_wanted
        );
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
        keep!(
            mode,
            rotation,
            temperature_unit,
            cooler_connected,
            cooler_state,
            available_modes,
            unavailable_reasons,
            gpus,
            chosen_gpu,
            fan_inputs,
            cooler_fan_input,
            pump_input,
            cooler_fan_rpm,
            pump_rpm,
            music_peaks,
            music_bar_fall,
            music_peak_fall,
            audio_connected,
            audio_wanted
        );
        changes
    }

    /// Whether no key is present.
    pub fn is_empty(&self) -> bool {
        *self == State::default()
    }
}
