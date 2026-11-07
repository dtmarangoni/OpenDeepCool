//! A fake `opendeepcoold` for the settings app's end-to-end run (Phase 10 plan, Task 8;
//! decisions log D37). It serves the main interface of `odc-ipc` (`docs/spec/dbus.md`) on a
//! private bus, takes its state and its answers from a file the test writes, and writes every
//! call it gets to a log the test reads. So `odc-ui-e2e`, which links another major version of
//! zbus, steers it with files alone.
//!
//! It never applies a change itself: a change is answered and logged, and the test confirms it
//! by writing the new state, which the fake sends each watcher as `StateChanged` with the keys
//! that changed, as the service does. So a test sees exactly when the app gets the service's
//! word (`docs/design/settings-app.md`, Refused changes).
//!
//! `state.toml` is written whole, then moved into place, so the fake never reads half of it:
//!
//! ```toml
//! [fake]
//! serial = 3        # logged as `state 3` once the file is taken
//! on_bus = true     # owns the service's bus name; false gives it up, as a stopped service
//! reads = true      # false: every read and watch is refused with NotAuthorized
//! refuse = "ModeUnavailable: Gpu usage: no device found"  # when set: every change, and Refresh
//!
//! [state]           # the contract's keys, each as its signature's TOML form
//! Mode = 1
//! Gpus = [["0000:0a:00.0", "acmegpu", "dedicated", 4660, 22136, true]]
//! ```
//!
//! `calls.log` gets one line per call, such as `SetMode(5)`, followed by ` refused <error>` when
//! refused, and `state <serial>`, or `state error: <why>`, for each file taken.

use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use odc_ipc::{BUS_NAME, Error, OBJECT_PATH, STATE_KEYS};
use zbus::message::Header;
use zbus::names::BusName;
use zbus::object_server::SignalEmitter;
use zbus::wire::{OwnedFd, OwnedVariant, Variant};
use zbus::{Connection, connection};

/// The file the test writes, and the log the fake writes, in the fake's folder.
pub const STATE_FILE: &str = "state.toml";
pub const CALL_LOG: &str = "calls.log";

/// How often the fake looks at the state file.
const POLL: Duration = Duration::from_millis(10);

/// The errors a state file's `refuse` may name: the service's own.
pub const ERRORS: [&str; 8] = [
    "NotAuthorized",
    "ModeUnavailable",
    "NotYetAvailable",
    "InvalidArgument",
    "TooManyWatchers",
    "Busy",
    "AudioNotWanted",
    "Failed",
];

type Dict = HashMap<String, OwnedVariant>;

/// What the `[fake]` table asks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Behaviour {
    serial: u64,
    on_bus: bool,
    reads: bool,
    /// The error's name and text.
    refuse: Option<(String, String)>,
}

#[derive(Default)]
struct Inner {
    behaviour: Behaviour,
    state: Dict,
    watchers: BTreeSet<String>,
    frame_watchers: BTreeSet<String>,
}

struct Shared {
    log: Mutex<File>,
    inner: Mutex<Inner>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn clone(dict: &Dict) -> Dict {
    dict.iter()
        .map(|(key, value)| {
            let value = value
                .try_clone()
                .expect("the state holds no file descriptor");
            (key.clone(), value)
        })
        .collect()
}

fn error(name: &str, text: &str) -> Error {
    let text = text.to_owned();
    match name {
        "NotAuthorized" => Error::NotAuthorized(text),
        "ModeUnavailable" => Error::ModeUnavailable(text),
        "NotYetAvailable" => Error::NotYetAvailable(text),
        "InvalidArgument" => Error::InvalidArgument(text),
        "TooManyWatchers" => Error::TooManyWatchers(text),
        "Busy" => Error::Busy(text),
        "AudioNotWanted" => Error::AudioNotWanted(text),
        _ => Error::Failed(text),
    }
}

impl Shared {
    fn log(&self, line: &str) {
        let mut log = lock(&self.log);
        // A log the test can't read fails the test, which reads it.
        let _ = writeln!(log, "{line}");
        let _ = log.flush();
    }

    /// A change, or Refresh: logged, then answered as the state file asks.
    fn change(&self, call: String) -> Result<(), Error> {
        let refuse = lock(&self.inner).behaviour.refuse.clone();
        match refuse {
            Some((name, text)) => {
                self.log(&format!("{call} refused {name}"));
                Err(error(&name, &text))
            }
            None => {
                self.log(&call);
                Ok(())
            }
        }
    }

    /// A read or a watch: logged, then refused when the state file says so.
    fn read(&self, call: &str) -> Result<MutexGuard<'_, Inner>, Error> {
        let inner = lock(&self.inner);
        if inner.behaviour.reads {
            self.log(call);
            Ok(inner)
        } else {
            self.log(&format!("{call} refused NotAuthorized"));
            Err(Error::NotAuthorized(
                "the fake refuses this session every read".to_owned(),
            ))
        }
    }
}

fn sender(header: &Header<'_>) -> Result<String, Error> {
    header
        .sender()
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Failed("a call without a sender".to_owned()))
}

/// The main interface, as `odc-ipc`'s proxy calls it.
struct Main {
    shared: Arc<Shared>,
}

#[zbus::interface(
    name = "io.github.dtmarangoni.OpenDeepCool1",
    introspection_docs = false
)]
impl Main {
    async fn set_mode(&self, mode: u32) -> Result<(), Error> {
        self.shared.change(format!("SetMode({mode})"))
    }

    async fn rotate_screen(&self) -> Result<(), Error> {
        self.shared.change("RotateScreen()".to_owned())
    }

    async fn set_rotation(&self, rotation: u8) -> Result<(), Error> {
        self.shared.change(format!("SetRotation({rotation})"))
    }

    async fn set_temperature_unit(&self, unit: &str) -> Result<(), Error> {
        self.shared.change(format!("SetTemperatureUnit({unit:?})"))
    }

    async fn choose_gpu(&self, pci: &str) -> Result<(), Error> {
        self.shared.change(format!("ChooseGpu({pci:?})"))
    }

    async fn set_cooler_fan_input(
        &self,
        chip: &str,
        device: &str,
        input: u32,
    ) -> Result<(), Error> {
        let call = format!("SetCoolerFanInput({chip:?}, {device:?}, {input})");
        self.shared.change(call)
    }

    async fn set_pump_input(&self, chip: &str, device: &str, input: u32) -> Result<(), Error> {
        let call = format!("SetPumpInput({chip:?}, {device:?}, {input})");
        self.shared.change(call)
    }

    async fn refresh(&self) -> Result<(), Error> {
        self.shared.change("Refresh()".to_owned())
    }

    /// The fake takes no sound.
    #[zbus(out_args("pipe"))]
    async fn attach_audio(&self) -> Result<OwnedFd, Error> {
        self.shared.log("AttachAudio() refused AudioNotWanted");
        Err(Error::AudioNotWanted("the fake takes no sound".to_owned()))
    }

    async fn set_music_peaks(&self, peaks: bool) -> Result<(), Error> {
        self.shared.change(format!("SetMusicPeaks({peaks})"))
    }

    async fn set_music_bar_fall(&self, stop: &str) -> Result<(), Error> {
        self.shared.change(format!("SetMusicBarFall({stop:?})"))
    }

    async fn set_music_peak_fall(&self, stop: &str) -> Result<(), Error> {
        self.shared.change(format!("SetMusicPeakFall({stop:?})"))
    }

    #[zbus(out_args("state"))]
    async fn get_state(&self) -> Result<Dict, Error> {
        Ok(clone(&self.shared.read("GetState()")?.state))
    }

    async fn watch(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let sender = sender(&header)?;
        self.shared.read("Watch()")?.watchers.insert(sender);
        Ok(())
    }

    async fn unwatch(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let sender = sender(&header)?;
        self.shared.read("Unwatch()")?.watchers.remove(&sender);
        Ok(())
    }

    async fn watch_frames(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let sender = sender(&header)?;
        self.shared
            .read("WatchFrames()")?
            .frame_watchers
            .insert(sender);
        Ok(())
    }

    async fn unwatch_frames(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        let sender = sender(&header)?;
        self.shared
            .read("UnwatchFrames()")?
            .frame_watchers
            .remove(&sender);
        Ok(())
    }

    #[zbus(out_args("metrics"))]
    async fn diagnostics(&self) -> Result<Vec<(String, String, String)>, Error> {
        drop(self.shared.read("Diagnostics()")?);
        Ok(Vec::new())
    }

    #[zbus(signal)]
    async fn state_changed(emitter: &SignalEmitter<'_>, changes: Dict) -> zbus::Result<()>;
}

/// The fake, serving until dropped.
pub struct FakeService {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeService {
    /// Connects to the bus at `address` and serves there, with its files in `dir`; returns once
    /// connected. Until a state file is taken, it doesn't own the service's name: the app sees
    /// no service.
    ///
    /// # Errors
    ///
    /// When the log can't be created, or the bus can't be reached.
    pub fn start(address: &str, dir: &Path) -> io::Result<FakeService> {
        fs::create_dir_all(dir)?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(CALL_LOG))?;
        let shared = Arc::new(Shared {
            log: Mutex::new(log),
            inner: Mutex::new(Inner::default()),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let (ready, started) = mpsc::channel();
        let (address, dir, flag) = (address.to_owned(), dir.to_owned(), Arc::clone(&stop));
        let thread = thread::spawn(move || run(&address, &dir, &shared, &flag, &ready));
        match started.recv() {
            Ok(Ok(())) => Ok(FakeService {
                stop,
                thread: Some(thread),
            }),
            Ok(Err(e)) => Err(io::Error::other(e)),
            Err(_) => Err(io::Error::other("the fake service's thread ended")),
        }
    }
}

impl Drop for FakeService {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The fake's thread: the connection, then the state file, looked at every 10 ms. zbus runs
/// the connection on threads of its own, so the calls are answered meanwhile.
fn run(
    address: &str,
    dir: &Path,
    shared: &Arc<Shared>,
    stop: &AtomicBool,
    ready: &mpsc::Sender<Result<(), String>>,
) {
    let main = Main {
        shared: Arc::clone(shared),
    };
    let built = zbus::block_on(async {
        connection::Builder::address(address)
            .serve_at(OBJECT_PATH, main)
            .build()
            .await
    });
    let conn = match built {
        Ok(conn) => conn,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    let path = dir.join(STATE_FILE);
    let mut seen = None;
    let mut owned = false;
    while !stop.load(Ordering::Relaxed) {
        let stamp = fs::metadata(&path)
            .ok()
            .map(|m| (m.ino(), m.mtime(), m.mtime_nsec(), m.len()));
        if stamp.is_some() && stamp != seen {
            seen = stamp;
            zbus::block_on(take(&conn, &path, shared, &mut owned));
        }
        thread::sleep(POLL);
    }
}

/// Takes the state file: its state and its answers, the changed keys sent to each watcher, and
/// the bus name taken or given up.
async fn take(conn: &Connection, path: &PathBuf, shared: &Shared, owned: &mut bool) {
    let parsed = fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|text| parse(&text));
    let (behaviour, state) = match parsed {
        Ok(parsed) => parsed,
        Err(why) => {
            shared.log(&format!("state error: {why}"));
            return;
        }
    };
    let serial = behaviour.serial;
    let on_bus = behaviour.on_bus;
    let sends = {
        let mut inner = lock(&shared.inner);
        let changed: Dict = state
            .iter()
            .filter(|(key, value)| inner.state.get(*key) != Some(*value))
            .map(|(key, value)| {
                let value = value.try_clone().expect("no file descriptor");
                (key.clone(), value)
            })
            .collect();
        // A service that leaves takes its watchers with it, and one that refuses a session
        // drops its watches at the next check (Phase 7: each second).
        if !behaviour.reads || !on_bus {
            inner.watchers.clear();
            inner.frame_watchers.clear();
        }
        inner.state = state;
        inner.behaviour = behaviour;
        let mut sends = Vec::new();
        if *owned && on_bus && !changed.is_empty() {
            for watcher in &inner.watchers {
                let mut changes = clone(&changed);
                // `FanInputs` goes to frame watchers only (Phase 10 plan, Task 2).
                if !inner.frame_watchers.contains(watcher) {
                    changes.remove("FanInputs");
                }
                if !changes.is_empty() {
                    sends.push((watcher.clone(), changes));
                }
            }
        }
        sends
    };
    for (to, changes) in sends {
        if let (Ok(to), Ok(emitter)) =
            (BusName::try_from(to), SignalEmitter::new(conn, OBJECT_PATH))
        {
            let _ = Main::state_changed(&emitter.set_destination(to), changes).await;
        }
    }
    if on_bus != *owned {
        let result = if on_bus {
            conn.request_name(BUS_NAME).await.map(|_| ())
        } else {
            conn.release_name(BUS_NAME).await.map(|_| ())
        };
        if let Err(e) = result {
            shared.log(&format!("state error: the bus name: {e}"));
            return;
        }
        *owned = on_bus;
    }
    shared.log(&format!("state {serial}"));
}

/// A state file: its `[fake]` answers and its `[state]`, each key in the contract's type.
fn parse(text: &str) -> Result<(Behaviour, Dict), String> {
    let table: toml::Table = text.parse().map_err(|e| format!("{e}"))?;
    let fake = table
        .get("fake")
        .and_then(toml::Value::as_table)
        .ok_or("no [fake] table")?;
    let flag = |key: &str| {
        fake.get(key)
            .and_then(toml::Value::as_bool)
            .ok_or(format!("[fake] {key} isn't true or false"))
    };
    let serial = fake
        .get("serial")
        .and_then(toml::Value::as_integer)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or("[fake] serial isn't a number")?;
    let refuse = match fake.get("refuse") {
        None => None,
        Some(value) => {
            let text = value.as_str().ok_or("[fake] refuse isn't text")?;
            let (name, why) = text
                .split_once(": ")
                .ok_or("[fake] refuse isn't \"<error>: <text>\"")?;
            if !ERRORS.contains(&name) {
                return Err(format!("{name} isn't one of the service's errors"));
            }
            Some((name.to_owned(), why.to_owned()))
        }
    };
    let behaviour = Behaviour {
        serial,
        on_bus: flag("on_bus")?,
        reads: flag("reads")?,
        refuse,
    };
    let given = table
        .get("state")
        .and_then(toml::Value::as_table)
        .ok_or("no [state] table")?;
    let mut state = Dict::new();
    for (key, value) in given {
        let signature = STATE_KEYS
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, signature)| *signature)
            .ok_or(format!("{key} isn't a state key"))?;
        state.insert(key.clone(), variant(key, signature, value)?);
    }
    Ok((behaviour, state))
}

fn int<T: TryFrom<i64>>(value: &toml::Value) -> Option<T> {
    value.as_integer().and_then(|n| T::try_from(n).ok())
}

fn text(value: &toml::Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

/// A float, an integer taken as one, or TOML's `nan` (an unreadable speed).
fn number(value: &toml::Value) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|n| n as f64))
}

fn items(value: &toml::Value, len: usize) -> Option<&[toml::Value]> {
    value
        .as_array()
        .map(Vec::as_slice)
        .filter(|items| items.len() == len)
}

fn list<T>(value: &toml::Value, each: impl Fn(&toml::Value) -> Option<T>) -> Option<Vec<T>> {
    value.as_array()?.iter().map(each).collect()
}

/// `value` as `key`'s signature asks (`STATE_KEYS`): integers, text, booleans and numbers, a
/// table of modes for `a{us}`, and an array of fields for each structure.
fn variant(key: &str, signature: &str, value: &toml::Value) -> Result<OwnedVariant, String> {
    let variant: Option<Variant<'static>> = match signature {
        "u" => int::<u32>(value).map(Variant::from),
        "y" => int::<u8>(value).map(Variant::from),
        "s" => text(value).map(Variant::from),
        "b" => value.as_bool().map(Variant::from),
        "d" => number(value).map(Variant::from),
        "au" => list(value, int::<u32>).map(Variant::from),
        "a{us}" => value.as_table().and_then(|table| {
            table
                .iter()
                .map(|(mode, reason)| Some((mode.parse::<u32>().ok()?, text(reason)?)))
                .collect::<Option<HashMap<u32, String>>>()
                .map(Variant::from)
        }),
        "a(sssqqb)" => list(value, |gpu| {
            let [pci, driver, kind, vendor, device, rule] = items(gpu, 6)? else {
                return None;
            };
            Some((
                text(pci)?,
                text(driver)?,
                text(kind)?,
                int::<u16>(vendor)?,
                int::<u16>(device)?,
                rule.as_bool()?,
            ))
        })
        .map(Variant::from),
        "a(ssusd)" => list(value, |input| {
            let [chip, device, number_, label, rpm] = items(input, 5)? else {
                return None;
            };
            Some((
                text(chip)?,
                text(device)?,
                int::<u32>(number_)?,
                text(label)?,
                number(rpm)?,
            ))
        })
        .map(Variant::from),
        "(ssu)" => items(value, 3).and_then(|fields| {
            let [chip, device, input] = fields else {
                return None;
            };
            Some(Variant::from((
                text(chip)?,
                text(device)?,
                int::<u32>(input)?,
            )))
        }),
        _ => None,
    };
    let variant = variant.ok_or(format!("{key} isn't {signature}"))?;
    OwnedVariant::try_from(variant).map_err(|e| format!("{key}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ON: &str = "serial = 1\non_bus = true\nreads = true";

    #[test]
    fn a_file_it_can_t_take_is_refused_with_its_reason() {
        for (fake, state, why) in [
            (
                "serial = 1\non_bus = true",
                "",
                "[fake] reads isn't true or false",
            ),
            (
                "on_bus = true\nreads = true",
                "",
                "[fake] serial isn't a number",
            ),
            (ON, "Mood = 1", "Mood isn't a state key"),
            (ON, "Mode = \"one\"", "Mode isn't u"),
            (ON, "Mode = -1", "Mode isn't u"),
            (ON, "Rotation = 300", "Rotation isn't y"),
            (ON, "AvailableModes = [1, \"2\"]", "AvailableModes isn't au"),
            (
                ON,
                "UnavailableReasons = { x = \"why\" }",
                "UnavailableReasons isn't a{us}",
            ),
            (
                ON,
                "Gpus = [[\"0000:0a:00.0\", \"acmegpu\"]]",
                "Gpus isn't a(sssqqb)",
            ),
            (ON, "PumpInput = [\"acme6000\", 2]", "PumpInput isn't (ssu)"),
            (
                "serial = 1\non_bus = true\nreads = true\nrefuse = \"Nope: no\"",
                "",
                "Nope isn't one of the service's errors",
            ),
            (
                "serial = 1\non_bus = true\nreads = true\nrefuse = \"no\"",
                "",
                "[fake] refuse isn't \"<error>: <text>\"",
            ),
        ] {
            let text = format!("[fake]\n{fake}\n[state]\n{state}");
            assert_eq!(parse(&text).map(|_| ()), Err(why.to_owned()), "{text}");
        }
        assert!(parse("[state]\n").is_err());
        assert!(parse(&format!("[fake]\n{ON}\n")).is_err());
    }

    #[test]
    fn each_key_takes_its_signature_s_toml_form() {
        let text = format!(
            "[fake]\n{ON}\nrefuse = \"NotAuthorized: no\"\n[state]\n{}",
            r#"Mode = 23
Rotation = 3
TemperatureUnit = "fahrenheit"
CoolerConnected = false
CoolerState = "on fire"
AvailableModes = []
UnavailableReasons = { 2 = "Gpu usage: no device found", 21 = "Gpu usage: no device found" }
Gpus = []
ChosenGpu = ""
GpuInUse = ""
FanInputs = [["acme6000", "devices/platform/acme6000.2592", 1, "Pump", nan]]
CoolerFanInput = ["", "", 0]
CoolerFanRpm = nan
PumpRpm = 3400
MusicBarFall = "faster""#
        );
        let (behaviour, state) = parse(&text).expect("taken");
        assert_eq!(
            behaviour,
            Behaviour {
                serial: 1,
                on_bus: true,
                reads: true,
                refuse: Some(("NotAuthorized".to_owned(), "no".to_owned())),
            }
        );
        for (key, signature) in STATE_KEYS {
            if let Some(value) = state.get(key) {
                assert_eq!(value.value_signature().to_string(), signature, "{key}");
            }
        }
        assert_eq!(state.len(), 15);
        // A value the contract doesn't know reaches the app as given (decisions log D33).
        assert_eq!(
            String::try_from(state["CoolerState"].try_clone().expect("cloned")),
            Ok("on fire".to_owned())
        );
    }
}
