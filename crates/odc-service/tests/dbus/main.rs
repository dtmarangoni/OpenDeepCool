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
    FallStop, INTERFACE, OBJECT_PATH, OpenDeepCoolProxy, STATE_KEYS, State, USE_ACTION,
    frame_from_pixels,
};

use odc_protocol::{Frame, Message, decode};

use odc_render::{Mode, WINDOW};

use odc_sensors::{Binding, Exact, Gpu, GpuKind, Metric, PciAddr, Reading, Snapshot, Unavailable};

use odc_service::audio::{AudioSource, PipeAudio};

use odc_service::dbus::{Bus, Checks, Parts, Side};

use odc_service::engine::{Apply, Choices, Engine, availability};

use odc_service::log::{Journal, Recorded};

use odc_service::polkit::{
    Action, Answer, Authorizer, BusCredentials, Credentials, Identity, PolkitAuthorizer,
};

use odc_service::pulse::Pulse;

use odc_service::runtime::{
    DeviceControl, Durations, LevelTap, SensorLoop, SensorRequest, Sensors, Slot,
};

use odc_service::service::{Config, Running};

use odc_service::settings::{
    Fall, Settings, SettingsStore, TemperatureUnit, WindowCorners, WindowOpacity,
};

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
    Snapshot::new(|metric| match metric {
        Metric::GpuUsage => gpu_usage.clone(),
        other => value(11 + 10 * other as i64),
    })
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
    /// The levels the engine would analyse in a Music mode, which the test posts itself.
    levels: LevelTap,
    /// The pulse the engine sends after a tick whose status changed.
    pulse: Arc<Pulse>,
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
        let levels = LevelTap::default();
        let pulse = Arc::new(Pulse::default());
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
            levels: levels.clone(),
            pulse: Arc::clone(&pulse),
            journal: Arc::clone(&journal) as Arc<dyn Journal>,
            dev: dev.then(|| (GROUP, recording.handle(), Arc::clone(&durations))),
            audio: Arc::clone(&audio),
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
            levels,
            pulse,
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

    /// Hands the D-Bus side the levels of a tick, as the engine does in a Music mode.
    fn levels(&self, levels: [f64; 14]) {
        self.levels.post(levels);
    }

    /// The engine's status after a tick that drew other readings: the snapshot, and the pulse.
    fn read(&self, snapshot: Snapshot) {
        self.status.write().expect("the status").snapshot = Some(snapshot);
        self.pulse.send();
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
        Service::start_on(name, script, dev, &Settings::default())
    }

    /// [`Service::start`] on `saved`.
    fn start_on(name: &str, script: Vec<Presence>, dev: bool, saved: &Settings) -> Service {
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

/// The status of [`status`], with every mode available.
fn music_status() -> Status {
    let snapshot = readings(value(51));
    Status {
        availability: availability(&snapshot, true),
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

/// A snapshot whose metrics each have their own fraction that isn't whole (`base` thousandths
/// over 1000, rising with the metric), CPU Power having no value.
fn fractions(base: i64) -> Snapshot {
    Snapshot::new(|metric| match metric {
        Metric::CpuPower => Reading::Unavailable(Unavailable::NoDevice),
        other => {
            Reading::Value(Exact::new(base + 1000 * other as i64 + 7, 1000).expect("a denominator"))
        }
    })
}

/// What `Readings` says of [`fractions`]: every metric but CPU Power, by its spec name.
fn readings_of(base: i64) -> HashMap<String, (i64, u64)> {
    Metric::ALL
        .into_iter()
        .filter(|&metric| metric != Metric::CpuPower)
        .map(|metric| {
            (
                metric.spec_name().to_owned(),
                (base + 1000 * metric as i64 + 7, 1000),
            )
        })
        .collect()
}

/// The next `StateChanged` a stream brings within `limit`, as its keys and its state.
async fn next_changes<S>(stream: &mut S, limit: Duration) -> Option<(Vec<String>, State)>
where
    S: Stream<Item = odc_ipc::StateChanged> + Unpin,
{
    let signal = within(limit, next(stream)).await.flatten()?;
    let changes = signal.args().expect("its changes").changes;
    Some((keys(&changes), state_of(&changes)))
}

/// Levels that are all `n` hundredths, within 0 to 1 for `n` up to 100.
fn levels_at(n: f64) -> [f64; 14] {
    std::array::from_fn(|column| (n + column as f64) / 100.0)
}

/// The next `Levels` a stream brings within `limit`.
async fn next_levels<S>(stream: &mut S, limit: Duration) -> Option<Vec<f64>>
where
    S: Stream<Item = odc_ipc::Levels> + Unpin,
{
    let signal = within(limit, next(stream)).await.flatten()?;
    Some(signal.args().expect("its levels").levels)
}

mod audio;
mod dev_interface;
mod frames;
mod gpu_and_timing;
mod polkit;
mod service;
mod setters;
mod settings;
mod state;
mod watchers;
