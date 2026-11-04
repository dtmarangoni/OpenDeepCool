//! OpenDeepCool's D-Bus contract (Phase 7 plan, Task 4; `docs/spec/dbus.md`): the names, the
//! state the service tells its clients, the methods, the errors, and the client proxies the
//! tests and the settings app (Phase 10) use. Nothing is broadcast: the interface has no
//! properties, and `StateChanged` is addressed to each watcher alone (owner, 2026-10-26).

mod state;

pub use state::{
    CoolerState, FanInput, FanSpeed, Gpu, GpuKind, Input, STATE_KEYS, State, StateError,
    TemperatureUnit,
};

use std::collections::HashMap;

use zbus::wire::OwnedVariant;

/// The service's well-known bus name, and its main interface's name. The `1` is the interface's
/// major version (owner, 2026-10-24): an incompatible change would add `…OpenDeepCool2`.
pub const BUS_NAME: &str = "io.github.dtmarangoni.OpenDeepCool1";
pub const INTERFACE: &str = BUS_NAME;
pub const OBJECT_PATH: &str = "/io/github/dtmarangoni/OpenDeepCool1";

/// The dev interface, registered only when the service starts with `--dev`, at its own path
/// (owner, 2026-10-26; solution review OV-13).
pub const DEV_INTERFACE: &str = "io.github.dtmarangoni.OpenDeepCool1.Dev";
pub const DEV_PATH: &str = "/io/github/dtmarangoni/OpenDeepCool1/Dev";

/// The polkit actions: `use` for every read, `change-settings` for every change, `Refresh()`
/// included; both for the active local session only (owner, 2026-10-26).
pub const USE_ACTION: &str = "io.github.dtmarangoni.OpenDeepCool1.use";
pub const CHANGE_SETTINGS_ACTION: &str = "io.github.dtmarangoni.OpenDeepCool1.change-settings";

/// The prefix of the service's error names.
pub const ERROR_PREFIX: &str = "io.github.dtmarangoni.OpenDeepCool1.Error";

/// The errors the service replies with, named `<ERROR_PREFIX>.<variant>`; each carries a
/// one-line text a person can read.
#[derive(zbus::DBusError, Debug)]
#[zbus(prefix = "io.github.dtmarangoni.OpenDeepCool1.Error")]
pub enum Error {
    #[zbus(error)]
    ZBus(zbus::Error),
    /// polkit refused the caller: it isn't the active local session.
    NotAuthorized(String),
    /// The mode's metrics can't be read on this PC; the text says why.
    ModeUnavailable(String),
    /// A mode a later phase draws.
    NotYetAvailable(String),
    /// An argument outside what the service found or allows.
    InvalidArgument(String),
    /// The caller's user, or everyone together, already watches as often as allowed.
    TooManyWatchers(String),
    /// The dev copy only: a test session holds the service.
    Busy(String),
}

/// The main interface, for clients: the settings app (Phase 10), the tests, and `busctl`.
#[zbus::proxy(
    interface = "io.github.dtmarangoni.OpenDeepCool1",
    default_service = "io.github.dtmarangoni.OpenDeepCool1",
    default_path = "/io/github/dtmarangoni/OpenDeepCool1"
)]
pub trait OpenDeepCool {
    /// Switches to `mode`.
    async fn set_mode(&self, mode: u32) -> Result<(), Error>;
    /// One press of Rotate Screen: a quarter turn clockwise, 3 wrapping to 0.
    async fn rotate_screen(&self) -> Result<(), Error>;
    async fn set_rotation(&self, rotation: u8) -> Result<(), Error>;
    /// `celsius` or `fahrenheit`.
    async fn set_temperature_unit(&self, unit: &str) -> Result<(), Error>;
    /// A PCI address listed in `Gpus`.
    async fn choose_gpu(&self, pci: &str) -> Result<(), Error>;
    /// A fan input listed in `FanInputs`, or all empty for none.
    async fn set_cooler_fan_input(&self, chip: &str, device: &str, input: u32)
    -> Result<(), Error>;
    async fn set_pump_input(&self, chip: &str, device: &str, input: u32) -> Result<(), Error>;
    /// Fresh readings at once, and the picture from home.
    async fn refresh(&self) -> Result<(), Error>;
    /// The whole state, as `a{sv}`; [`State::from_dict`] reads it.
    async fn get_state(&self) -> Result<HashMap<String, OwnedVariant>, Error>;
    /// Sends this caller `StateChanged` while it watches.
    async fn watch(&self) -> Result<(), Error>;
    async fn unwatch(&self) -> Result<(), Error>;
    /// Each metric, its bound source, and its state.
    async fn diagnostics(&self) -> Result<Vec<(String, String, String)>, Error>;
    /// The keys that changed, or the whole state after a watcher passes its check again.
    #[zbus(signal)]
    fn state_changed(&self, changes: HashMap<String, OwnedVariant>) -> zbus::Result<()>;
}

/// The dev interface (only with `--dev`): the readings override and the test session.
#[zbus::proxy(
    interface = "io.github.dtmarangoni.OpenDeepCool1.Dev",
    default_service = "io.github.dtmarangoni.OpenDeepCool1",
    default_path = "/io/github/dtmarangoni/OpenDeepCool1/Dev"
)]
pub trait Dev {
    /// Exact values (metric → numerator and denominator) and unavailable metrics with their
    /// reasons, from the next tick on; returns that tick's number. Needs the test session.
    async fn override_readings(
        &self,
        values: HashMap<String, (i64, u64)>,
        unavailable: HashMap<String, String>,
    ) -> Result<u64, Error>;
    async fn clear_override(&self) -> Result<(), Error>;
    /// Holds the test session: other callers' changes get `Busy`, and the frame recording runs.
    async fn begin_test(&self) -> Result<(), Error>;
    async fn end_test(&self) -> Result<(), Error>;
    /// Hands the cooler over until the session ends (owner, 2026-10-29).
    async fn release_cooler(&self) -> Result<(), Error>;
    /// Each frame write's and each snapshot's duration in µs since the session began, the
    /// newest 100 000 of each (owner, 2026-11-04). Needs the test session.
    async fn durations(&self) -> Result<(Vec<u64>, Vec<u64>), Error>;
}
