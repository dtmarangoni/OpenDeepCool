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
    INTERFACE, OBJECT_PATH, OpenDeepCoolProxy, STATE_KEYS, State, USE_ACTION,
};
use odc_protocol::{Frame, Message, decode};
use odc_sensors::{
    Binding, Exact, Fan, FanInput, Gpu, GpuKind, Metric, PciAddr, Reading, Snapshot, Unavailable,
};
use odc_service::dbus::{Bus, Checks, Parts, Side, kind_of, reason_of};
use odc_service::engine::{Apply, Choices, Engine, availability};
use odc_service::log::{Journal, Recorded};
use odc_service::polkit::{
    Action, Answer, Authorizer, BusCredentials, Credentials, Identity, PolkitAuthorizer,
};
use odc_service::pulse::Pulse;
use odc_service::runtime::{DeviceControl, Durations, Sensors, Slot};
use odc_service::service::{Config, Running};
use odc_service::settings::{SavedInput, Settings, SettingsStore, TemperatureUnit};
use odc_service::status::{FIRST_READINGS, Status, parse_first_readings};
use odc_testkit::bus::{PrivateBus, sleep, within};
use odc_testkit::capture::read_recording;
use zbus::message::Header;
use zbus::wire::OwnedVariant;
use zbus::{Connection, Guid, connection};

/// The callers' made-up Unix user, and the service's made-up group.
const USER: u32 = 1000;
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
        },
        Gpu {
            pci: PciAddr::from_str("0000:00:02.0").expect("an address"),
            driver: "i915".to_owned(),
            kind: GpuKind::Integrated,
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
    journal: Arc<Recorded>,
    /// The threads' durations, which the test writes itself.
    durations: Arc<Durations>,
}

impl Harness {
    fn start(name: &str, settings: Settings, status: Status, dev: bool) -> Harness {
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
        let durations = Arc::new(Durations::default());
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
            pulse: Arc::new(Pulse::default()),
            journal: Arc::clone(&journal) as Arc<dyn Journal>,
            dev: dev.then(|| (GROUP, recording.handle(), Arc::clone(&durations))),
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
            journal,
            durations,
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

    fn saved_settings(&self) -> Settings {
        toml::from_str(&self.saved().1).expect("settings")
    }

    /// Nothing was stored since `before`, and nothing handed to the engine.
    fn nothing_stored_or_applied(&self, before: &(u64, String)) {
        assert_eq!(self.saved(), *before, "nothing stored");
        assert_eq!(self.apply.take(), None, "nothing applied");
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
        // Music comes in Phase 9.
        assert!(matches!(
            caller.main.set_mode(13).await,
            Err(Error::NotYetAvailable(_))
        ));
        for mode in [0, 24, 70_000, u32::MAX] {
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
            },
        );
        h.credentials.give(
            &unknown.name(),
            Identity {
                user: Some(USER),
                groups: None,
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
        assert_eq!(changes.len(), STATE_KEYS.len(), "every key in one signal");
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
        let dir = dir(name);
        let bus = PrivateBus::start(&dir.join("bus")).expect("a private bus");
        let state = dir.join("state");
        fs::create_dir_all(&state).expect("a state directory");
        let store = SettingsStore::new(&state);
        let (settings, _) = store.load().expect("the defaults");
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
