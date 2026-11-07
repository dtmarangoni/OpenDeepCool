//! Layer 3, the D-Bus side (Phase 7 plan, Task 7; `docs/spec/dbus.md`): every call checked with
//! polkit, every argument compared with what the service found, the watchers and their check each
//! second, the dev interface and its test session, the wake-up from standby, and the contract.
//! Each test runs on a private `dbus-daemon` of its own, with a fake polkit and fake credentials
//! (owner, 2026-10-29). The D-Bus side runs alone where a test reads what it hands the engine and
//! the device thread; the whole service runs, with the fake cooler, where a test needs those
//! threads. Every device, chip and address is made up (AGENTS.md rule 10).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::future::{Future, poll_fn};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::pin::{Pin, pin};
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, RwLock};
use std::task::Poll;
use std::time::Duration;

use futures_core::Stream;
use odc_device::fake::{FakeConnector, FakeLog, Presence};
use odc_device::{Link, Lp360, Recording};
use odc_ipc::{
    BUS_NAME, CHANGE_SETTINGS_ACTION, CoolerState, DEV_INTERFACE, DEV_PATH, DevProxy, Error,
    FallStop, FanSpeed, INTERFACE, OBJECT_PATH, OpenDeepCoolProxy, STATE_KEYS, State, USE_ACTION,
    frame_from_pixels,
};
use odc_protocol::{Frame, Message, decode};
use odc_render::{Mode, WINDOW, draws};
use odc_sensors::{
    Binding, Exact, Fan, FanInput, Gpu, GpuKind, Metric, PciAddr, Reading, Snapshot, Unavailable,
};
use odc_service::audio::{AudioSource, PipeAudio};
use odc_service::dbus::{Bus, Checks, Parts, Side, kind_of, reason_of};
use odc_service::engine::{Apply, Choices, Drawn, Engine, availability, availability_of};
use odc_service::log::{Journal, Recorded};
use odc_service::polkit::{
    Action, Answer, Authorizer, BusCredentials, Credentials, Identity, PolkitAuthorizer,
};
use odc_service::pulse::Pulse;
use odc_service::runtime::{DeviceControl, Durations, SensorLoop, SensorRequest, Sensors, Slot};
use odc_service::service::{Config, Running};
use odc_service::settings::{Fall, SavedInput, Settings, SettingsStore, TemperatureUnit};
use odc_service::status::{FIRST_READINGS, Status, parse_first_readings};
use odc_testkit::bus::{PrivateBus, sleep, within};
use odc_testkit::capture::read_recording;
use zbus::message::Header;
use zbus::wire::OwnedVariant;
use zbus::{Connection, Guid, connection};

/// The callers' made-up Unix user and process, and the service's made-up group.
const USER: u32 = 1000;
const PID: u32 = 4242;
const GROUP: u32 = 960;

/// How long a signal or a state that must come may take.
const WAIT: Duration = Duration::from_secs(3);
/// How long a signal that mustn't come is waited for.
const QUIET: Duration = Duration::from_millis(400);

const CHIP: &str = "fakeboardchip";
const CHIP_DEVICE: &str = "devices/platform/fakeboardchip.2592";

fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("dbus")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a directory");
    dir
}

async fn next<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await
}

/// Both futures' outputs, run side by side.
async fn both<A: Future, B: Future>(a: A, b: B) -> (A::Output, B::Output) {
    let (mut a, mut b) = (pin!(a), pin!(b));
    let (mut left, mut right) = (None, None);
    poll_fn(|cx| {
        if left.is_none()
            && let Poll::Ready(out) = a.as_mut().poll(cx)
        {
            left = Some(out);
        }
        if right.is_none()
            && let Poll::Ready(out) = b.as_mut().poll(cx)
        {
            right = Some(out);
        }
        if left.is_some() && right.is_some() {
            Poll::Ready((left.take().expect("a"), right.take().expect("b")))
        } else {
            Poll::Pending
        }
    })
    .await
}

/// polkit, faked: every caller passes unless refused; every question asked is kept.
#[derive(Clone, Default)]
struct FakePolkit {
    refused: Arc<Mutex<HashSet<String>>>,
    asked: Arc<Mutex<Vec<(String, Action)>>>,
}

impl FakePolkit {
    fn refuse(&self, name: &str) {
        self.refused
            .lock()
            .expect("refused")
            .insert(name.to_owned());
    }

    fn allow(&self, name: &str) {
        self.refused.lock().expect("refused").remove(name);
    }

    fn asked(&self, name: &str) -> Vec<Action> {
        self.asked
            .lock()
            .expect("asked")
            .iter()
            .filter(|(sender, _)| sender == name)
            .map(|(_, action)| *action)
            .collect()
    }
}

impl Authorizer for FakePolkit {
    fn allowed<'a>(&'a self, sender: &'a str, action: Action) -> Answer<'a, bool> {
        Box::pin(async move {
            self.asked
                .lock()
                .expect("asked")
                .push((sender.to_owned(), action));
            Ok(!self.refused.lock().expect("refused").contains(sender))
        })
    }
}

/// The bus's credentials, faked: user [`USER`] in group [`GROUP`], unless a caller is given
/// another identity.
#[derive(Clone, Default)]
struct FakeCredentials {
    given: Arc<Mutex<HashMap<String, Identity>>>,
}

impl FakeCredentials {
    fn give(&self, name: &str, identity: Identity) {
        self.given
            .lock()
            .expect("given")
            .insert(name.to_owned(), identity);
    }
}

impl Credentials for FakeCredentials {
    fn of<'a>(&'a self, sender: &'a str) -> Answer<'a, Identity> {
        Box::pin(async move {
            Ok(self
                .given
                .lock()
                .expect("given")
                .get(sender)
                .cloned()
                .unwrap_or(Identity {
                    user: Some(USER),
                    groups: Some(vec![GROUP]),
                    pid: Some(PID),
                }))
        })
    }
}

fn value(n: i64) -> Reading {
    Reading::Value(Exact::whole(n))
}

/// Readings with a distinct value per metric, and `gpu_usage` for Gpu usage.
fn readings(gpu_usage: Reading) -> Snapshot {
    Snapshot::new(
        |metric| match metric {
            Metric::GpuUsage => gpu_usage.clone(),
            other => value(11 + 10 * other as i64),
        },
        Fan::Chosen(Reading::Value(Exact::new(24_685, 20).expect("a fraction"))),
        Fan::NotChosen,
    )
}

fn gpus() -> Vec<Gpu> {
    vec![
        Gpu {
            pci: PciAddr::from_str("0000:0a:00.0").expect("an address"),
            driver: "amdgpu".to_owned(),
            kind: GpuKind::Dedicated,
            vendor: Some(0x0abc),
            device: Some(0x0def),
        },
        Gpu {
            pci: PciAddr::from_str("0000:00:02.0").expect("an address"),
            driver: "i915".to_owned(),
            kind: GpuKind::Integrated,
            vendor: None,
            device: None,
        },
    ]
}

fn fan_inputs() -> Vec<FanInput> {
    (1..=2)
        .map(|input| FanInput {
            chip: CHIP.to_owned(),
            device: PathBuf::from(CHIP_DEVICE),
            input,
            label: None,
        })
        .collect()
}

/// Every metric bound but CPU Power, which this account may not read.
fn bindings() -> Vec<(Metric, Binding)> {
    Metric::ALL
        .into_iter()
        .map(|metric| {
            let binding = if metric == Metric::CpuPower {
                Binding::Unbound(Unavailable::PermissionDenied {
                    path: PathBuf::from("/fake/energy_uj"),
                })
            } else {
                Binding::Bound(format!("fake {}", metric.spec_name()))
            };
            (metric, binding)
        })
        .collect()
}

fn status(snapshot: Snapshot) -> Status {
    Status {
        link: Some(Link::Connected),
        availability: availability(&snapshot, true),
        bindings: bindings(),
        gpus: gpus(),
        fan_inputs: fan_inputs(),
        snapshot: Some(snapshot),
        ..Status::default()
    }
}

/// `OverrideReadings`' two maps: exact values, and unavailable metrics' reason kinds.
type Values = HashMap<String, (i64, u64)>;
type Reasons = HashMap<String, String>;

/// A client: its connection and both proxies.
struct Client {
    conn: Connection,
    main: OpenDeepCoolProxy<'static>,
    dev: DevProxy<'static>,
}

impl Client {
    async fn on(bus: &PrivateBus) -> Client {
        let conn = connection::Builder::address(bus.address())
            .build()
            .await
            .expect("a connection");
        Client {
            main: OpenDeepCoolProxy::new(&conn).await.expect("a proxy"),
            dev: DevProxy::new(&conn).await.expect("a proxy"),
            conn,
        }
    }

    fn name(&self) -> String {
        self.conn.unique_name().expect("a unique name").to_string()
    }
}

/// The D-Bus side alone: the test reads what it hands the engine and the device thread, and
/// writes the status itself.
struct Harness {
    /// First, so it stops before the bus does; kept only for that.
    _side: Side,
    bus: PrivateBus,
    polkit: FakePolkit,
    credentials: FakeCredentials,
    state: PathBuf,
    apply: Arc<Slot<Apply>>,
    device: Receiver<DeviceControl>,
    /// What the sensor thread would be sent.
    sensors: Receiver<SensorRequest>,
    /// The engine's frames for `Frame` watchers, which the test posts itself.
    frames: Arc<Slot<Frame>>,
    frame_pulse: Arc<Pulse>,
    journal: Arc<Recorded>,
    /// The threads' durations, which the test writes itself.
    durations: Arc<Durations>,
    /// The sound the engine would hear.
    audio: Arc<PipeAudio>,
    /// The status, which the test writes in the engine's place.
    status: Arc<RwLock<Status>>,
}

impl Harness {
    fn start(name: &str, settings: Settings, status: Status, dev: bool) -> Harness {
        Harness::drawing(name, settings, status, dev, draws)
    }

    /// [`Harness::start`], drawing the modes `drawn` lets it: the test-only switch (Phase 9 plan,
    /// Task 3).
    fn drawing(name: &str, settings: Settings, status: Status, dev: bool, drawn: Drawn) -> Harness {
        let dir = dir(name);
        let bus = PrivateBus::start(&dir.join("bus")).expect("a private bus");
        let state = dir.join("state");
        fs::create_dir_all(&state).expect("a state directory");
        let store = SettingsStore::new(&state);
        store.store(&settings).expect("the settings saved");
        let recording = Recording::dev(
            FakeConnector::new([Presence::Present]),
            dir.join("dev-frames.log"),
            u64::MAX,
        )
        .expect("a recording");
        let (polkit, credentials) = (FakePolkit::default(), FakeCredentials::default());
        let apply = Arc::new(Slot::default());
        let status = Arc::new(RwLock::new(status));
        let journal = Arc::new(Recorded::default());
        let (device_tx, device) = mpsc::channel();
        let (sensors_tx, sensors) = mpsc::channel();
        let (frames, frame_pulse) = (Arc::new(Slot::default()), Arc::new(Pulse::default()));
        let durations = Arc::new(Durations::default());
        let audio = Arc::new(PipeAudio::new());
        let side = Side::start(Parts {
            bus: Bus::Address(bus.address().to_owned()),
            checks: Checks::Given {
                authorizer: Box::new(polkit.clone()),
                credentials: Box::new(credentials.clone()),
            },
            store,
            settings,
            status: Arc::clone(&status),
            apply: Arc::clone(&apply),
            device: device_tx,
            sensors: sensors_tx,
            frames: Arc::clone(&frames),
            frame_pulse: Arc::clone(&frame_pulse),
            pulse: Arc::new(Pulse::default()),
            journal: Arc::clone(&journal) as Arc<dyn Journal>,
            dev: dev.then(|| (GROUP, recording.handle(), Arc::clone(&durations))),
            audio: Arc::clone(&audio),
            drawn,
        })
        .expect("the D-Bus side starts");
        Harness {
            _side: side,
            bus,
            polkit,
            credentials,
            state,
            apply,
            device,
            sensors,
            frames,
            frame_pulse,
            journal,
            durations,
            audio,
            status,
        }
    }

    async fn client(&self) -> Client {
        Client::on(&self.bus).await
    }

    /// The settings file's inode and text: a store renames a new file over it.
    fn saved(&self) -> (u64, String) {
        let path = self.state.join("settings.toml");
        let inode = fs::metadata(&path).expect("the settings file").ino();
        (inode, fs::read_to_string(&path).expect("the settings file"))
    }

    /// Hands the D-Bus side a frame, as the engine does after each tick.
    fn frame(&self, frame: Frame) {
        self.frames.post(frame);
        self.frame_pulse.send();
    }

    fn saved_settings(&self) -> Settings {
        toml::from_str(&self.saved().1).expect("settings")
    }

    /// Nothing was stored since `before`, and nothing handed to the engine.
    fn nothing_stored_or_applied(&self, before: &(u64, String)) {
        assert_eq!(self.saved(), *before, "nothing stored");
        assert_eq!(self.apply.take(), None, "nothing applied");
        assert_eq!(self.sent(), [], "nothing sent to the sensor thread");
    }

    /// What the sensor thread was sent since the last look, in order.
    fn sent(&self) -> Vec<SensorRequest> {
        self.sensors.try_iter().collect()
    }

    fn journal_has(&self, part: &str) -> bool {
        self.journal
            .lines()
            .iter()
            .any(|(_, line)| line.contains(part))
    }
}

/// `StateChanged`'s keys, sorted.
fn keys(changes: &HashMap<String, OwnedVariant>) -> Vec<String> {
    let mut keys: Vec<String> = changes.keys().cloned().collect();
    keys.sort();
    keys
}

#[test]
fn a_change_is_stored_applied_and_sent_to_each_watcher_alone() {
    let h = Harness::start(
        "change",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (one, two, other, changer) = (
            h.client().await,
            h.client().await,
            h.client().await,
            h.client().await,
        );
        one.main.watch().await.expect("watches");
        two.main.watch().await.expect("watches");
        let mut first = one.main.receive_state_changed().await.expect("a stream");
        let mut second = two.main.receive_state_changed().await.expect("a stream");
        // A match rule for StateChanged, but no Watch().
        let mut unwatched = other.main.receive_state_changed().await.expect("a stream");
        changer.main.set_mode(3).await.expect("mode 3");
        assert_eq!(h.saved_settings().mode, 3, "stored");
        let applied = h.apply.take().expect("handed to the engine");
        assert_eq!(applied.settings.map(|s| s.mode), Some(3));
        for stream in [&mut first, &mut second] {
            let signal = within(WAIT, next(stream))
                .await
                .flatten()
                .expect("StateChanged");
            let changes = signal.args().expect("its changes").changes;
            assert_eq!(keys(&changes), ["Mode"]);
            assert_eq!(State::from_dict(&changes).expect("a state").mode, Some(3));
        }
        assert!(
            within(QUIET, next(&mut unwatched)).await.is_none(),
            "a connection that doesn't watch gets nothing"
        );
        assert_eq!(h.polkit.asked(&changer.name()), [Action::ChangeSettings]);
        assert_eq!(h.polkit.asked(&one.name()), [Action::Use]);
    });
}

#[test]
fn a_refused_caller_changes_nothing() {
    let h = Harness::start(
        "refused",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        h.polkit.refuse(&caller.name());
        let before = h.saved();
        assert!(matches!(
            caller.main.set_mode(3).await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            caller.main.refresh().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            caller.main.rotate_screen().await,
            Err(Error::NotAuthorized(_))
        ));
        h.nothing_stored_or_applied(&before);
        assert!(h.journal_has(&format!("SetMode(3) from {} refused", caller.name())));
    });
}

#[test]
fn modes_are_checked_against_the_table_this_build_and_the_readings() {
    // Gpu usage has had no device for longer than the first two minutes.
    let no_gpu = status(readings(Reading::Unavailable(Unavailable::NoDevice)));
    let h = Harness::start("modes", Settings::default(), no_gpu, false);
    zbus::block_on(async {
        let caller = h.client().await;
        let before = h.saved();
        // A mode outside the table (Phase 9 plan, Task 9).
        for mode in [0, 24, 100, 70_000, u32::MAX] {
            assert!(
                matches!(
                    caller.main.set_mode(mode).await,
                    Err(Error::InvalidArgument(_))
                ),
                "{mode}"
            );
        }
        match caller.main.set_mode(2).await {
            Err(Error::ModeUnavailable(why)) => {
                assert_eq!(why, "Gpu usage: no device found");
            }
            other => panic!("{other:?}"),
        }
        h.nothing_stored_or_applied(&before);
        // Phase 8 plan, Task 4: Temperature · CPU is drawn, and its reading is there.
        caller.main.set_mode(7).await.expect("mode 7");
        assert_eq!(h.saved_settings().mode, 7, "stored");
        let applied = h.apply.take().expect("handed to the engine");
        assert_eq!(applied.settings.map(|s| s.mode), Some(7));
        // Phase 9 plan, Task 9: the Dancer is drawn last, so every mode in the table is drawn and
        // none is refused as not yet available; only a reading can still refuse one.
        caller.main.set_mode(23).await.expect("mode 23");
        for mode in Mode::all() {
            let answer = caller.main.set_mode(u32::from(mode.number())).await;
            assert!(
                !matches!(answer, Err(Error::NotYetAvailable(_))),
                "mode {}: {answer:?}",
                mode.number()
            );
        }
    });
}

// Phase 8 plan, Task 7: Power · CPU is drawn and selectable; a unit change in it is stored and
// announced, and the engine gets the new settings, whose unit no Power picture reads (the engine's
// scenarios prove no frame changes, S15).
#[test]
fn mode_10_is_selectable_and_a_unit_change_in_it_is_only_stored() {
    let h = Harness::start(
        "power",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (watcher, caller) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        caller.main.set_mode(10).await.expect("mode 10");
        assert_eq!(h.saved_settings().mode, 10, "stored");
        let applied = h.apply.take().and_then(|a| a.settings);
        assert_eq!(applied.map(|s| s.mode), Some(10));
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("StateChanged");
        assert_eq!(keys(&signal.args().expect("its changes").changes), ["Mode"]);
        caller
            .main
            .set_temperature_unit("fahrenheit")
            .await
            .expect("stored");
        let saved = h.saved_settings();
        assert_eq!(
            (saved.mode, saved.temperature_unit),
            (10, TemperatureUnit::Fahrenheit)
        );
        let applied = h
            .apply
            .take()
            .and_then(|a| a.settings)
            .expect("the settings");
        assert_eq!(
            (applied.mode, applied.temperature_unit),
            (10, TemperatureUnit::Fahrenheit)
        );
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("StateChanged");
        assert_eq!(
            keys(&signal.args().expect("its changes").changes),
            ["TemperatureUnit"]
        );
    });
}

#[test]
fn arguments_outside_what_the_service_found_are_refused() {
    let h = Harness::start(
        "arguments",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let before = h.saved();
        let long = "x".repeat(65);
        let long_path = format!("devices/{}", "y".repeat(1020));
        let refusals = [
            caller.main.set_rotation(4).await,
            caller.main.set_temperature_unit("kelvin").await,
            caller.main.set_temperature_unit(&long).await,
            caller.main.choose_gpu("0000:0c:00.0").await,
            caller.main.choose_gpu(" ").await,
            caller.main.choose_gpu(&long).await,
            caller.main.set_cooler_fan_input(CHIP, CHIP_DEVICE, 3).await,
            caller
                .main
                .set_cooler_fan_input(CHIP, "../../etc/shadow", 1)
                .await,
            caller.main.set_cooler_fan_input(CHIP, &long_path, 1).await,
            caller
                .main
                .set_cooler_fan_input(&long, CHIP_DEVICE, 1)
                .await,
            caller
                .main
                .set_pump_input("otherchip", CHIP_DEVICE, 1)
                .await,
        ];
        for (i, refusal) in refusals.into_iter().enumerate() {
            assert!(
                matches!(refusal, Err(Error::InvalidArgument(_))),
                "{i}: {refusal:?}"
            );
        }
        h.nothing_stored_or_applied(&before);
    });
}

#[test]
fn the_choices_listed_are_stored() {
    let h = Harness::start(
        "choices",
        Settings {
            rotation: 3,
            ..Settings::default()
        },
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        caller.main.rotate_screen().await.expect("turned");
        assert_eq!(h.saved_settings().rotation, 0, "3 wraps to 0");
        caller
            .main
            .choose_gpu("0000:00:02.0")
            .await
            .expect("chosen");
        caller
            .main
            .set_cooler_fan_input(CHIP, CHIP_DEVICE, 1)
            .await
            .expect("chosen");
        caller
            .main
            .set_pump_input(CHIP, CHIP_DEVICE, 2)
            .await
            .expect("chosen");
        caller
            .main
            .set_temperature_unit("fahrenheit")
            .await
            .expect("saved");
        let saved = h.saved_settings();
        assert_eq!(saved.gpu_choice.as_deref(), Some("0000:00:02.0"));
        assert_eq!(
            saved.pump_input,
            Some(SavedInput {
                chip: CHIP.to_owned(),
                device_path: CHIP_DEVICE.to_owned(),
                input: 2,
            })
        );
        assert_eq!(saved.temperature_unit, TemperatureUnit::Fahrenheit);
        // All empty: none.
        caller.main.set_pump_input("", "", 0).await.expect("none");
        assert_eq!(h.saved_settings().pump_input, None);
        let state =
            State::from_dict(&caller.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(state.chosen_gpu.as_deref(), Some("0000:00:02.0"));
        assert_eq!(state.pump_input, Some(None));
        assert_eq!(state.cooler_fan_input.flatten().map(|i| i.input), Some(1));
        // Empty: back to Automatic (owner, 2026-11-04).
        caller.main.choose_gpu("").await.expect("automatic");
        assert_eq!(h.saved_settings().gpu_choice, None);
        let state =
            State::from_dict(&caller.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(state.chosen_gpu.as_deref(), Some(""));
    });
}

#[test]
fn choosing_automatic_while_no_gpu_is_chosen_changes_nothing() {
    let h = Harness::start(
        "automatic",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (watcher, caller) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let before = h.saved();
        caller.main.choose_gpu("").await.expect("success");
        h.nothing_stored_or_applied(&before);
        assert!(
            within(QUIET, next(&mut changes)).await.is_none(),
            "no signal"
        );
    });
}

#[test]
fn choosing_what_is_already_set_changes_nothing() {
    let settings = Settings {
        mode: 3,
        rotation: 2,
        temperature_unit: TemperatureUnit::Fahrenheit,
        gpu_choice: Some("0000:0a:00.0".to_owned()),
        cooler_fan_input: Some(SavedInput {
            chip: CHIP.to_owned(),
            device_path: CHIP_DEVICE.to_owned(),
            input: 1,
        }),
        ..Settings::default()
    };
    let h = Harness::start("already", settings, status(readings(value(51))), false);
    zbus::block_on(async {
        let (watcher, caller) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let before = h.saved();
        caller.main.set_mode(3).await.expect("success");
        caller.main.set_rotation(2).await.expect("success");
        caller
            .main
            .set_temperature_unit("fahrenheit")
            .await
            .expect("success");
        caller
            .main
            .choose_gpu("0000:0a:00.0")
            .await
            .expect("success");
        caller
            .main
            .set_cooler_fan_input(CHIP, CHIP_DEVICE, 1)
            .await
            .expect("success");
        caller
            .main
            .set_pump_input("", "", 0)
            .await
            .expect("success");
        h.nothing_stored_or_applied(&before);
        assert!(
            within(QUIET, next(&mut changes)).await.is_none(),
            "no signal"
        );
        // A real change still reaches the watcher.
        caller.main.set_rotation(1).await.expect("rotation 1");
        assert!(within(WAIT, next(&mut changes)).await.flatten().is_some());
    });
}

// `docs/spec/dbus.md`, Errors: a settings file the service can't write answers `…Error.Failed`,
// which the proxy turns into `Error::Failed` by its name alone. A plain file where the state
// directory was fails every write, even root's, as in CI's container.
#[test]
fn a_settings_file_that_cant_be_written_changes_nothing() {
    let h = Harness::start(
        "unwritable",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    fs::remove_dir_all(&h.state).expect("the state directory goes");
    fs::write(&h.state, "").expect("a plain file in its place");
    zbus::block_on(async {
        let caller = h.client().await;
        match caller.main.set_mode(3).await {
            Err(Error::Failed(text)) => {
                assert!(text.starts_with("the settings can't be saved: "), "{text}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(h.apply.take(), None, "nothing applied");
        let state =
            State::from_dict(&caller.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(state.mode, Some(1), "nothing changed");
        assert!(h.journal_has("the settings can't be saved"));
    });
}

#[test]
fn refresh_is_handed_to_the_engine_and_saves_nothing() {
    let h = Harness::start(
        "refresh",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let before = h.saved();
        caller.main.refresh().await.expect("refreshed");
        let applied = h.apply.take().expect("handed to the engine");
        assert!(applied.refresh);
        assert_eq!(applied.settings, None);
        assert_eq!(h.saved(), before);
    });
}

// S26 (owner, 2026-11-06): Refresh asks the sensor thread for its snapshot as the call arrives,
// so the answer is there before the engine's next tick, which takes the Refresh and resets on it:
// the picture at home on the next frame, with the fresh readings. Each Refresh asks anew.
#[test]
fn refresh_asks_the_sensor_thread_at_once_and_resets_on_the_next_frame() {
    let h = Harness::start(
        "refresh-asks",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        caller.main.refresh().await.expect("refreshed");
        let applied = h.apply.take().expect("handed to the engine");
        let asked = applied.asked.expect("a snapshot request");
        assert_eq!(h.sent(), [SensorRequest::Snapshot(asked)]);
        // The sensor thread's answer, then the engine's next tick, mid-scroll at 100%.
        let mut sensors = SensorLoop::new(FixedSensors(readings(value(101))));
        assert!(sensors.handle(SensorRequest::Snapshot(asked)), "at once");
        let answer = sensors.snapshot();
        let mut engine = Engine::new(Settings::default(), readings(value(100)));
        for _ in 0..5 {
            engine.tick(None, None, false);
        }
        let next = engine.tick(Some(applied), Some(answer), false);
        assert_eq!(next.frame, Some(home(readings(value(101)))));
        caller.main.refresh().await.expect("refreshed");
        let again = h.apply.take().expect("handed to the engine");
        assert!(again.asked > Some(asked), "a new request");
        assert_eq!(
            h.sent(),
            [SensorRequest::Snapshot(again.asked.expect("one"))]
        );
    });
}

// S25 (owner, 2026-11-06): another GPU reaches the sensor thread as the call arrives, then a
// request for a snapshot of it, whose number the engine waits for; a fan input reaches it with no
// snapshot, since it resets nothing.
#[test]
fn another_gpu_reaches_the_sensor_thread_at_once_with_a_snapshot_request() {
    let h = Harness::start(
        "gpu-asks",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        caller
            .main
            .choose_gpu("0000:00:02.0")
            .await
            .expect("chosen");
        let applied = h.apply.take().expect("handed to the engine");
        let settings = applied.settings.expect("the new settings");
        assert_eq!(settings.gpu_choice.as_deref(), Some("0000:00:02.0"));
        let asked = applied.asked.expect("a snapshot request");
        assert_eq!(
            h.sent(),
            [
                SensorRequest::Choices(Choices::of(&settings)),
                SensorRequest::Snapshot(asked)
            ],
            "the GPU first, so the answer reads it"
        );
        caller
            .main
            .set_pump_input(CHIP, CHIP_DEVICE, 2)
            .await
            .expect("chosen");
        let applied = h.apply.take().expect("handed to the engine");
        assert_eq!(applied.asked, None);
        let settings = applied.settings.expect("the new settings");
        assert_eq!(h.sent(), [SensorRequest::Choices(Choices::of(&settings))]);
        // Another setting: nothing for the sensor thread.
        caller.main.set_rotation(1).await.expect("turned");
        assert_eq!(h.apply.take().and_then(|a| a.asked), None);
        assert_eq!(h.sent(), []);
    });
}

#[test]
fn reads_need_polkit_s_use_and_a_refused_caller_registers_nothing() {
    let h = Harness::start(
        "reads",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (refused, changer) = (h.client().await, h.client().await);
        h.polkit.refuse(&refused.name());
        let mut changes = refused
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        assert!(matches!(
            refused.main.get_state().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            refused.main.watch().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            refused.main.diagnostics().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            refused.main.unwatch().await,
            Err(Error::NotAuthorized(_))
        ));
        assert_eq!(h.polkit.asked(&refused.name()), [Action::Use; 4]);
        // It wasn't registered: a change sends it nothing, even once polkit lets it read.
        h.polkit.allow(&refused.name());
        changer.main.set_mode(3).await.expect("mode 3");
        assert!(within(QUIET, next(&mut changes)).await.is_none());
    });
}

#[test]
fn get_state_returns_every_key_of_the_contract_with_its_type() {
    let h = Harness::start(
        "state",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let dict = caller.main.get_state().await.expect("the state");
        let mut got: Vec<(String, String)> = dict
            .iter()
            .map(|(key, value)| (key.clone(), value.value_signature().to_string()))
            .collect();
        got.sort();
        let mut want: Vec<(String, String)> = STATE_KEYS
            .iter()
            .map(|(key, signature)| ((*key).to_owned(), (*signature).to_owned()))
            .collect();
        want.sort();
        assert_eq!(got, want);
        let state = State::from_dict(&dict).expect("a state");
        assert_eq!(state.mode, Some(1));
        assert_eq!(state.cooler_state, Some(CoolerState::Connected));
        assert_eq!(state.chosen_gpu.as_deref(), Some(""));
        // 24 685 / 20 = 1234.25 RPM, drawn as 1234.
        assert_eq!(state.cooler_fan_rpm, Some(odc_ipc::FanSpeed::Rpm(1234)));
        assert_eq!(state.pump_rpm, Some(odc_ipc::FanSpeed::Unavailable));
        let gpus = state.gpus.expect("Gpus");
        assert_eq!(gpus[0].pci, "0000:0a:00.0");
        assert_eq!(gpus[0].kind, odc_ipc::GpuKind::Dedicated);
        let inputs = state.fan_inputs.expect("FanInputs");
        assert_eq!(
            (inputs[1].device.as_str(), inputs[1].input),
            (CHIP_DEVICE, 2)
        );
    });
}

#[test]
fn diagnostics_list_each_metric_with_its_source_and_state() {
    let mut readings = readings(value(51));
    readings = Snapshot::new(
        |metric| match metric {
            Metric::CpuPower => Reading::Unavailable(Unavailable::PermissionDenied {
                path: PathBuf::from("/fake/energy_uj"),
            }),
            other => readings.get(other).clone(),
        },
        Fan::NotChosen,
        Fan::NotChosen,
    );
    let h = Harness::start("diagnostics", Settings::default(), status(readings), false);
    zbus::block_on(async {
        let caller = h.client().await;
        let list = caller.main.diagnostics().await.expect("the list");
        let names: Vec<&str> = list.iter().map(|(name, _, _)| name.as_str()).collect();
        let want: Vec<&str> = Metric::ALL.iter().map(|m| m.spec_name()).collect();
        assert_eq!(names, want, "all seven, in the spec's order");
        assert_eq!(
            list[0],
            (
                "CPU Temperature".to_owned(),
                "fake CPU Temperature".to_owned(),
                "ok".to_owned()
            )
        );
        assert_eq!(
            list[2],
            (
                "CPU Power".to_owned(),
                String::new(),
                "no permission to read it".to_owned()
            )
        );
    });
}

/// The members of `interface` in an introspection document, each on one line with its
/// arguments, sorted: `<x/>` and `<x></x>` alike, whitespace and attribute order ignored.
fn members(xml: &str, interface: &str) -> Vec<String> {
    let start = xml
        .find(&format!("<interface name=\"{interface}\">"))
        .unwrap_or_else(|| panic!("{interface} in {xml}"));
    let body = &xml[start..];
    let body = &body[..body.find("</interface>").expect("the interface's end")];
    let mut members = Vec::new();
    let mut current: Option<String> = None;
    for tag in body.split('<').skip(1) {
        let tag = tag.split('>').next().expect("a tag").trim();
        let closing = tag.starts_with('/');
        let empty = tag.ends_with('/');
        let tag = tag.trim_matches('/').trim();
        let (name, rest) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
        let attributes: HashMap<&str, &str> = rest
            .split('"')
            .collect::<Vec<_>>()
            .chunks(2)
            .filter_map(|pair| match pair {
                [key, value] => Some((key.trim().trim_end_matches('='), *value)),
                _ => None,
            })
            .collect();
        match (name, closing) {
            ("method" | "signal" | "property", false) => {
                let line = format!("{name} {}", attributes["name"]);
                if empty {
                    members.push(line);
                } else {
                    current = Some(line);
                }
            }
            ("method" | "signal" | "property", true) => members.extend(current.take()),
            ("arg", _) => {
                let line = current.as_mut().expect("an argument inside a member");
                line.push_str(&format!(
                    " [{} {} {}]",
                    attributes.get("direction").unwrap_or(&"-"),
                    attributes.get("name").unwrap_or(&"-"),
                    attributes["type"]
                ));
            }
            (other, _) => assert!(
                matches!(other, "interface" | "!--"),
                "an unexpected element {other} in {interface}"
            ),
        }
    }
    members.sort();
    members
}

fn dbus_md() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/spec/dbus.md");
    fs::read_to_string(path).expect("docs/spec/dbus.md")
}

async fn introspect(conn: &Connection, path: &str) -> String {
    zbus::fdo::IntrospectableProxy::builder(conn)
        .destination(BUS_NAME)
        .path(path)
        .build()
        .await
        .expect("a proxy")
        .introspect()
        .await
        .expect("the introspection")
}

#[test]
fn the_introspection_is_the_contract_and_lists_no_properties() {
    let h = Harness::start(
        "contract",
        Settings::default(),
        status(readings(value(51))),
        true,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let main = introspect(&caller.conn, OBJECT_PATH).await;
        let documented = dbus_md();
        assert_eq!(members(&main, INTERFACE), members(&documented, INTERFACE));
        assert!(
            !members(&main, INTERFACE)
                .iter()
                .any(|m| m.starts_with("property")),
            "no properties"
        );
        let dev = introspect(&caller.conn, DEV_PATH).await;
        assert_eq!(
            members(&dev, DEV_INTERFACE),
            members(&documented, DEV_INTERFACE)
        );
    });
}

#[test]
fn without_dev_the_dev_interface_and_its_path_do_not_exist() {
    let h = Harness::start(
        "no-dev",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let root = introspect(&caller.conn, OBJECT_PATH).await;
        assert!(!root.contains(DEV_INTERFACE));
        assert!(!root.contains("<node name=\"Dev\""), "{root}");
        match caller.dev.begin_test().await {
            Err(Error::ZBus(zbus::Error::FDO(e))) => {
                assert!(matches!(*e, zbus::fdo::Error::UnknownObject(_)), "{e:?}");
            }
            other => panic!("{other:?}"),
        }
    });
}

#[test]
fn the_dev_interface_takes_only_the_service_s_group() {
    let h = Harness::start(
        "dev-group",
        Settings::default(),
        status(readings(value(51))),
        true,
    );
    zbus::block_on(async {
        let (outsider, unknown, member) = (h.client().await, h.client().await, h.client().await);
        h.credentials.give(
            &outsider.name(),
            Identity {
                user: Some(USER),
                groups: Some(vec![GROUP + 1]),
                pid: Some(PID),
            },
        );
        h.credentials.give(
            &unknown.name(),
            Identity {
                user: Some(USER),
                groups: None,
                pid: Some(PID),
            },
        );
        assert!(matches!(
            outsider.dev.begin_test().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(
            matches!(unknown.dev.begin_test().await, Err(Error::NotAuthorized(_))),
            "a missing group list means no"
        );
        // A method call without an interface field (dbus-daemon(1): the field is optional).
        let raw = outsider
            .conn
            .call_method(Some(BUS_NAME), DEV_PATH, None::<&str>, "BeginTest", &())
            .await;
        assert!(raw.is_err(), "{raw:?}");
        // No session began: a member of the group takes it.
        member.dev.begin_test().await.expect("the session");
        assert!(h.journal_has(&format!("held by {}", member.name())));
    });
}

#[test]
fn the_test_session_holds_the_service_until_its_holder_leaves() {
    let h = Harness::start(
        "session",
        Settings::default(),
        status(readings(value(51))),
        true,
    );
    zbus::block_on(async {
        let (holder, other) = (h.client().await, h.client().await);
        let values = HashMap::from([("CPU Usage".to_owned(), (42, 1))]);
        let none = HashMap::new();
        // Without a session.
        assert!(matches!(
            holder
                .dev
                .override_readings(values.clone(), none.clone())
                .await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            holder.dev.clear_override().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            holder.dev.release_cooler().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            holder.dev.end_test().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            holder.dev.durations().await,
            Err(Error::NotAuthorized(_))
        ));
        // A session starts the durations afresh (owner, 2026-11-04).
        h.durations.write(Duration::from_micros(9));
        h.durations.snapshot(Duration::from_micros(9));
        holder.dev.begin_test().await.expect("the session");
        holder.dev.begin_test().await.expect("again: no change");
        h.durations.write(Duration::from_micros(1_234));
        h.durations.snapshot(Duration::from_millis(56));
        assert_eq!(
            holder.dev.durations().await.expect("the durations"),
            (vec![1_234], vec![56_000])
        );
        assert!(matches!(other.dev.durations().await, Err(Error::Busy(_))));
        // Everyone else is refused.
        assert!(matches!(other.main.set_mode(3).await, Err(Error::Busy(_))));
        assert!(matches!(other.main.refresh().await, Err(Error::Busy(_))));
        assert!(matches!(other.dev.begin_test().await, Err(Error::Busy(_))));
        assert!(matches!(
            other
                .dev
                .override_readings(values.clone(), none.clone())
                .await,
            Err(Error::Busy(_))
        ));
        assert!(matches!(
            other.dev.release_cooler().await,
            Err(Error::Busy(_))
        ));
        assert!(matches!(other.dev.end_test().await, Err(Error::Busy(_))));
        // Reads aren't changes.
        other.main.get_state().await.expect("the state");
        // The holder may change.
        holder.main.set_mode(3).await.expect("mode 3");
        assert_eq!(
            h.apply.take().and_then(|a| a.settings).map(|s| s.mode),
            Some(3)
        );
        // Overrides out of form.
        let all = |except: Option<Metric>| -> HashMap<String, (i64, u64)> {
            Metric::ALL
                .into_iter()
                .filter(|m| Some(*m) != except)
                .map(|m| (m.spec_name().to_owned(), (40 + m as i64, 1)))
                .collect()
        };
        let bad: Vec<(Values, Reasons)> = vec![
            (all(Some(Metric::MemoryLoad)), HashMap::new()),
            (
                all(None),
                HashMap::from([("Memory Load".to_owned(), "no-device".to_owned())]),
            ),
            (
                {
                    let mut v = all(Some(Metric::MemoryLoad));
                    v.insert("RAM".to_owned(), (5, 1));
                    v
                },
                HashMap::new(),
            ),
            (
                all(Some(Metric::MemoryLoad)),
                HashMap::from([("Memory Load".to_owned(), "gone".to_owned())]),
            ),
            (
                {
                    let mut v = all(None);
                    v.insert("CPU Usage".to_owned(), (5, 0));
                    v
                },
                HashMap::new(),
            ),
        ];
        for (i, (values, unavailable)) in bad.into_iter().enumerate() {
            assert!(
                matches!(
                    holder.dev.override_readings(values, unavailable).await,
                    Err(Error::InvalidArgument(_))
                ),
                "{i}"
            );
        }
        assert_eq!(h.apply.take(), None, "no override posted");
        // The holder's connection closes, as when the test is killed.
        let gone = holder.name();
        drop(holder);
        let mut passed = false;
        for _ in 0..30 {
            if other.main.set_mode(2).await.is_ok() {
                passed = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        assert!(passed, "changes pass again once the holder left");
        assert!(h.journal_has(&format!("the test session of {gone} ended")));
        let cleared = h
            .apply
            .take()
            .and_then(|a| a.readings)
            .expect("an override's end");
        assert_eq!(cleared.readings, None);
    });
}

#[test]
fn watchers_are_capped_per_user_and_in_all_and_dropped_when_they_leave() {
    let h = Harness::start(
        "caps",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let mut clients = Vec::new();
        for _ in 0..33 {
            clients.push(h.client().await);
        }
        // The first nine share a user; the rest have one each.
        for (i, client) in clients.iter().enumerate() {
            let user = if i < 9 { 2000 } else { 3000 + i as u32 };
            h.credentials.give(
                &client.name(),
                Identity {
                    user: Some(user),
                    groups: Some(vec![]),
                    pid: Some(PID),
                },
            );
        }
        for client in &clients[..8] {
            client.main.watch().await.expect("watches");
        }
        clients[0]
            .main
            .watch()
            .await
            .expect("a second Watch() changes nothing");
        assert!(matches!(
            clients[8].main.watch().await,
            Err(Error::TooManyWatchers(_))
        ));
        for client in &clients[9..33] {
            client.main.watch().await.expect("watches");
        }
        // 8 + 24 = 32 in all.
        let late = h.client().await;
        assert!(matches!(
            late.main.watch().await,
            Err(Error::TooManyWatchers(_))
        ));
        // One leaves: its place is free.
        clients.truncate(32);
        let mut passed = false;
        for _ in 0..30 {
            if late.main.watch().await.is_ok() {
                passed = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        assert!(passed, "a watcher whose connection closed is dropped");
    });
}

#[test]
fn a_watcher_that_stops_passing_is_skipped_then_gets_the_whole_state() {
    let h = Harness::start(
        "recheck",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (watcher, changer) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        h.polkit.refuse(&watcher.name());
        // The check each second sees it.
        sleep(Duration::from_millis(1300)).await;
        changer.main.set_mode(3).await.expect("mode 3");
        assert!(
            within(Duration::from_millis(600), next(&mut changes))
                .await
                .is_none(),
            "skipped while it fails"
        );
        assert!(h.journal_has(&format!("watcher {} no longer passes", watcher.name())));
        h.polkit.allow(&watcher.name());
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("the whole state");
        let changes = signal.args().expect("its changes").changes;
        // Every key in one signal, but FanInputs, which only a frame watcher gets (Phase 10).
        assert_eq!(
            changes.len(),
            STATE_KEYS.len() - 1,
            "every key in one signal"
        );
        assert!(!changes.contains_key("FanInputs"));
        assert_eq!(State::from_dict(&changes).expect("a state").mode, Some(3));
    });
}

#[test]
fn a_wake_up_comes_only_from_the_connection_that_owns_logind_s_name() {
    let h = Harness::start(
        "wake",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (logind, faker) = (h.client().await, h.client().await);
        logind
            .conn
            .request_name("org.freedesktop.login1")
            .await
            .expect("logind's name");
        let prepare = |conn: &Connection, to: Option<&'static str>, start: bool| {
            let conn = conn.clone();
            async move {
                conn.emit_signal(
                    to,
                    "/org/freedesktop/login1",
                    "org.freedesktop.login1.Manager",
                    "PrepareForSleep",
                    &start,
                )
                .await
                .expect("sent");
            }
        };
        // From another connection, broadcast or addressed to the service: nothing.
        prepare(&faker.conn, None, false).await;
        prepare(&faker.conn, Some(BUS_NAME), false).await;
        // From logind, before a sleep: nothing.
        prepare(&logind.conn, None, true).await;
        sleep(QUIET).await;
        assert!(
            h.device.try_recv().is_err(),
            "no Wake for the device thread"
        );
        assert_eq!(h.apply.take(), None, "no wake-up for the engine");
        // From logind, after a sleep.
        prepare(&logind.conn, None, false).await;
        let mut woke = false;
        for _ in 0..30 {
            if h.device.try_recv() == Ok(DeviceControl::Wake) {
                woke = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        assert!(woke, "Wake reaches the device thread");
        let applied = h.apply.take().expect("the engine is told");
        assert!(
            applied.wake,
            "the engine restarts the sensor thread's intervals"
        );
        assert!(h.device.try_recv().is_err(), "one Wake");
        assert!(h.journal_has("back from standby"));
    });
}

/// polkit's authority, faked on a peer-to-peer connection: it keeps every question and its
/// signature, and answers `allowed`.
struct FakeAuthority {
    asked: Arc<Mutex<Vec<(String, Questioned)>>>,
    allowed: bool,
}

type Questioned = (
    (String, HashMap<String, OwnedVariant>),
    String,
    HashMap<String, String>,
    u32,
    String,
);

#[zbus::interface(name = "org.freedesktop.PolicyKit1.Authority")]
impl FakeAuthority {
    async fn check_authorization(
        &self,
        #[zbus(header)] header: Header<'_>,
        subject: (String, HashMap<String, OwnedVariant>),
        action_id: String,
        details: HashMap<String, String>,
        flags: u32,
        cancellation_id: String,
    ) -> ((bool, bool, HashMap<String, String>),) {
        // zbus keeps a body's signature as the structure of its arguments.
        let signature = header.signature().to_string_no_parens();
        self.asked.lock().expect("asked").push((
            signature,
            (subject, action_id, details, flags, cancellation_id),
        ));
        ((self.allowed, false, HashMap::new()),)
    }
}

#[test]
fn polkit_is_asked_by_bus_name_for_the_action_without_a_prompt() {
    for allowed in [true, false] {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let authority = FakeAuthority {
            asked: Arc::clone(&asked),
            allowed,
        };
        zbus::block_on(async {
            let (ours, theirs) = UnixStream::pair().expect("a socket pair");
            let server = connection::Builder::unix_stream(theirs)
                .server(Guid::generate())
                .p2p()
                .serve_at("/org/freedesktop/PolicyKit1/Authority", authority)
                .build();
            let client = connection::Builder::unix_stream(ours).p2p().build();
            let (server, client) = both(server, client).await;
            let (_server, client) = (server.expect("the server"), client.expect("the client"));
            let polkit = PolkitAuthorizer::new(&client).await.expect("polkit");
            for action in [Action::Use, Action::ChangeSettings] {
                assert_eq!(polkit.allowed(":1.42", action).await, Ok(allowed));
            }
        });
        let asked = asked.lock().expect("asked");
        assert_eq!(asked.len(), 2);
        for ((signature, question), action) in
            asked.iter().zip([USE_ACTION, CHANGE_SETTINGS_ACTION])
        {
            // polkit 128's declared signature, introspected on 2026-10-29.
            assert_eq!(signature, "(sa{sv})sa{ss}us");
            let ((kind, subject), action_id, details, flags, cancellation) = question;
            assert_eq!(kind, "system-bus-name");
            assert_eq!(subject.len(), 1);
            let name: String = subject["name"]
                .try_clone()
                .expect("a string")
                .try_into()
                .expect("a string");
            assert_eq!(name, ":1.42");
            assert_eq!(action_id, action);
            assert!(details.is_empty());
            assert_eq!(*flags, 0, "no interactive prompt");
            assert_eq!(cancellation, "");
        }
    }
}

#[test]
fn the_bus_says_who_a_caller_is() {
    let dir = dir("credentials");
    let bus = PrivateBus::start(&dir.join("bus")).expect("a private bus");
    let mine = fs::metadata(&dir).expect("the directory").uid();
    zbus::block_on(async {
        let conn = connection::Builder::address(bus.address())
            .build()
            .await
            .expect("a connection");
        let credentials = BusCredentials::new(&conn).await.expect("credentials");
        let name = conn.unique_name().expect("a name").to_string();
        let identity = credentials.of(&name).await.expect("an identity");
        assert_eq!(identity.user, Some(mine));
        assert!(identity.groups.is_some_and(|g| !g.is_empty()));
    });
}

#[test]
fn every_reason_has_its_kind_and_back() {
    let reasons = [
        Unavailable::NoRule { driver: "x".into() },
        Unavailable::NoDevice,
        Unavailable::PermissionDenied {
            path: PathBuf::new(),
        },
        Unavailable::ReadFailed {
            path: PathBuf::new(),
        },
        Unavailable::NotYet,
        Unavailable::IntegratedGpuPower,
        Unavailable::UnknownGpuKind,
        Unavailable::DriverUpdated,
        Unavailable::LibraryFailed {
            library: "x".into(),
            error: "y".into(),
        },
    ];
    let kinds: HashSet<&str> = reasons.iter().map(kind_of).collect();
    assert_eq!(kinds.len(), reasons.len(), "one kind each");
    for reason in &reasons {
        let back = reason_of(kind_of(reason)).expect("its kind is taken");
        assert_eq!(kind_of(&back), kind_of(reason));
    }
    assert_eq!(reason_of("gone"), None);
}

/// Fixed readings for the whole service, every metric bound.
struct FixedSensors(Snapshot);

impl Sensors for FixedSensors {
    fn snapshot(&mut self) -> Snapshot {
        self.0.clone()
    }
    fn restart_intervals(&mut self) {}
    fn choose(&mut self, _: &Choices) {}
    fn bindings(&self) -> Vec<(Metric, Binding)> {
        bindings()
    }
}

/// The whole service, with the fake cooler, fixed readings and fake checks.
struct Service {
    /// First, so it stops before the bus does; kept only for that.
    _running: Running,
    bus: PrivateBus,
    log: FakeLog,
    journal: Arc<Recorded>,
    recording: PathBuf,
}

impl Service {
    fn start(name: &str, script: Vec<Presence>, dev: bool) -> Service {
        Service::drawing(name, script, dev, &Settings::default(), draws)
    }

    /// [`Service::start`] on `saved`, drawing the modes `drawn` lets it: the test-only switch
    /// (Phase 9 plan, Task 3).
    fn drawing(
        name: &str,
        script: Vec<Presence>,
        dev: bool,
        saved: &Settings,
        drawn: Drawn,
    ) -> Service {
        let dir = dir(name);
        let bus = PrivateBus::start(&dir.join("bus")).expect("a private bus");
        let state = dir.join("state");
        fs::create_dir_all(&state).expect("a state directory");
        let store = SettingsStore::new(&state);
        store.store(saved).expect("the settings saved");
        let (settings, _) = store.load().expect("the settings");
        let connector = FakeConnector::new(script);
        let log = connector.log();
        let recording_path = dir.join("dev-frames.log");
        let recording = Recording::dev(connector, &recording_path, u64::MAX).expect("a recording");
        let handle = recording.handle();
        let journal = Arc::new(Recorded::default());
        let running = Running::start(Config {
            settings,
            store,
            lp360: Lp360::new(recording),
            sensors: FixedSensors(readings(value(51))),
            gpus: gpus(),
            fan_inputs: fan_inputs(),
            bus: Bus::Address(bus.address().to_owned()),
            checks: Checks::Given {
                authorizer: Box::new(FakePolkit::default()),
                credentials: Box::new(FakeCredentials::default()),
            },
            journal: Arc::clone(&journal) as Arc<dyn Journal>,
            dev: dev.then_some((GROUP, handle)),
            drawn,
        })
        .expect("the service starts");
        Service {
            _running: running,
            bus,
            log,
            journal,
            recording: recording_path,
        }
    }

    fn messages(&self) -> usize {
        self.log.connections().iter().map(Vec::len).sum()
    }
}

/// Mode 1's picture for `snapshot`, at home: values under 100 stand still.
fn home(snapshot: Snapshot) -> Frame {
    Engine::new(Settings::default(), snapshot)
        .tick(None, None, false)
        .frame
        .expect("a frame")
}

#[test]
fn an_override_applies_from_the_tick_it_returns_and_a_release_hands_the_cooler_over() {
    let s = Service::start("release", vec![Presence::Present], true);
    zbus::block_on(async {
        let (holder, reader) = (Client::on(&s.bus).await, Client::on(&s.bus).await);
        holder.dev.begin_test().await.expect("the session");
        let values: HashMap<String, (i64, u64)> = Metric::ALL
            .into_iter()
            .map(|m| (m.spec_name().to_owned(), (13 + 7 * m as i64, 1)))
            .collect();
        let tick = holder
            .dev
            .override_readings(values, HashMap::new())
            .await
            .expect("the override's tick");
        let overridden =
            Snapshot::new(|m| value(13 + 7 * m as i64), Fan::NotChosen, Fan::NotChosen);
        // The recording is flushed once a second.
        sleep(Duration::from_millis(1500)).await;
        let from_tick: Vec<Frame> = read_recording(&s.recording)
            .filter(|r| r.tick >= tick)
            .map(|r| {
                match decode(r.bytes.as_slice().try_into().expect("64 bytes")).expect("a report") {
                    Message::Frame(frame) => frame,
                    Message::Startup => panic!("03 08 after the start"),
                }
            })
            .collect();
        assert!(from_tick.len() >= 5, "{}", from_tick.len());
        let want = home(overridden);
        assert!(from_tick.iter().all(|f| *f == want), "from tick {tick} on");
        // The threads timed each write and snapshot of the session (owner, 2026-11-04).
        let (writes, snapshots) = holder.dev.durations().await.expect("the durations");
        assert!(writes.len() >= from_tick.len(), "{}", writes.len());
        assert!(!snapshots.is_empty());
        // The release: the connection closes and nothing more is written.
        holder.dev.release_cooler().await.expect("released");
        assert_eq!(s.log.open(), 0, "the cooler's node is closed");
        let state =
            State::from_dict(&reader.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(state.cooler_state, Some(CoolerState::Released));
        assert_eq!(state.cooler_connected, Some(false));
        let written = s.messages();
        sleep(QUIET).await;
        assert_eq!(s.messages(), written, "nothing written while released");
        // The holder's connection closes: the next report is 03 08, then the real readings,
        // from the tick the engine took the override's end, at most a tick later.
        let connections = s.log.connections().len();
        drop(holder);
        let mut back = false;
        for _ in 0..30 {
            sleep(Duration::from_millis(100)).await;
            if s.log.connections().len() > connections
                && s.log.connections().last().is_some_and(|c| c.len() >= 5)
            {
                back = true;
                break;
            }
        }
        assert!(back, "the service takes the cooler back");
        let last = s.log.connections().pop().expect("a connection");
        assert_eq!(last[0], Message::Startup, "03 08 first");
        assert!(
            matches!(last[1], Message::Frame(_)),
            "then that tick's frame"
        );
        assert_eq!(
            last.last(),
            Some(&Message::Frame(home(readings(value(51))))),
            "the override ended with the session"
        );
        assert!(
            s.journal
                .lines()
                .iter()
                .any(|(_, l)| l.contains("taken back"))
        );
    });
}

#[test]
fn the_journal_says_the_readings_the_first_frame_is_drawn_from() {
    let s = Service::start("first-readings", vec![Presence::Present], false);
    let lines = s.journal.lines();
    let line = lines
        .iter()
        .map(|(_, l)| l.as_str())
        .find(|l| l.starts_with(FIRST_READINGS))
        .expect("a first readings line");
    let first = parse_first_readings(line).expect("the line reads back");
    let want = readings(value(51));
    assert!(
        first
            .iter()
            .all(|(metric, reading)| reading == want.get(*metric)),
        "{line}"
    );
}

#[test]
fn cooler_state_follows_the_cooler_once_per_change() {
    let mut script = vec![Presence::Absent; 20];
    script.extend(vec![
        Presence::NoAccess(PathBuf::from("/dev/hidraw-fake")),
        Presence::NoAccess(PathBuf::from("/dev/hidraw-fake")),
        Presence::FailsAfter(4),
        Presence::Present,
    ]);
    let s = Service::start("cooler-state", script, false);
    zbus::block_on(async {
        let watcher = Client::on(&s.bus).await;
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let first =
            State::from_dict(&watcher.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(first.cooler_state, Some(CoolerState::NotFound));
        let mut seen = Vec::new();
        while seen.last() != Some(&CoolerState::Connected) || seen.len() < 4 {
            let Some(Some(signal)) = within(Duration::from_secs(5), next(&mut changes)).await
            else {
                break;
            };
            let changes = signal.args().expect("its changes").changes;
            if let Some(state) = State::from_dict(&changes).expect("a state").cooler_state {
                seen.push(state);
            }
        }
        assert_eq!(
            seen,
            [
                CoolerState::NoAccess,
                CoolerState::Connected,
                CoolerState::WriteFailed,
                CoolerState::Connected
            ]
        );
    });
    let lines: Vec<String> = s
        .journal
        .lines()
        .into_iter()
        .map(|(_, line)| line)
        .filter(|line| line.contains("cooler"))
        .collect();
    assert_eq!(
        lines,
        [
            "no cooler is attached",
            // The fake's error has no errno.
            "the cooler at /dev/hidraw-fake can't be opened",
            "the cooler is connected",
            "a write to the cooler failed",
            "the cooler is connected",
        ]
    );
}

/// The test-only switch (Phase 9 plan, Task 3): every mode drawn, the Music modes included.
fn every_mode(_: Mode) -> bool {
    true
}

/// The status of [`status`], with the modes the test-only switch draws available.
fn music_status() -> Status {
    let snapshot = readings(value(51));
    Status {
        availability: availability_of(&snapshot, true, every_mode),
        ..status(snapshot)
    }
}

fn music(mode: u16) -> Settings {
    Settings {
        mode,
        ..Settings::default()
    }
}

/// `AttachAudio()`'s write end as a file.
fn writer(fd: zbus::wire::OwnedFd) -> fs::File {
    fs::File::from(std::os::fd::OwnedFd::from(fd))
}

/// Stereo frames whose mono samples are `samples`, both channels alike.
fn frames(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|s| [s.to_ne_bytes(), s.to_ne_bytes()].concat())
        .collect()
}

/// Whether a write to the pipe now finds its read end closed.
fn closed(pipe: &mut fs::File) -> bool {
    matches!(pipe.write_all(&frames(&[0.0])), Err(e) if e.kind() == ErrorKind::BrokenPipe)
}

/// Waits until `check` holds, at most [`WAIT`]; whether it did.
async fn soon(mut check: impl FnMut() -> bool) -> bool {
    for _ in 0..(WAIT.as_millis() / 20) {
        if check() {
            return true;
        }
        sleep(Duration::from_millis(20)).await;
    }
    check()
}

fn state_of(changes: &HashMap<String, OwnedVariant>) -> State {
    State::from_dict(changes).expect("a state")
}

// docs/spec/dbus.md, AttachAudio (Phase 9 plan, Task 3): allowed in a Music mode, a pipe whose
// samples reach the engine's window; the attach is logged by the caller's user and PID, never with
// any sound; AudioConnected tells the watchers.
#[test]
fn an_allowed_attach_gets_a_pipe_whose_samples_reach_the_window() {
    let h = Harness::drawing("attach", music(13), music_status(), false, every_mode);
    zbus::block_on(async {
        let (helper, watcher) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let before = state_of(&watcher.main.get_state().await.expect("the state"));
        assert_eq!(before.audio_wanted, Some(true));
        assert_eq!(before.audio_connected, Some(false));
        let mut pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        assert_eq!(h.polkit.asked(&helper.name()), [Action::FeedAudio]);
        let samples = [0.5_f32, -0.25, 0.125];
        pipe.write_all(&frames(&samples)).expect("written");
        assert!(
            soon(|| h.audio.window()[WINDOW - 3..] == samples).await,
            "the samples reach the window"
        );
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("StateChanged");
        let changed = signal.args().expect("its changes").changes;
        assert_eq!(keys(&changed), ["AudioConnected"]);
        assert_eq!(state_of(&changed).audio_connected, Some(true));
        assert!(h.journal_has(&format!(
            "sound attached from {} (user {USER}, PID {PID})",
            helper.name()
        )));
    });
}

#[test]
fn a_refused_or_unwanted_attach_gets_no_pipe() {
    let h = Harness::drawing(
        "attach-refused",
        music(13),
        music_status(),
        false,
        every_mode,
    );
    zbus::block_on(async {
        let helper = h.client().await;
        h.polkit.refuse(&helper.name());
        assert!(matches!(
            helper.main.attach_audio().await,
            Err(Error::NotAuthorized(_))
        ));
        assert_eq!(h.audio.holder(), None);
    });
    // Outside the Music modes it's refused before polkit is asked (owner, 2026-10-26).
    let h = Harness::start(
        "attach-unwanted",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let helper = h.client().await;
        let state = state_of(&helper.main.get_state().await.expect("the state"));
        assert_eq!(state.audio_wanted, Some(false));
        assert!(matches!(
            helper.main.attach_audio().await,
            Err(Error::AudioNotWanted(_))
        ));
        assert_eq!(h.polkit.asked(&helper.name()), [Action::Use]);
        assert_eq!(h.audio.holder(), None);
        assert!(h.journal_has("AttachAudio from"));
    });
}

// The check every second (Phase 9 plan, Task 3): a caller that no longer passes polkit's
// feed-audio has its pipe closed, which its writer sees as EPIPE, and the journal says why.
#[test]
fn an_attached_caller_that_stops_passing_polkit_loses_its_pipe() {
    let h = Harness::drawing(
        "attach-recheck",
        music(14),
        music_status(),
        false,
        every_mode,
    );
    zbus::block_on(async {
        let helper = h.client().await;
        let mut pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        sleep(Duration::from_millis(1200)).await;
        assert!(!closed(&mut pipe), "still allowed after a check");
        h.polkit.refuse(&helper.name());
        assert!(soon(|| closed(&mut pipe)).await, "closed within a second");
        assert_eq!(h.audio.holder(), None);
        assert!(
            h.journal_has("polkit no longer allows io.github.dtmarangoni.OpenDeepCool1.feed-audio")
        );
    });
}

// The latest authorized attach wins; a caller that leaves the bus loses its pipe.
#[test]
fn a_newer_attach_replaces_the_older_and_leaving_the_bus_closes_the_pipe() {
    let h = Harness::drawing(
        "attach-replace",
        music(23),
        music_status(),
        false,
        every_mode,
    );
    zbus::block_on(async {
        let (first, second) = (h.client().await, h.client().await);
        let mut older = writer(first.main.attach_audio().await.expect("a pipe"));
        let mut newer = writer(second.main.attach_audio().await.expect("a pipe"));
        assert!(soon(|| closed(&mut older)).await, "the older pipe closed");
        assert!(!closed(&mut newer));
        assert_eq!(h.audio.holder().map(|(_, n)| n), Some(second.name()));
        assert!(h.journal_has(&format!("in place of {}'s", first.name())));
        let name = second.name();
        drop(second);
        assert!(soon(|| closed(&mut newer)).await, "closed when it left");
        assert_eq!(h.audio.holder(), None);
        assert!(h.journal_has(&format!("the sound from {name} is closed: it left the bus")));
    });
}

// Sound only in the Music modes (owner, 2026-10-26): leaving them closes the pipe at once and turns
// AudioWanted off; coming back turns it on, and the watchers hear both.
#[test]
fn leaving_music_closes_the_pipe_and_audio_wanted_follows_the_mode() {
    let h = Harness::drawing("attach-leave", music(13), music_status(), false, every_mode);
    zbus::block_on(async {
        let (helper, watcher, changer) = (h.client().await, h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let mut pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        let connected = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal");
        assert_eq!(
            keys(&connected.args().expect("changes").changes),
            ["AudioConnected"]
        );
        changer.main.set_mode(1).await.expect("mode 1");
        assert_eq!(h.audio.holder(), None, "closed by the time SetMode returns");
        assert!(soon(|| closed(&mut pipe)).await);
        let left = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal");
        let left = left.args().expect("changes").changes;
        assert_eq!(keys(&left), ["AudioConnected", "AudioWanted", "Mode"]);
        assert_eq!(state_of(&left).audio_wanted, Some(false));
        assert!(h.journal_has("the display left the Music modes"));
        assert!(matches!(
            helper.main.attach_audio().await,
            Err(Error::AudioNotWanted(_))
        ));
        changer.main.set_mode(16).await.expect("mode 16");
        let back = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal");
        let back = back.args().expect("changes").changes;
        assert_eq!(keys(&back), ["AudioWanted", "Mode"]);
        assert_eq!(state_of(&back).audio_wanted, Some(true));
    });
}

// docs/spec/dbus.md, the Music setters: each validates its stop, stores it, hands it to the engine
// and tells the watchers; what's already set changes nothing.
#[test]
fn each_music_setter_validates_stores_and_tells_the_watchers() {
    let h = Harness::start(
        "music-settings",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (changer, watcher) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let state = state_of(&watcher.main.get_state().await.expect("the state"));
        assert_eq!(
            (
                state.music_peaks,
                state.music_bar_fall,
                state.music_peak_fall
            ),
            (Some(true), Some(FallStop::Moderate), Some(FallStop::Slow)),
            "Winamp's defaults"
        );
        changer.main.set_music_peaks(false).await.expect("stored");
        changer
            .main
            .set_music_bar_fall("fast")
            .await
            .expect("stored");
        changer
            .main
            .set_music_peak_fall("slower")
            .await
            .expect("stored");
        let saved = h.saved_settings();
        assert_eq!(
            (
                saved.music_peaks,
                saved.music_bar_fall,
                saved.music_peak_fall
            ),
            (false, Fall::Fast, Fall::Slower)
        );
        let applied = h
            .apply
            .take()
            .and_then(|a| a.settings)
            .expect("handed to the engine");
        assert_eq!(applied, saved, "the newest settings");
        let mut seen = State::default();
        while seen.music_peak_fall.is_none() {
            let signal = within(WAIT, next(&mut changes))
                .await
                .flatten()
                .expect("a signal");
            seen.merge(state_of(&signal.args().expect("changes").changes));
        }
        assert_eq!(
            (seen.music_peaks, seen.music_bar_fall, seen.music_peak_fall),
            (Some(false), Some(FallStop::Fast), Some(FallStop::Slower))
        );
        assert_eq!(seen.mode, None, "no other key");
        let before = h.saved();
        for bad in ["fastest", "Moderate", "", &"x".repeat(65)] {
            assert!(
                matches!(
                    changer.main.set_music_bar_fall(bad).await,
                    Err(Error::InvalidArgument(_))
                ),
                "{bad:?}"
            );
            assert!(matches!(
                changer.main.set_music_peak_fall(bad).await,
                Err(Error::InvalidArgument(_))
            ));
        }
        changer
            .main
            .set_music_peaks(false)
            .await
            .expect("already set");
        changer
            .main
            .set_music_bar_fall("fast")
            .await
            .expect("already set");
        h.nothing_stored_or_applied(&before);
        h.polkit.refuse(&changer.name());
        assert!(matches!(
            changer.main.set_music_peaks(true).await,
            Err(Error::NotAuthorized(_))
        ));
        h.nothing_stored_or_applied(&before);
    });
}

// docs/spec/dbus.md, OverrideAudio: the test session's sound replaces the helper's until
// ClearOverride or the session's end.
#[test]
fn the_sound_override_needs_the_session_and_ends_with_it() {
    let h = Harness::drawing(
        "override-audio",
        music(13),
        music_status(),
        true,
        every_mode,
    );
    zbus::block_on(async {
        let (holder, other) = (h.client().await, h.client().await);
        let (read_end, write_end) =
            rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).expect("a pipe");
        let fd = zbus::wire::Fd::from(&read_end);
        assert!(
            matches!(
                holder.dev.override_audio(fd).await,
                Err(Error::NotAuthorized(_))
            ),
            "BeginTest() first"
        );
        holder.dev.begin_test().await.expect("the session");
        assert!(matches!(
            other
                .dev
                .override_audio(zbus::wire::Fd::from(&read_end))
                .await,
            Err(Error::Busy(_))
        ));
        holder
            .dev
            .override_audio(zbus::wire::Fd::from(&read_end))
            .await
            .expect("overridden");
        drop(read_end);
        assert!(h.audio.overridden());
        let mut test = fs::File::from(write_end);
        test.write_all(&frames(&[0.375; 4])).expect("written");
        assert!(soon(|| h.audio.window()[WINDOW - 1] == 0.375).await);
        // The engine has taken every override, as the D-Bus side alone has none.
        h.status.write().expect("the status").adopted = Some((u64::MAX, 0));
        holder.dev.clear_override().await.expect("cleared");
        assert!(!h.audio.overridden());
        assert!(soon(|| closed(&mut test)).await, "its pipe closed");
        let (read_end, _write_end) =
            rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).expect("a pipe");
        holder
            .dev
            .override_audio(zbus::wire::Fd::from(&read_end))
            .await
            .expect("overridden");
        holder.dev.end_test().await.expect("ended");
        assert!(!h.audio.overridden(), "ended with the session");
    });
}

/// Column 7 in mode 13's spectrum area, rows 3–14: its bar's height from the floor, and whether a
/// dot is lit above it.
fn column_7(frame: &Frame) -> (usize, bool) {
    let lit: Vec<usize> = (1..=12).filter(|&h| frame.get(15 - h, 7)).collect();
    let bar = lit
        .iter()
        .enumerate()
        .take_while(|&(i, &h)| h == i + 1)
        .count();
    (bar, lit.len() > bar)
}

/// The frames the fake cooler got on its last connection.
fn frames_written(s: &Service) -> Vec<Frame> {
    s.log
        .connections()
        .pop()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| match m {
            Message::Frame(frame) => Some(frame),
            Message::Startup => None,
        })
        .collect()
}

// The whole service (Phase 9 plan, Task 3): a tone through the pipe draws its bar; when the pipe
// closes (here its writer ends; polkit's check closes it the same way, above), the spectrum hears
// silence and the bar falls to the floor at its bar fall rate: 2 rows a tick at moderate, H 12
// (music.md point 5), a peak dot above it.
#[test]
fn a_tone_through_the_pipe_draws_its_bar_and_a_closed_pipe_lets_it_fall() {
    let s = Service::drawing(
        "music-pipe",
        vec![Presence::Present],
        false,
        &music(13),
        every_mode,
    );
    zbus::block_on(async {
        let helper = Client::on(&s.bus).await;
        let mut pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        // Column 7's centre at −20 dBFS, written as it plays, 20 ms at a time.
        let tone: Vec<f32> = (0..960 * 40)
            .map(|n| {
                (0.1 * (2.0 * std::f64::consts::PI * 645.891_059 * n as f64 / 48_000.0).sin())
                    as f32
            })
            .collect();
        let player = std::thread::spawn(move || {
            for chunk in tone.chunks(960) {
                if pipe.write_all(&frames(chunk)).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            drop(pipe);
        });
        assert!(
            soon(|| frames_written(&s).iter().any(|f| column_7(f).0 >= 8)).await,
            "the tone's bar rises"
        );
        player.join().expect("played");
        // Its writer closed: silence. The bar falls from where it was to the floor, 2 rows a tick;
        // the peak dot, at its slow default, takes about 1.4 s to come down from there.
        sleep(Duration::from_millis(2500)).await;
        let shown: Vec<(usize, bool)> = frames_written(&s).iter().map(column_7).collect();
        let top = shown
            .iter()
            .rposition(|&(bar, _)| bar >= 8)
            .expect("the bar was up");
        let bars: Vec<usize> = shown[top..].iter().map(|&(bar, _)| bar).collect();
        // A dot right above the bar looks like a bar a row taller: once the bar is down, at most
        // the floor and a dot touching it.
        let falling = bars.iter().take_while(|&&bar| bar > 2).count();
        assert!(
            bars[..falling].windows(2).all(|w| w[0] - w[1] == 2),
            "falling at the bar fall rate, 2 rows a tick: {bars:?}"
        );
        assert!(bars[falling..].iter().all(|&bar| bar <= 2), "{bars:?}");
        assert_eq!(
            shown.last(),
            Some(&(1, false)),
            "down to the floor, the dot too"
        );
        assert!(
            s.journal
                .lines()
                .iter()
                .any(|(_, l)| l.contains("stopped, its writer closed it"))
        );
    });
}

/// A frame with one pixel lit, a different one for each `n`.
fn lit(n: usize) -> Frame {
    let mut frame = Frame::blank();
    frame.set(1 + (n / 14) % 14, 1 + n % 14, true);
    frame
}

/// The next `Frame` a stream brings within `limit`, as a frame.
async fn next_frame<S>(stream: &mut S, limit: Duration) -> Option<Frame>
where
    S: Stream<Item = odc_ipc::Frame> + Unpin,
{
    let signal = within(limit, next(stream)).await.flatten()?;
    let pixels = signal.args().expect("its pixels").pixels;
    assert_eq!(pixels.len(), 196, "196 bytes");
    Some(frame_from_pixels(&pixels).expect("each byte 0 or 1"))
}

// Phase 10 plan, Task 2: `Frame` goes to each frame watcher alone, once per change; a newcomer
// gets the next frame drawn; a connection with a match rule but no watch gets nothing; a second
// WatchFrames() changes nothing; one leaving keeps the signal for the other.
#[test]
fn frames_go_to_each_frame_watcher_alone_once_per_change() {
    let h = Harness::start(
        "frames",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (watcher, bystander) = (h.client().await, h.client().await);
        let mut seen = watcher.main.receive_frame().await.expect("a stream");
        let mut unseen = bystander.main.receive_frame().await.expect("a stream");
        h.frame(lit(0));
        assert_eq!(
            next_frame(&mut seen, QUIET).await,
            None,
            "no watcher, no frame"
        );
        watcher.main.watch_frames().await.expect("watches");
        watcher
            .main
            .watch_frames()
            .await
            .expect("a second call changes nothing");
        h.frame(lit(1));
        assert_eq!(next_frame(&mut seen, WAIT).await, Some(lit(1)));
        h.frame(lit(1));
        assert_eq!(next_frame(&mut seen, QUIET).await, None, "unchanged");
        h.frame(lit(2));
        assert_eq!(next_frame(&mut seen, WAIT).await, Some(lit(2)));
        assert_eq!(
            next_frame(&mut unseen, QUIET).await,
            None,
            "a match rule without a watch"
        );
        // A newcomer gets the next frame drawn, unchanged or not; the first gets only changes.
        let newcomer = h.client().await;
        let mut fresh = newcomer.main.receive_frame().await.expect("a stream");
        newcomer.main.watch_frames().await.expect("watches");
        h.frame(lit(2));
        assert_eq!(next_frame(&mut fresh, WAIT).await, Some(lit(2)));
        assert_eq!(next_frame(&mut seen, QUIET).await, None);
        drop(seen);
        drop(watcher);
        h.frame(lit(3));
        assert_eq!(next_frame(&mut fresh, WAIT).await, Some(lit(3)), "kept");
        newcomer.main.unwatch_frames().await.expect("stops");
        h.frame(lit(4));
        assert_eq!(
            next_frame(&mut fresh, QUIET).await,
            None,
            "after UnwatchFrames"
        );
    });
}

// Only the active session's callers watch frames: a refused caller can't start, and a watcher
// the check stops allowing gets none until it passes again, then the next frame.
#[test]
fn only_authorized_callers_get_frames_and_a_refused_watcher_waits_for_its_check() {
    let h = Harness::start(
        "frames-polkit",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (refused, watcher) = (h.client().await, h.client().await);
        h.polkit.refuse(&refused.name());
        assert!(matches!(
            refused.main.watch_frames().await,
            Err(Error::NotAuthorized(_))
        ));
        let mut frames = watcher.main.receive_frame().await.expect("a stream");
        watcher.main.watch_frames().await.expect("watches");
        h.frame(lit(5));
        assert_eq!(next_frame(&mut frames, WAIT).await, Some(lit(5)));
        // The check runs on the D-Bus side's one-second timer: its journal line says when.
        let h = &h;
        let until = |line: String| async move {
            for _ in 0..30 {
                if h.journal_has(&line) {
                    return;
                }
                sleep(Duration::from_millis(100)).await;
            }
            panic!("no journal line {line:?}");
        };
        h.polkit.refuse(&watcher.name());
        until(format!("watcher {} no longer passes", watcher.name())).await;
        h.frame(lit(6));
        assert_eq!(next_frame(&mut frames, QUIET).await, None, "skipped");
        h.polkit.allow(&watcher.name());
        until(format!(
            "watcher {} passes polkit's check again",
            watcher.name()
        ))
        .await;
        h.frame(lit(6));
        assert_eq!(next_frame(&mut frames, WAIT).await, Some(lit(6)), "back");
    });
}

// FanInputs, whose speeds change every second, reaches only callers that watch frames too;
// GetState always has the speeds.
#[test]
fn fan_speeds_reach_only_frame_watchers_and_follow_the_readings() {
    let h = Harness::start(
        "fan-speeds",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (app, other) = (h.client().await, h.client().await);
        app.main.watch().await.expect("watches");
        app.main.watch_frames().await.expect("watches frames");
        other.main.watch().await.expect("watches");
        let mut app_changes = app.main.receive_state_changed().await.expect("a stream");
        let mut other_changes = other.main.receive_state_changed().await.expect("a stream");
        h.status.write().expect("the status").fan_rpms = vec![value(1234), value(12_345)];
        let signal = within(WAIT, next(&mut app_changes))
            .await
            .flatten()
            .expect("the speeds");
        let changes =
            State::from_dict(&signal.args().expect("its changes").changes).expect("a state");
        let speeds: Vec<FanSpeed> = changes
            .fan_inputs
            .expect("FanInputs")
            .iter()
            .map(|i| i.rpm)
            .collect();
        assert_eq!(speeds, [FanSpeed::Rpm(1234), FanSpeed::Rpm(9999)], "capped");
        assert!(
            within(QUIET, next(&mut other_changes)).await.is_none(),
            "not to a plain watcher"
        );
        let state =
            State::from_dict(&other.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(
            state.fan_inputs.expect("FanInputs")[0].rpm,
            FanSpeed::Rpm(1234)
        );
    });
}

// The GPU in use follows the choice, Automatic reading the first GPU; each GPU has its IDs and
// rule flag; the present flags follow a chosen input vanishing.
#[test]
fn the_state_names_the_gpu_in_use_its_ids_and_whether_the_inputs_are_there() {
    let h = Harness::start(
        "gpu-in-use",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let state = || async {
            State::from_dict(&caller.main.get_state().await.expect("the state")).expect("a state")
        };
        let now = state().await;
        assert_eq!(now.chosen_gpu.as_deref(), Some(""));
        assert_eq!(now.gpu_in_use.as_deref(), Some("0000:0a:00.0"), "Automatic");
        let gpus = now.gpus.expect("Gpus");
        assert_eq!((gpus[0].vendor, gpus[0].device), (0x0abc, 0x0def));
        assert_eq!((gpus[1].vendor, gpus[1].device), (0, 0), "none in sysfs");
        assert!(
            gpus.iter().all(|g| g.has_rule),
            "amdgpu and i915 have rules"
        );
        assert_eq!(now.cooler_fan_input_present, Some(true));
        assert_eq!(now.pump_input_present, Some(true), "none chosen");
        caller
            .main
            .choose_gpu("0000:00:02.0")
            .await
            .expect("chosen");
        assert_eq!(state().await.gpu_in_use.as_deref(), Some("0000:00:02.0"));
        let gone = Snapshot::new(
            |metric| value(11 + 10 * metric as i64),
            Fan::Chosen(Reading::Unavailable(Unavailable::NoDevice)),
            Fan::NotChosen,
        );
        h.status.write().expect("the status").snapshot = Some(gone);
        assert_eq!(state().await.cooler_fan_input_present, Some(false));
    });
}

// Phase 10 plan, Task 2, through the whole service: a value of 100 or more scrolls, so the frame
// changes every tick: about ten `Frame` signals a second, each a frame of its cycle, none twice
// in a row. They come with no cooler at all, so the preview shows what the cooler would (D20).
#[test]
fn a_scrolling_value_gives_ten_frames_a_second_even_with_no_cooler() {
    let s = Service::start("frames-scroll", vec![Presence::Absent], true);
    let reading = |m: Metric| {
        if m == Metric::CpuUsage {
            100
        } else {
            13 + 7 * m as i64
        }
    };
    zbus::block_on(async {
        let client = Client::on(&s.bus).await;
        client.dev.begin_test().await.expect("the session");
        let values: HashMap<String, (i64, u64)> = Metric::ALL
            .into_iter()
            .map(|m| (m.spec_name().to_owned(), (reading(m), 1)))
            .collect();
        client
            .dev
            .override_readings(values, HashMap::new())
            .await
            .expect("the override");
        let mut frames = client.main.receive_frame().await.expect("a stream");
        client.main.watch_frames().await.expect("watches");
        let start = std::time::Instant::now();
        let mut got = Vec::new();
        while start.elapsed() < Duration::from_secs(2) {
            if let Some(frame) = next_frame(&mut frames, Duration::from_millis(300)).await {
                got.push(frame);
            }
        }
        assert!(
            (17..=23).contains(&got.len()),
            "{} frames in 2 s",
            got.len()
        );
        let snapshot = Snapshot::new(|m| value(reading(m)), Fan::NotChosen, Fan::NotChosen);
        let mut engine = Engine::new(Settings::default(), snapshot);
        let cycle: Vec<Frame> = (0..60)
            .filter_map(|_| engine.tick(None, None, false).frame)
            .collect();
        assert!(got.iter().all(|f| cycle.contains(f)), "frames of the cycle");
        assert!(got.windows(2).all(|w| w[0] != w[1]), "one per change");
    });
}
