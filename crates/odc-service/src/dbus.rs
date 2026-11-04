//! The D-Bus side (Phase 7 plan, Task 7; `docs/spec/dbus.md`): the main interface, the dev
//! interface when the service starts with `--dev`, the watchers and their check each second, and
//! the journal lines for the calls it refuses. Every call is checked with polkit; arguments are
//! only compared with what the service found itself, and no file path is ever built from one
//! (owner, 2026-10-26; solution review OV-14). The D-Bus side keeps the saved settings and meets
//! the other threads only through the `Apply` mailbox, the device thread's control channel, the
//! status and the pulse, so a slow polkit check never delays a frame.

use std::collections::HashMap;
use std::future::{Future, poll_fn};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard};
use std::task::Poll;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use odc_device::{Link, Reason, RecordingHandle};
use odc_ipc::{BUS_NAME, CoolerState, DEV_PATH, Error, FanSpeed, Input, OBJECT_PATH, State};
use odc_render::{Mode, Shown, draws, number};
use odc_sensors::{Binding, Exact, Fan, Metric, Reading, Snapshot, Unavailable};
use zbus::fdo::DBusProxy;
use zbus::message::{Header, Type};
use zbus::names::BusName;
use zbus::object_server::SignalEmitter;
use zbus::wire::OwnedVariant;
use zbus::{Connection, DBusError, MatchRule, MessageStream, connection};

use crate::engine::{Apply, Override, shown_mode};
use crate::log::{Journal, Priority};
use crate::polkit::{Action, Authorizer, BusCredentials, Credentials, Identity, PolkitAuthorizer};
use crate::pulse::Pulse;
use crate::runtime::{DeviceControl, Durations, Slot};
use crate::settings::{SavedInput, Settings, SettingsStore, TemperatureUnit};
use crate::sleep::{SleepWatch, next};
use crate::status::Status;
use crate::watchers::{Checked, Watchers};

/// Each watcher is checked again this often, on the D-Bus side's own timer.
const CHECK_EVERY: Duration = Duration::from_secs(1);

/// How long the start waits for the bus.
const START_TIMEOUT: Duration = Duration::from_secs(30);

/// A string argument longer than this, in bytes, is refused before any lookup; a device path may
/// have up to [`PATH_LIMIT`] (owner, 2026-10-26; solution review OV-14).
const STRING_LIMIT: usize = 64;
const PATH_LIMIT: usize = 1024;

/// What `Busy` says.
const BUSY: &str = "a test is running on this dev copy";

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The dev copy's part (`docs/spec/dbus.md`, Dev interface).
struct DevSide {
    /// The group whose members may use the dev interface: the service's own.
    group: u32,
    recording: RecordingHandle,
    /// The test session's holder, by unique bus name.
    session: Mutex<Option<String>>,
    /// Whether the session asked for the cooler, so its end gives it back.
    released: AtomicBool,
    /// Each frame write's and each snapshot's, cleared when a session begins.
    durations: Arc<Durations>,
}

impl DevSide {
    fn new(group: u32, recording: RecordingHandle, durations: Arc<Durations>) -> DevSide {
        DevSide {
            group,
            recording,
            session: Mutex::new(None),
            released: AtomicBool::new(false),
            durations,
        }
    }
}

/// What both interfaces share.
struct Shared {
    store: SettingsStore,
    /// The saved settings: the D-Bus side is the only writer.
    settings: Mutex<Settings>,
    status: Arc<RwLock<Status>>,
    apply: Arc<Slot<Apply>>,
    device: Sender<DeviceControl>,
    authorizer: Box<dyn Authorizer>,
    credentials: Box<dyn Credentials>,
    dbus: DBusProxy<'static>,
    watchers: Mutex<Watchers>,
    pulse: Arc<Pulse>,
    journal: Arc<dyn Journal>,
    dev: Option<DevSide>,
    /// The last readings override's number.
    overrides: AtomicU64,
}

/// The caller's unique name. The bus sets it on every message; a message without one is refused.
fn sender(header: &Header<'_>) -> Result<String, Error> {
    header
        .sender()
        .map(|s| s.as_str().to_owned())
        .ok_or_else(|| Error::NotAuthorized("a call with no sender".into()))
}

fn invalid(text: impl Into<String>) -> Error {
    Error::InvalidArgument(text.into())
}

/// Refuses a string longer than `limit` bytes before any lookup.
fn within(what: &str, text: &str, limit: usize) -> Result<(), Error> {
    if text.len() > limit {
        return Err(invalid(format!(
            "{what} of {} bytes, more than {limit}",
            text.len()
        )));
    }
    Ok(())
}

impl Shared {
    /// Writes the refusal of `call` from `sender` to the journal, with its reason, and returns it.
    fn refused(&self, call: &str, sender: &str, error: Error) -> Error {
        let why = error.description().unwrap_or("refused").to_owned();
        self.journal.write(
            Priority::Notice,
            &format!("{call} from {sender} refused: {why}"),
        );
        error
    }

    fn logged<T>(&self, call: &str, sender: &str, result: Result<T, Error>) -> Result<T, Error> {
        result.map_err(|e| self.refused(call, sender, e))
    }

    /// polkit's answer for `action`. A check that fails is a refusal, with a journal line.
    async fn check(&self, sender: &str, action: Action, call: &str) -> Result<(), Error> {
        match self.authorizer.allowed(sender, action).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(self.refused(
                call,
                sender,
                Error::NotAuthorized(format!("polkit doesn't allow {}", action.id())),
            )),
            Err(why) => {
                self.journal.write(
                    Priority::Warning,
                    &format!("{call} from {sender}: the polkit check failed: {why}"),
                );
                Err(Error::NotAuthorized(format!(
                    "the polkit check failed: {why}"
                )))
            }
        }
    }

    /// A read's caller: polkit's `use`.
    async fn reader(&self, header: &Header<'_>, call: &str) -> Result<String, Error> {
        let sender = sender(header)?;
        self.check(&sender, Action::Use, call).await?;
        Ok(sender)
    }

    /// A change's caller: polkit's `change-settings`, then no test session held by someone else.
    async fn changer(&self, header: &Header<'_>, call: &str) -> Result<String, Error> {
        let sender = sender(header)?;
        self.check(&sender, Action::ChangeSettings, call).await?;
        if let Some(dev) = &self.dev
            && lock(&dev.session).as_ref().is_some_and(|h| *h != sender)
        {
            return Err(self.refused(call, &sender, Error::Busy(BUSY.into())));
        }
        Ok(sender)
    }

    /// A dev call's caller: in the service's group, as the bus reports it, a missing list meaning
    /// no; then polkit's `change-settings` (owner, 2026-10-26; solution review OV-13).
    async fn dev_caller(
        &self,
        header: &Header<'_>,
        call: &str,
    ) -> Result<(String, &DevSide), Error> {
        let dev = self
            .dev
            .as_ref()
            .ok_or_else(|| Error::NotAuthorized("not a dev copy".into()))?;
        let sender = sender(header)?;
        let in_group = matches!(
            self.credentials.of(&sender).await,
            Ok(Identity { groups: Some(groups), .. }) if groups.contains(&dev.group)
        );
        if !in_group {
            return Err(self.refused(
                call,
                &sender,
                Error::NotAuthorized("only the service's group may use the dev interface".into()),
            ));
        }
        self.check(&sender, Action::ChangeSettings, call).await?;
        Ok((sender, dev))
    }

    /// Whether `sender` holds the test session.
    fn holds(dev: &DevSide, sender: &str) -> Result<(), Error> {
        match lock(&dev.session).as_deref() {
            Some(holder) if holder == sender => Ok(()),
            Some(_) => Err(Error::Busy(BUSY.into())),
            None => Err(Error::NotAuthorized("call BeginTest() first".into())),
        }
    }

    /// Saves the settings `change` makes, and tells the engine and the watchers. Choosing what's
    /// already set stores, sends and signals nothing (`docs/spec/display.md`).
    fn save(&self, change: impl FnOnce(&mut Settings)) -> Result<(), Error> {
        let mut saved = lock(&self.settings);
        let mut new = saved.clone();
        change(&mut new);
        if new == *saved {
            return Ok(());
        }
        if let Err(e) = self.store.store(&new) {
            self.journal.write(
                Priority::Error,
                &format!("the settings can't be saved: {e}"),
            );
            return Err(Error::ZBus(zbus::Error::Failure(format!(
                "the settings can't be saved: {e}"
            ))));
        }
        *saved = new.clone();
        drop(saved);
        self.apply.merge(
            Apply {
                settings: Some(new),
                ..Apply::default()
            },
            Apply::then,
        );
        self.pulse.send();
        Ok(())
    }

    fn set_mode(&self, mode: u32) -> Result<(), Error> {
        let shown = u16::try_from(mode)
            .ok()
            .and_then(Mode::new)
            .ok_or_else(|| invalid(format!("mode {mode} isn't in the mode table")))?;
        if !draws(shown) {
            return Err(Error::NotYetAvailable(format!(
                "mode {mode} comes in a later version"
            )));
        }
        if let Some(reason) = read(&self.status).availability.reasons.get(&mode) {
            return Err(Error::ModeUnavailable(reason.clone()));
        }
        self.save(|s| s.mode = shown.number())
    }

    fn choose_gpu(&self, pci: &str) -> Result<(), Error> {
        within("a PCI address", pci, STRING_LIMIT)?;
        let listed = read(&self.status)
            .gpus
            .iter()
            .any(|gpu| gpu.pci.to_string() == pci);
        if !listed {
            return Err(invalid(format!("{pci:?} isn't listed in Gpus")));
        }
        self.save(|s| s.gpu_choice = Some(pci.to_owned()))
    }

    /// The input listed in `FanInputs` with this chip, device path and number, or none for all
    /// empty.
    fn fan_input(&self, chip: &str, device: &str, input: u32) -> Result<Option<SavedInput>, Error> {
        within("a chip name", chip, STRING_LIMIT)?;
        within("a device path", device, PATH_LIMIT)?;
        if chip.is_empty() && device.is_empty() && input == 0 {
            return Ok(None);
        }
        let listed = read(&self.status).fan_inputs.iter().any(|found| {
            found.chip == chip && found.device.to_str() == Some(device) && found.input == input
        });
        if !listed {
            return Err(invalid(format!(
                "{chip:?}, {device:?}, {input} isn't listed in FanInputs"
            )));
        }
        Ok(Some(SavedInput {
            chip: chip.to_owned(),
            device_path: device.to_owned(),
            input,
        }))
    }

    /// The whole state: the settings' keys from the saved settings, the rest from the status.
    fn state(&self) -> State {
        let settings = lock(&self.settings).clone();
        state_of(&settings, &read(&self.status))
    }

    /// After a wake-up from standby: a new conversation with the cooler, and the readings
    /// averaged over the interval start over.
    fn woke(&self) {
        self.journal.write(
            Priority::Info,
            "back from standby: a new conversation with the cooler, and the readings start over",
        );
        // The device thread only ends with the service.
        let _ = self.device.send(DeviceControl::Wake);
        self.apply.merge(
            Apply {
                wake: true,
                ..Apply::default()
            },
            Apply::then,
        );
    }

    /// A name left the bus: its watcher goes, and a test session it held ends.
    fn gone(&self, name: &str) {
        lock(&self.watchers).remove(name);
        if let Some(dev) = &self.dev
            && lock(&dev.session).as_deref() == Some(name)
        {
            self.journal.write(
                Priority::Notice,
                &format!("the test session of {name} ended: it left the bus"),
            );
            self.end_session(dev);
        }
    }

    /// Ends the test session: the override goes, the recording stops, and a released cooler is
    /// taken back, `03 08` first. Then the recording keeps that `03 08` and the frame after it,
    /// so a test sees the service take the cooler back (owner, 2026-11-04).
    fn end_session(&self, dev: &DevSide) {
        *lock(&dev.session) = None;
        self.post_override(None);
        if dev.released.swap(false, Ordering::Relaxed) {
            dev.recording.end_session_keeping_return();
            let _ = self.device.send(DeviceControl::Resume);
        } else {
            dev.recording.end_session();
        }
    }

    /// Posts a readings override, or its end; returns its number.
    fn post_override(&self, readings: Option<Snapshot>) -> u64 {
        let id = self.overrides.fetch_add(1, Ordering::Relaxed) + 1;
        self.apply.merge(
            Apply {
                readings: Some(Override { id, readings }),
                ..Apply::default()
            },
            Apply::then,
        );
        id
    }

    /// Waits until the engine took override `id`, or a newer one; returns that tick.
    async fn adopted(&self, id: u64) -> u64 {
        let mut seen = self.pulse.count();
        loop {
            if let Some((taken, tick)) = read(&self.status).adopted
                && taken >= id
            {
                return tick;
            }
            seen = self.pulse.after(seen).await;
        }
    }

    /// Checks each watcher for `use` again; returns those that pass again after failing, which
    /// get the whole state.
    async fn check_watchers(&self) -> Vec<String> {
        let names = lock(&self.watchers).names();
        let mut again = Vec::new();
        for name in names {
            let allowed = matches!(self.authorizer.allowed(&name, Action::Use).await, Ok(true));
            match lock(&self.watchers).checked(&name, allowed) {
                Checked::Again => {
                    self.journal.write(
                        Priority::Info,
                        &format!("watcher {name} passes polkit's check again"),
                    );
                    again.push(name);
                }
                Checked::Failed => self.journal.write(
                    Priority::Notice,
                    &format!(
                        "watcher {name} no longer passes polkit's check: skipped until it does"
                    ),
                ),
                Checked::Same => {}
            }
        }
        again
    }
}

/// The state the service tells its clients (`docs/spec/dbus.md`, State).
fn state_of(settings: &Settings, status: &Status) -> State {
    let (connected, cooler) = cooler_state(status);
    let input = |saved: &Option<SavedInput>| {
        saved.as_ref().map(|s| Input {
            chip: s.chip.clone(),
            device: s.device_path.clone(),
            input: s.input,
        })
    };
    let fans = status.snapshot.as_ref();
    State {
        mode: Some(u32::from(shown_mode(settings).number())),
        rotation: Some(settings.rotation),
        temperature_unit: Some(match settings.temperature_unit {
            TemperatureUnit::Celsius => odc_ipc::TemperatureUnit::Celsius,
            TemperatureUnit::Fahrenheit => odc_ipc::TemperatureUnit::Fahrenheit,
        }),
        cooler_connected: Some(connected),
        cooler_state: Some(cooler),
        available_modes: Some(status.availability.modes.clone()),
        unavailable_reasons: Some(
            status
                .availability
                .reasons
                .iter()
                .map(|(mode, why)| (*mode, why.clone()))
                .collect(),
        ),
        gpus: Some(
            status
                .gpus
                .iter()
                .map(|gpu| odc_ipc::Gpu {
                    pci: gpu.pci.to_string(),
                    driver: gpu.driver.clone(),
                    kind: match &gpu.kind {
                        odc_sensors::GpuKind::Dedicated => odc_ipc::GpuKind::Dedicated,
                        odc_sensors::GpuKind::Integrated => odc_ipc::GpuKind::Integrated,
                        odc_sensors::GpuKind::Unknown(odc_sensors::Unknown::QueryFailed {
                            ..
                        }) => odc_ipc::GpuKind::QueryFailed,
                        odc_sensors::GpuKind::Unknown(_) => odc_ipc::GpuKind::Unknown,
                    },
                })
                .collect(),
        ),
        chosen_gpu: Some(settings.gpu_choice.clone().unwrap_or_default()),
        fan_inputs: Some(
            status
                .fan_inputs
                .iter()
                .map(|found| odc_ipc::FanInput {
                    chip: found.chip.clone(),
                    device: found.device.to_string_lossy().into_owned(),
                    input: found.input,
                    label: found.label.clone().unwrap_or_default(),
                })
                .collect(),
        ),
        cooler_fan_input: Some(input(&settings.cooler_fan_input)),
        pump_input: Some(input(&settings.pump_input)),
        cooler_fan_rpm: Some(speed(fans.map(|s| &s.cooler_fans))),
        pump_rpm: Some(speed(fans.map(|s| &s.pump))),
    }
}

/// `CoolerConnected` and `CoolerState`, from the last write (owner, 2026-10-27; solution review
/// P5-02), or the release.
fn cooler_state(status: &Status) -> (bool, CoolerState) {
    if status.released {
        return (false, CoolerState::Released);
    }
    match &status.link {
        Some(Link::Connected) => (true, CoolerState::Connected),
        Some(Link::Disconnected(Reason::NotFound)) | None => (false, CoolerState::NotFound),
        Some(Link::Disconnected(Reason::NoAccess { .. })) => (false, CoolerState::NoAccess),
        Some(Link::Disconnected(Reason::WriteFailed { .. })) => (false, CoolerState::WriteFailed),
    }
}

/// A fan's speed in whole RPM, capped at 9999, like every number the display draws.
fn speed(fan: Option<&Fan>) -> FanSpeed {
    match fan {
        Some(Fan::Chosen(Reading::Value(rpm))) => match number(Some(*rpm)) {
            Shown::Number(n) => FanSpeed::Rpm(n),
            Shown::Dashes => FanSpeed::Unavailable,
        },
        _ => FanSpeed::Unavailable,
    }
}

/// `Diagnostics()`: each metric, in the spec's order, with its source and its state (owner,
/// 2026-10-29).
fn diagnostics(status: &Status) -> Vec<(String, String, String)> {
    Metric::ALL
        .into_iter()
        .map(|metric| {
            let source = status
                .bindings
                .iter()
                .find(|(m, _)| *m == metric)
                .and_then(|(_, binding)| match binding {
                    Binding::Bound(source) => Some(source.clone()),
                    Binding::Unbound(_) => None,
                })
                .unwrap_or_default();
            let state = match status.snapshot.as_ref().map(|s| s.get(metric)) {
                Some(Reading::Value(_)) => "ok".to_owned(),
                Some(Reading::Unavailable(reason)) => reason.to_string(),
                None => Unavailable::NotYet.to_string(),
            };
            (metric.spec_name().to_owned(), source, state)
        })
        .collect()
}

/// The reason kinds `OverrideReadings` takes (owner, 2026-10-29), each with the reason it stands
/// for.
pub fn reason_of(kind: &str) -> Option<Unavailable> {
    let placeholder = || "override".to_owned();
    Some(match kind {
        "no-rule" => Unavailable::NoRule {
            driver: placeholder(),
        },
        "no-device" => Unavailable::NoDevice,
        "permission-denied" => Unavailable::PermissionDenied {
            path: PathBuf::new(),
        },
        "read-failed" => Unavailable::ReadFailed {
            path: PathBuf::new(),
        },
        "not-yet" => Unavailable::NotYet,
        "integrated-gpu-power" => Unavailable::IntegratedGpuPower,
        "unknown-gpu-kind" => Unavailable::UnknownGpuKind,
        "driver-updated" => Unavailable::DriverUpdated,
        "library-failed" => Unavailable::LibraryFailed {
            library: placeholder(),
            error: String::new(),
        },
        _ => return None,
    })
}

/// A reason's kind; every reason has one, so a reason Phase 6 adds fails to compile here until
/// it gets its kind.
pub fn kind_of(reason: &Unavailable) -> &'static str {
    match reason {
        Unavailable::NoRule { .. } => "no-rule",
        Unavailable::NoDevice => "no-device",
        Unavailable::PermissionDenied { .. } => "permission-denied",
        Unavailable::ReadFailed { .. } => "read-failed",
        Unavailable::NotYet => "not-yet",
        Unavailable::IntegratedGpuPower => "integrated-gpu-power",
        Unavailable::UnknownGpuKind => "unknown-gpu-kind",
        Unavailable::DriverUpdated => "driver-updated",
        Unavailable::LibraryFailed { .. } => "library-failed",
    }
}

/// `OverrideReadings`' arguments as readings: all seven metrics, each in exactly one map, by the
/// spec's names (owner, 2026-10-29). The fans are left as they are.
fn overridden(
    values: &HashMap<String, (i64, u64)>,
    unavailable: &HashMap<String, String>,
) -> Result<Snapshot, Error> {
    let mut readings: HashMap<Metric, Reading> = HashMap::new();
    let metric = |name: &str| {
        within("a metric name", name, STRING_LIMIT)?;
        Metric::from_spec_name(name).ok_or_else(|| invalid(format!("no metric is called {name:?}")))
    };
    for (name, &(num, den)) in values {
        let exact =
            Exact::new(num, den).ok_or_else(|| invalid(format!("{name}: denominator 0")))?;
        readings.insert(metric(name)?, Reading::Value(exact));
    }
    for (name, kind) in unavailable {
        within("a reason", kind, STRING_LIMIT)?;
        let reason =
            reason_of(kind).ok_or_else(|| invalid(format!("{name}: no reason kind {kind:?}")))?;
        if readings
            .insert(metric(name)?, Reading::Unavailable(reason))
            .is_some()
        {
            return Err(invalid(format!("{name} is given twice")));
        }
    }
    if let Some(missing) = Metric::ALL.iter().find(|m| !readings.contains_key(m)) {
        return Err(invalid(format!("{} is missing", missing.spec_name())));
    }
    Ok(Snapshot::new(
        |metric| readings.remove(&metric).expect("every metric"),
        Fan::NotChosen,
        Fan::NotChosen,
    ))
}

/// The main interface (`docs/spec/dbus.md`, Main interface). It has no properties: the state is
/// read with `GetState()`, and changes go to each watcher alone.
pub struct Main {
    shared: Arc<Shared>,
}

#[zbus::interface(
    name = "io.github.dtmarangoni.OpenDeepCool1",
    introspection_docs = false
)]
impl Main {
    async fn set_mode(&self, #[zbus(header)] header: Header<'_>, mode: u32) -> Result<(), Error> {
        let call = format!("SetMode({mode})");
        let sender = self.shared.changer(&header, &call).await?;
        self.shared
            .logged(&call, &sender, self.shared.set_mode(mode))
    }

    async fn rotate_screen(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        self.shared.changer(&header, "RotateScreen").await?;
        self.shared.save(|s| s.rotation = (s.rotation + 1) % 4)
    }

    async fn set_rotation(
        &self,
        #[zbus(header)] header: Header<'_>,
        rotation: u8,
    ) -> Result<(), Error> {
        let call = format!("SetRotation({rotation})");
        let sender = self.shared.changer(&header, &call).await?;
        let result = if rotation > 3 {
            Err(invalid(format!("rotation {rotation} isn't 0 to 3")))
        } else {
            self.shared.save(|s| s.rotation = rotation)
        };
        self.shared.logged(&call, &sender, result)
    }

    async fn set_temperature_unit(
        &self,
        #[zbus(header)] header: Header<'_>,
        unit: &str,
    ) -> Result<(), Error> {
        let call = "SetTemperatureUnit";
        let sender = self.shared.changer(&header, call).await?;
        let result = within("a unit", unit, STRING_LIMIT).and_then(|()| {
            let unit = match unit {
                "celsius" => TemperatureUnit::Celsius,
                "fahrenheit" => TemperatureUnit::Fahrenheit,
                other => return Err(invalid(format!("{other:?} isn't celsius or fahrenheit"))),
            };
            self.shared.save(|s| s.temperature_unit = unit)
        });
        self.shared.logged(call, &sender, result)
    }

    async fn choose_gpu(&self, #[zbus(header)] header: Header<'_>, pci: &str) -> Result<(), Error> {
        let call = "ChooseGpu";
        let sender = self.shared.changer(&header, call).await?;
        self.shared
            .logged(call, &sender, self.shared.choose_gpu(pci))
    }

    async fn set_cooler_fan_input(
        &self,
        #[zbus(header)] header: Header<'_>,
        chip: &str,
        device: &str,
        input: u32,
    ) -> Result<(), Error> {
        let call = "SetCoolerFanInput";
        let sender = self.shared.changer(&header, call).await?;
        let result = self
            .shared
            .fan_input(chip, device, input)
            .and_then(|chosen| self.shared.save(|s| s.cooler_fan_input = chosen));
        self.shared.logged(call, &sender, result)
    }

    async fn set_pump_input(
        &self,
        #[zbus(header)] header: Header<'_>,
        chip: &str,
        device: &str,
        input: u32,
    ) -> Result<(), Error> {
        let call = "SetPumpInput";
        let sender = self.shared.changer(&header, call).await?;
        let result = self
            .shared
            .fan_input(chip, device, input)
            .and_then(|chosen| self.shared.save(|s| s.pump_input = chosen));
        self.shared.logged(call, &sender, result)
    }

    /// Fresh readings at once, and the picture from home; nothing is saved.
    async fn refresh(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        self.shared.changer(&header, "Refresh").await?;
        self.shared.apply.merge(
            Apply {
                refresh: true,
                ..Apply::default()
            },
            Apply::then,
        );
        Ok(())
    }

    #[zbus(out_args("state"))]
    async fn get_state(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<HashMap<String, OwnedVariant>, Error> {
        self.shared.reader(&header, "GetState").await?;
        Ok(self.shared.state().to_dict())
    }

    async fn watch(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let call = "Watch";
        let sender = self.shared.reader(&header, call).await?;
        let user = match self.shared.credentials.of(&sender).await {
            Ok(Identity {
                user: Some(user), ..
            }) => user,
            _ => {
                return Err(self.shared.refused(
                    call,
                    &sender,
                    Error::NotAuthorized("the bus doesn't say whose the caller is".into()),
                ));
            }
        };
        let added = lock(&self.shared.watchers)
            .add(&sender, user)
            .map_err(Error::TooManyWatchers);
        self.shared.logged(call, &sender, added)?;
        // A caller that left the bus before it was added would stay: its NameOwnerChanged came
        // first.
        if let Ok(name) = BusName::try_from(sender.as_str())
            && let Ok(false) = self.shared.dbus.name_has_owner(name).await
        {
            lock(&self.shared.watchers).remove(&sender);
        }
        Ok(())
    }

    async fn unwatch(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let sender = self.shared.reader(&header, "Unwatch").await?;
        lock(&self.shared.watchers).remove(&sender);
        Ok(())
    }

    #[zbus(out_args("metrics"))]
    async fn diagnostics(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<Vec<(String, String, String)>, Error> {
        self.shared.reader(&header, "Diagnostics").await?;
        Ok(diagnostics(&read(&self.shared.status)))
    }

    #[zbus(signal)]
    pub async fn state_changed(
        emitter: &SignalEmitter<'_>,
        changes: HashMap<String, OwnedVariant>,
    ) -> zbus::Result<()>;
}

/// The dev interface (`docs/spec/dbus.md`, Dev interface), registered only with `--dev`.
pub struct Dev {
    shared: Arc<Shared>,
}

#[zbus::interface(
    name = "io.github.dtmarangoni.OpenDeepCool1.Dev",
    introspection_docs = false
)]
impl Dev {
    #[zbus(out_args("tick"))]
    async fn override_readings(
        &self,
        #[zbus(header)] header: Header<'_>,
        values: HashMap<String, (i64, u64)>,
        unavailable: HashMap<String, String>,
    ) -> Result<u64, Error> {
        let call = "OverrideReadings";
        let (sender, dev) = self.shared.dev_caller(&header, call).await?;
        let readings = Shared::holds(dev, &sender).and_then(|()| overridden(&values, &unavailable));
        let readings = self.shared.logged(call, &sender, readings)?;
        let id = self.shared.post_override(Some(readings));
        Ok(self.shared.adopted(id).await)
    }

    async fn clear_override(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let call = "ClearOverride";
        let (sender, dev) = self.shared.dev_caller(&header, call).await?;
        self.shared
            .logged(call, &sender, Shared::holds(dev, &sender))?;
        let id = self.shared.post_override(None);
        self.shared.adopted(id).await;
        Ok(())
    }

    async fn begin_test(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let call = "BeginTest";
        let (sender, dev) = self.shared.dev_caller(&header, call).await?;
        let mut session = lock(&dev.session);
        match session.as_deref() {
            Some(holder) if holder == sender => Ok(()),
            Some(_) => Err(self.shared.refused(call, &sender, Error::Busy(BUSY.into()))),
            None => {
                *session = Some(sender.clone());
                drop(session);
                dev.durations.clear();
                dev.recording.begin_session();
                self.shared.journal.write(
                    Priority::Notice,
                    &format!("a test session began, held by {sender}"),
                );
                Ok(())
            }
        }
    }

    async fn end_test(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let call = "EndTest";
        let (sender, dev) = self.shared.dev_caller(&header, call).await?;
        self.shared
            .logged(call, &sender, Shared::holds(dev, &sender))?;
        self.shared.end_session(dev);
        self.shared.journal.write(
            Priority::Notice,
            &format!("the test session of {sender} ended"),
        );
        Ok(())
    }

    /// Each frame write's and each snapshot's duration in µs since the session began, the newest
    /// 100 000 of each (owner, 2026-11-04). It needs the test session.
    #[zbus(out_args("writes", "snapshots"))]
    async fn durations(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(Vec<u64>, Vec<u64>), Error> {
        let call = "Durations";
        let (sender, dev) = self.shared.dev_caller(&header, call).await?;
        self.shared
            .logged(call, &sender, Shared::holds(dev, &sender))?;
        Ok(dev.durations.lists())
    }

    /// Closes the cooler until the session ends (owner, 2026-10-29); returns once the device
    /// thread let it go.
    async fn release_cooler(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let call = "ReleaseCooler";
        let (sender, dev) = self.shared.dev_caller(&header, call).await?;
        self.shared
            .logged(call, &sender, Shared::holds(dev, &sender))?;
        dev.released.store(true, Ordering::Relaxed);
        let _ = self.shared.device.send(DeviceControl::Release);
        let mut seen = self.shared.pulse.count();
        while !read(&self.shared.status).released {
            seen = self.shared.pulse.after(seen).await;
        }
        Ok(())
    }
}

/// Which bus the service serves on.
pub enum Bus {
    System,
    /// A private bus, for the tests.
    Address(String),
}

/// Who answers the checks.
pub enum Checks {
    /// polkit, and the bus's own credentials.
    Polkit,
    /// Given ones: the tests' fakes.
    Given {
        authorizer: Box<dyn Authorizer>,
        credentials: Box<dyn Credentials>,
    },
}

/// What the D-Bus side starts with.
pub struct Parts {
    pub bus: Bus,
    pub checks: Checks,
    pub store: SettingsStore,
    /// The saved settings, as loaded.
    pub settings: Settings,
    pub status: Arc<RwLock<Status>>,
    pub apply: Arc<Slot<Apply>>,
    pub device: Sender<DeviceControl>,
    pub pulse: Arc<Pulse>,
    pub journal: Arc<dyn Journal>,
    /// With `--dev`: the service's group, the recording, and the threads' durations.
    pub dev: Option<(u32, RecordingHandle, Arc<Durations>)>,
}

/// The running D-Bus side: its thread, inside one `zbus::block_on` call, and its timer.
pub struct Side {
    stop: Arc<AtomicBool>,
    pulse: Arc<Pulse>,
    threads: Vec<JoinHandle<()>>,
}

impl Side {
    /// Starts the D-Bus side; returns once the bus name is owned, so `READY=1` can follow.
    ///
    /// # Errors
    ///
    /// When the bus can't be reached, or the name is taken.
    pub fn start(parts: Parts) -> Result<Side, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let pulse = Arc::clone(&parts.pulse);
        let (ready, started) = mpsc::channel();
        let serving = {
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("dbus".into())
                .spawn(move || zbus::block_on(serve(parts, &ready, &stop)))
                .map_err(|e| e.to_string())?
        };
        let timer = {
            let (stop, pulse) = (Arc::clone(&stop), Arc::clone(&pulse));
            thread::Builder::new()
                .name("dbus-timer".into())
                .spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        thread::sleep(CHECK_EVERY);
                        pulse.send();
                    }
                })
                .map_err(|e| e.to_string())?
        };
        let side = Side {
            stop,
            pulse,
            threads: vec![serving, timer],
        };
        match started.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => Ok(side),
            Ok(Err(why)) => Err(why),
            Err(_) => Err("the bus didn't answer in time".into()),
        }
    }

    /// Stops serving: the name and the connection go. Dropping the side does the same.
    pub fn stop(self) {}
}

impl Drop for Side {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.pulse.send();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/// The D-Bus side's one `block_on` future: the connection, the interfaces, the subscriptions,
/// the name, then the watchers' signals, the names that leave the bus and the wake-ups, until the
/// service stops.
async fn serve(parts: Parts, ready: &Sender<Result<(), String>>, stop: &AtomicBool) {
    let (conn, shared, names, sleep) = match set_up(parts).await {
        Ok(set) => set,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    let tasks: Vec<Pin<Box<dyn Future<Output = ()> + Send + '_>>> = vec![
        Box::pin(tell_watchers(&shared, &conn, stop)),
        Box::pin(names.run(|name| shared.gone(name))),
        Box::pin(sleep.run(|| shared.woke())),
    ];
    first_of(tasks).await;
    if !stop.load(Ordering::Relaxed) {
        // The bus went away: the service ends, and systemd starts it again.
        panic!("the D-Bus connection ended");
    }
    // The interfaces hold users of the connection; without them, it closes.
    let server = conn.object_server();
    let _ = server.remove::<Main, _>(OBJECT_PATH).await;
    let _ = server.remove::<Dev, _>(DEV_PATH).await;
}

async fn set_up(parts: Parts) -> zbus::Result<(Connection, Arc<Shared>, NameWatch, SleepWatch)> {
    let conn = match parts.bus {
        Bus::System => connection::Builder::system(),
        Bus::Address(address) => connection::Builder::address(address.as_str()),
    }
    .build()
    .await?;
    let (authorizer, credentials): (Box<dyn Authorizer>, Box<dyn Credentials>) = match parts.checks
    {
        Checks::Polkit => (
            Box::new(PolkitAuthorizer::new(&conn).await?),
            Box::new(BusCredentials::new(&conn).await?),
        ),
        Checks::Given {
            authorizer,
            credentials,
        } => (authorizer, credentials),
    };
    let shared = Arc::new(Shared {
        store: parts.store,
        settings: Mutex::new(parts.settings),
        status: parts.status,
        apply: parts.apply,
        device: parts.device,
        authorizer,
        credentials,
        dbus: DBusProxy::new(&conn).await?,
        watchers: Mutex::new(Watchers::default()),
        pulse: parts.pulse,
        journal: parts.journal,
        dev: parts
            .dev
            .map(|(group, recording, durations)| DevSide::new(group, recording, durations)),
        overrides: AtomicU64::new(0),
    });
    let server = conn.object_server();
    server
        .at(
            OBJECT_PATH,
            Main {
                shared: Arc::clone(&shared),
            },
        )
        .await?;
    if shared.dev.is_some() {
        server
            .at(
                DEV_PATH,
                Dev {
                    shared: Arc::clone(&shared),
                },
            )
            .await?;
    }
    // Subscribed before the name is taken, so nothing is missed.
    let names = NameWatch::subscribe(&conn).await?;
    let sleep = SleepWatch::subscribe(&conn).await?;
    conn.request_name(BUS_NAME).await?;
    Ok((conn, shared, names, sleep))
}

/// Polls every task until the first one ends: the D-Bus side runs its tasks side by side with no
/// async library of its own.
async fn first_of(mut tasks: Vec<Pin<Box<dyn Future<Output = ()> + Send + '_>>>) {
    poll_fn(|cx| {
        for task in &mut tasks {
            if task.as_mut().poll(cx).is_ready() {
                return Poll::Ready(());
            }
        }
        Poll::Pending
    })
    .await;
}

/// Sends each watcher the keys that changed, at every pulse: the status changed, a setting was
/// saved, or a second passed, when every watcher is checked again. A watcher that passes again
/// gets the whole state; one that fails gets nothing (owner, 2026-10-26).
async fn tell_watchers(shared: &Shared, conn: &Connection, stop: &AtomicBool) {
    let mut sent = shared.state();
    let mut seen = shared.pulse.count();
    let mut check_at = Instant::now() + CHECK_EVERY;
    loop {
        seen = shared.pulse.after(seen).await;
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let again = if Instant::now() >= check_at {
            check_at = Instant::now() + CHECK_EVERY;
            shared.check_watchers().await
        } else {
            Vec::new()
        };
        let state = shared.state();
        let changes = state.changed_since(&sent);
        let watchers = lock(&shared.watchers).allowed();
        for name in watchers {
            let dict = if again.contains(&name) {
                state.to_dict()
            } else if !changes.is_empty() {
                changes.to_dict()
            } else {
                continue;
            };
            signal(conn, &name, dict).await;
        }
        sent = state;
    }
}

/// `StateChanged`, addressed to one watcher: the bus delivers it to that connection alone (D-Bus
/// specification, Message Bus Message Routing).
async fn signal(conn: &Connection, to: &str, changes: HashMap<String, OwnedVariant>) {
    let (Ok(destination), Ok(emitter)) =
        (BusName::try_from(to), SignalEmitter::new(conn, OBJECT_PATH))
    else {
        return;
    };
    // A watcher that just left misses nothing it could read.
    let _ = Main::state_changed(&emitter.set_destination(destination), changes).await;
}

/// The bus's `NameOwnerChanged`: a watcher, or a test session's holder, whose connection closed.
struct NameWatch {
    stream: MessageStream,
}

/// The bus itself, which alone sends `NameOwnerChanged`.
const BUS_DRIVER: &str = "org.freedesktop.DBus";

impl NameWatch {
    async fn subscribe(conn: &Connection) -> zbus::Result<NameWatch> {
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(BUS_DRIVER)
            .interface(BUS_DRIVER)
            .member("NameOwnerChanged")
            .build()?;
        Ok(NameWatch {
            stream: MessageStream::for_match_rule(rule, conn, None).await?,
        })
    }

    /// Calls `gone` for each unique name that leaves the bus. A look-alike addressed to the
    /// service from another connection is ignored: only the bus sends the real one.
    async fn run(mut self, gone: impl Fn(&str)) {
        while let Some(message) = next(&mut self.stream).await {
            let Ok(message) = message else {
                continue;
            };
            if message.header().sender().map(|s| s.as_str()) != Some(BUS_DRIVER) {
                continue;
            }
            let Ok((name, _, new_owner)) = message.body().deserialize::<(String, String, String)>()
            else {
                continue;
            };
            if name.starts_with(':') && new_owner.is_empty() {
                gone(&name);
            }
        }
    }
}
