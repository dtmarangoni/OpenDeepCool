//! `opendeepcool-audio` against a fake display service and a fake logind on a private bus (Phase 9
//! plan, Task 4). This test program is also the fake `pw-record` (AGENTS.md rule 13): each case
//! links that name to it in the case's folder and gives the helper the link, and it plays the
//! recorder as the folder's `mode` file tells it, logging each start to `runs`. The fake service
//! reads the pipe with the service's own `PipeAudio`. Nothing here touches PipeWire, the system
//! bus or the real service.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::panic;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use odc_audio::{Config, Log, Priority, pw_record_args, run};
use odc_ipc::{BUS_NAME, Error, INTERFACE, OBJECT_PATH, State};
use odc_service::audio::{AudioSource, PipeAudio};
use odc_testkit::bus::{PrivateBus, sleep};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::wire::{OwnedFd, OwnedVariant, Variant};
use zbus::{Connection, connection};

type Test = (&'static str, fn());

const TESTS: [Test; 9] = [
    (
        "samples_reach_the_service_and_a_closed_pipe_is_attached_again_only_while_wanted",
        samples_reach_the_service_and_a_closed_pipe_is_attached_again_only_while_wanted,
    ),
    (
        "no_recorder_runs_until_sound_is_wanted_and_it_stops_within_a_second",
        no_recorder_runs_until_sound_is_wanted_and_it_stops_within_a_second,
    ),
    (
        "a_refused_attach_is_logged_once_and_tried_again_only_on_a_change_or_the_timer",
        a_refused_attach_is_logged_once_and_tried_again_only_on_a_change_or_the_timer,
    ),
    (
        "a_new_active_session_tries_a_refused_watch_at_once",
        a_new_active_session_tries_a_refused_watch_at_once,
    ),
    (
        "a_recorder_that_ends_after_a_while_is_started_again_at_once",
        a_recorder_that_ends_after_a_while_is_started_again_at_once,
    ),
    (
        "a_recorder_that_fails_at_once_is_started_again_only_after_the_timer",
        a_recorder_that_fails_at_once_is_started_again_only_after_the_timer,
    ),
    (
        "a_missing_pw_record_is_logged_once",
        a_missing_pw_record_is_logged_once,
    ),
    (
        "the_service_leaving_stops_the_recorder_and_its_return_is_watched_again",
        the_service_leaving_stops_the_recorder_and_its_return_is_watched_again,
    ),
    (
        "a_state_change_from_another_connection_is_ignored",
        a_state_change_from_another_connection_is_ignored,
    ),
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let me = PathBuf::from(&args[0]);
    if me.file_name().and_then(|n| n.to_str()) == Some("pw-record") {
        process::exit(fake_pw_record(&me, &args[1..]));
    }
    let filter = args.iter().skip(1).find(|arg| !arg.starts_with('-'));
    let mut failed = Vec::new();
    let mut passed = 0;
    for (name, test) in TESTS {
        if filter.is_some_and(|f| !name.contains(f.as_str())) {
            continue;
        }
        if panic::catch_unwind(test).is_ok() {
            passed += 1;
            println!("test {name} ... ok");
        } else {
            println!("test {name} ... FAILED");
            failed.push(name);
        }
    }
    let result = if failed.is_empty() { "ok" } else { "FAILED" };
    println!(
        "\ntest result: {result}. {passed} passed; {} failed",
        failed.len()
    );
    if !failed.is_empty() {
        process::exit(101);
    }
}

/// The fake `pw-record`: it logs its start, `pid <pid> <args>`, to `runs`, then writes stereo
/// frames of 0.25 to its standard output every 20 ms until a write fails, as `mode` says:
/// `write` for ever, `fail` not at all, or `end-after <ms>` that long, the first run only.
fn fake_pw_record(me: &Path, args: &[String]) -> i32 {
    let dir = me.parent().expect("the case's folder");
    let mut runs = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("runs"))
        .expect("the runs log");
    writeln!(runs, "pid {} {}", process::id(), args.join(" ")).expect("logged");
    drop(runs);
    let first = fs::read_to_string(dir.join("runs"))
        .map(|t| t.lines().count() == 1)
        .unwrap_or(true);
    let mode = fs::read_to_string(dir.join("mode")).unwrap_or_else(|_| "write".into());
    let end = match mode.trim().split_once(' ') {
        Some(("end-after", ms)) if first => {
            Some(Instant::now() + Duration::from_millis(ms.parse().unwrap_or(0)))
        }
        _ => None,
    };
    if mode.trim() == "fail" {
        return 1;
    }
    let chunk: Vec<u8> = (0..960)
        .flat_map(|_| [0.25f32.to_ne_bytes(), 0.25f32.to_ne_bytes()].concat())
        .collect();
    let mut out = std::io::stdout().lock();
    loop {
        if end.is_some_and(|end| Instant::now() >= end) {
            return 0;
        }
        if out.write_all(&chunk).and_then(|()| out.flush()).is_err() {
            return 0;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// A case's folder, with `pw-record` linked to this program.
fn case(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("odc-audio")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a case folder");
    symlink(
        std::env::current_exe().expect("this program"),
        dir.join("pw-record"),
    )
    .expect("the fake pw-record");
    dir
}

/// The fake recorder's starts so far: each one's PID and arguments.
fn runs(dir: &Path) -> Vec<(u32, String)> {
    fs::read_to_string(dir.join("runs"))
        .unwrap_or_default()
        .lines()
        .map(|line| {
            let rest = line.strip_prefix("pid ").expect("a run");
            let (pid, args) = rest.split_once(' ').expect("a PID and arguments");
            (pid.parse().expect("a PID"), args.to_owned())
        })
        .collect()
}

/// Whether process `pid` is gone, reaped by the helper.
fn gone(pid: u32) -> bool {
    !Path::new(&format!("/proc/{pid}")).exists()
}

/// Waits until `check` holds, at most `limit`; whether it did.
fn within(limit: Duration, mut check: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + limit;
    while Instant::now() < until {
        if check() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    check()
}

/// The helper's lines.
#[derive(Clone, Default)]
struct Recorded(Arc<Mutex<Vec<(Priority, String)>>>);

impl Log for Recorded {
    fn write(&self, priority: Priority, line: &str) {
        self.0
            .lock()
            .expect("the lines")
            .push((priority, line.to_owned()));
    }
}

impl Recorded {
    fn count(&self, part: &str) -> usize {
        self.0
            .lock()
            .expect("the lines")
            .iter()
            .filter(|(_, l)| l.contains(part))
            .count()
    }
}

/// What the fake service does, which the test sets.
#[derive(Default)]
struct Knobs {
    wanted: AtomicBool,
    refuse_watch: AtomicBool,
    refuse_attach: AtomicBool,
    watches: AtomicUsize,
    attaches: AtomicUsize,
    watchers: Mutex<Vec<String>>,
    audio: PipeAudio,
}

/// The fake display service's main interface: `Watch()`, `GetState()` and `AttachAudio()`, as
/// `docs/spec/dbus.md` says, with the sound read by the service's own `PipeAudio`.
struct FakeMain {
    knobs: Arc<Knobs>,
    conn: Arc<Mutex<Option<Connection>>>,
}

fn state(knobs: &Knobs) -> State {
    State {
        audio_wanted: Some(knobs.wanted.load(Ordering::SeqCst)),
        audio_connected: Some(knobs.audio.holder().is_some()),
        ..State::default()
    }
}

/// `StateChanged` with `changes` to every watcher.
async fn tell(conn: &Connection, knobs: &Knobs, changes: State) {
    let watchers = knobs.watchers.lock().expect("the watchers").clone();
    for watcher in watchers {
        let emitter = SignalEmitter::new(conn, OBJECT_PATH)
            .expect("an emitter")
            .set_destination(watcher.as_str().try_into().expect("a name"));
        let _ = emitter
            .emit(INTERFACE, "StateChanged", &(changes.to_dict(),))
            .await;
    }
}

#[zbus::interface(name = "io.github.dtmarangoni.OpenDeepCool1")]
impl FakeMain {
    async fn watch(&self, #[zbus(header)] header: Header<'_>) -> Result<(), Error> {
        self.knobs.watches.fetch_add(1, Ordering::SeqCst);
        if self.knobs.refuse_watch.load(Ordering::SeqCst) {
            return Err(Error::NotAuthorized("not the active session".into()));
        }
        let sender = header.sender().expect("a sender").to_string();
        self.knobs
            .watchers
            .lock()
            .expect("the watchers")
            .push(sender);
        Ok(())
    }

    async fn get_state(&self) -> HashMap<String, OwnedVariant> {
        state(&self.knobs).to_dict()
    }

    async fn attach_audio(&self, #[zbus(header)] header: Header<'_>) -> Result<OwnedFd, Error> {
        self.knobs.attaches.fetch_add(1, Ordering::SeqCst);
        if !self.knobs.wanted.load(Ordering::SeqCst) {
            return Err(Error::AudioNotWanted("mode 1 is shown".into()));
        }
        if self.knobs.refuse_attach.load(Ordering::SeqCst) {
            return Err(Error::NotAuthorized(
                "polkit doesn't allow feed-audio".into(),
            ));
        }
        let sender = header.sender().expect("a sender").to_string();
        let (read, write) = std::io::pipe().expect("a pipe");
        let (knobs, conn) = (Arc::clone(&self.knobs), Arc::clone(&self.conn));
        self.knobs
            .audio
            .attach(read.into(), &sender, move |_, _| {
                // The service tells its watchers the pipe closed.
                if let Some(conn) = conn.lock().expect("the connection").clone() {
                    zbus::block_on(tell(&conn, &knobs, connected(false)));
                }
            })
            .expect("attached");
        let conn = self.conn.lock().expect("the connection").clone();
        if let Some(conn) = conn {
            tell(&conn, &self.knobs, connected(true)).await;
        }
        Ok(OwnedFd::from(std::os::fd::OwnedFd::from(write)))
    }
}

fn connected(on: bool) -> State {
    State {
        audio_connected: Some(on),
        ..State::default()
    }
}

/// The fake service on the private bus, owning the service's name until dropped.
struct FakeService {
    knobs: Arc<Knobs>,
    conn: Connection,
    slot: Arc<Mutex<Option<Connection>>>,
}

impl Drop for FakeService {
    /// Leaves the bus as a service that ends does: its interface holds the connection, so the name
    /// is released and the interface removed by hand, and its pipe closes.
    fn drop(&mut self) {
        self.slot.lock().expect("the connection").take();
        self.knobs.audio.detach();
        zbus::block_on(async {
            let _ = self
                .conn
                .object_server()
                .remove::<FakeMain, _>(OBJECT_PATH)
                .await;
            let _ = self.conn.release_name(BUS_NAME).await;
        });
    }
}

impl FakeService {
    fn start(bus: &PrivateBus, wanted: bool) -> FakeService {
        let knobs = Arc::new(Knobs::default());
        knobs.wanted.store(wanted, Ordering::SeqCst);
        let slot = Arc::new(Mutex::new(None));
        let conn = zbus::block_on(async {
            let conn = connection::Builder::address(bus.address())
                .serve_at(
                    OBJECT_PATH,
                    FakeMain {
                        knobs: Arc::clone(&knobs),
                        conn: Arc::clone(&slot),
                    },
                )
                .build()
                .await
                .expect("a connection");
            conn.request_name(BUS_NAME).await.expect("the name");
            conn
        });
        *slot.lock().expect("the connection") = Some(conn.clone());
        FakeService { knobs, conn, slot }
    }

    /// The display enters or leaves the Music modes: leaving closes the pipe at once.
    fn set_wanted(&self, wanted: bool) {
        self.knobs.wanted.store(wanted, Ordering::SeqCst);
        let mut changes = State {
            audio_wanted: Some(wanted),
            ..State::default()
        };
        if !wanted && self.knobs.audio.detach().is_some() {
            changes.audio_connected = Some(false);
        }
        zbus::block_on(tell(&self.conn, &self.knobs, changes));
    }

    /// The service closes the pipe, as when polkit stops allowing it.
    fn close_pipe(&self) {
        if self.knobs.audio.detach().is_some() {
            zbus::block_on(tell(&self.conn, &self.knobs, connected(false)));
        }
    }

    fn attaches(&self) -> usize {
        self.knobs.attaches.load(Ordering::SeqCst)
    }

    fn hears(&self, sample: f32) -> bool {
        self.knobs.audio.window().last() == Some(&sample)
    }
}

/// The fake logind, owning its name, which announces a new active session on seat0.
struct FakeLogind {
    conn: Connection,
}

impl FakeLogind {
    fn start(bus: &PrivateBus) -> FakeLogind {
        let conn = zbus::block_on(async {
            let conn = connection::Builder::address(bus.address())
                .build()
                .await
                .expect("a connection");
            conn.request_name("org.freedesktop.login1")
                .await
                .expect("the name");
            conn
        });
        FakeLogind { conn }
    }

    fn new_active_session(&self) {
        let changed: HashMap<&str, Variant<'_>> = HashMap::from([(
            "ActiveSession",
            Variant::from((
                "2",
                zbus::wire::ObjectPath::try_from("/org/freedesktop/login1/session/_32")
                    .expect("a path"),
            )),
        )]);
        zbus::block_on(self.conn.emit_signal(
            None::<()>,
            "/org/freedesktop/login1/seat/seat0",
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &("org.freedesktop.login1.Seat", changed, Vec::<String>::new()),
        ))
        .expect("sent");
    }
}

/// The helper, on its own connection and thread, until dropped.
struct Helper {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    log: Recorded,
}

impl Helper {
    fn start(bus: &PrivateBus, config: Config) -> Helper {
        let stop = Arc::new(AtomicBool::new(false));
        let log = Recorded::default();
        let address = bus.address().to_owned();
        let (until, lines) = (Arc::clone(&stop), log.clone());
        let thread = thread::spawn(move || {
            zbus::block_on(async {
                let conn = connection::Builder::address(address.as_str())
                    .build()
                    .await
                    .expect("a connection");
                let stopped = async {
                    while !until.load(Ordering::SeqCst) {
                        sleep(Duration::from_millis(20)).await;
                    }
                };
                run(&conn, &config, &lines, stopped).await.expect("ran");
            });
        });
        Helper {
            stop,
            thread: Some(thread),
            log,
        }
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A config with the fake recorder in `dir` and the fallback timer's waits.
fn config(dir: &Path, first: Duration, longest: Duration) -> Config {
    Config {
        program: dir.join("pw-record"),
        target: None,
        first_wait: first,
        longest_wait: longest,
    }
}

fn bus(dir: &Path) -> PrivateBus {
    PrivateBus::start(&dir.join("bus")).expect("a private bus")
}

const SECOND: Duration = Duration::from_secs(1);

// Phase 9 plan, Task 4: the fake recorder's samples reach the fake service through the pipe; when
// the service closes the pipe the recorder ends, and the helper attaches again only while
// AudioWanted is still true.
fn samples_reach_the_service_and_a_closed_pipe_is_attached_again_only_while_wanted() {
    let dir = case("samples");
    let bus = bus(&dir);
    let service = FakeService::start(&bus, true);
    let helper = Helper::start(&bus, config(&dir, Duration::from_millis(300), 3 * SECOND));
    assert!(
        within(3 * SECOND, || service.hears(0.25)),
        "the samples arrive"
    );
    let first = runs(&dir);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].1, pw_record_args(None).join(" "), "the arguments");
    assert_eq!(helper.log.count("capturing the default output"), 1);
    // The service closes the pipe while it still wants sound: a new pipe, a new recorder.
    thread::sleep(Duration::from_millis(500));
    service.close_pipe();
    assert!(
        within(2 * SECOND, || runs(&dir).len() == 2),
        "attached again"
    );
    assert!(
        within(SECOND, || gone(first[0].0)),
        "the first recorder ended"
    );
    assert!(within(2 * SECOND, || service.hears(0.25)));
    // The display leaves Music: the recorder stops, and nothing attaches again.
    let second = runs(&dir)[1].0;
    service.set_wanted(false);
    assert!(within(SECOND, || gone(second)), "stopped within a second");
    thread::sleep(Duration::from_millis(600));
    assert_eq!(runs(&dir).len(), 2);
    assert_eq!(helper.log.count("the display shows no Music mode"), 1);
}

fn no_recorder_runs_until_sound_is_wanted_and_it_stops_within_a_second() {
    let dir = case("wanted");
    let bus = bus(&dir);
    let service = FakeService::start(&bus, false);
    let _helper = Helper::start(&bus, config(&dir, Duration::from_millis(300), 3 * SECOND));
    thread::sleep(Duration::from_millis(500));
    assert!(runs(&dir).is_empty(), "no recorder outside Music");
    assert_eq!(service.attaches(), 0, "no attach either");
    service.set_wanted(true);
    assert!(
        within(SECOND, || runs(&dir).len() == 1),
        "started on the change"
    );
    let pid = runs(&dir)[0].0;
    service.set_wanted(false);
    assert!(within(SECOND, || gone(pid)), "stopped within a second");
}

// A minute of refusals writes one line, not 30: the helper tries again only on a watched change or
// the fallback timer, whose wait doubles (here 100 ms up to 400 ms, for 2 s up to 60 s).
fn a_refused_attach_is_logged_once_and_tried_again_only_on_a_change_or_the_timer() {
    let dir = case("refused");
    let bus = bus(&dir);
    let service = FakeService::start(&bus, true);
    service.knobs.refuse_attach.store(true, Ordering::SeqCst);
    let helper = Helper::start(
        &bus,
        config(&dir, Duration::from_millis(100), Duration::from_millis(400)),
    );
    thread::sleep(Duration::from_millis(2500));
    // At 0, 0.1, 0.3, 0.7, 1.1, 1.5, 1.9 and 2.3 s.
    let tries = service.attaches();
    assert!((5..=10).contains(&tries), "{tries} tries");
    assert_eq!(helper.log.count("refused this session's sound"), 1);
    assert!(runs(&dir).is_empty());
}

// org.freedesktop.login1(5): seat0's ActiveSession changes when another session becomes active;
// the helper tries a refused Watch() again at once, long before its timer.
fn a_new_active_session_tries_a_refused_watch_at_once() {
    let dir = case("seat");
    let bus = bus(&dir);
    let logind = FakeLogind::start(&bus);
    let service = FakeService::start(&bus, true);
    service.knobs.refuse_watch.store(true, Ordering::SeqCst);
    let helper = Helper::start(&bus, config(&dir, 30 * SECOND, 60 * SECOND));
    assert!(within(SECOND, || service
        .knobs
        .watches
        .load(Ordering::SeqCst)
        == 1));
    thread::sleep(Duration::from_millis(300));
    assert_eq!(service.knobs.watches.load(Ordering::SeqCst), 1);
    assert_eq!(helper.log.count("doesn't let this session watch it"), 1);
    service.knobs.refuse_watch.store(false, Ordering::SeqCst);
    logind.new_active_session();
    assert!(
        within(SECOND, || runs(&dir).len() == 1),
        "watched, then attached and recording"
    );
}

// After a wake-up (owner, 2026-10-26): a recorder that ran a while and ended, while AudioWanted
// stays true, is started again at once, on a new pipe.
fn a_recorder_that_ends_after_a_while_is_started_again_at_once() {
    let dir = case("wake");
    fs::write(dir.join("mode"), "end-after 2300").expect("the mode");
    let bus = bus(&dir);
    let service = FakeService::start(&bus, true);
    let _helper = Helper::start(&bus, config(&dir, 2 * SECOND, 60 * SECOND));
    assert!(within(SECOND, || runs(&dir).len() == 1));
    let started = Instant::now();
    let pid = runs(&dir)[0].0;
    assert!(within(4 * SECOND, || gone(pid)), "the first one ended");
    let ended = Instant::now();
    assert!(
        within(2 * SECOND, || runs(&dir).len() == 2),
        "started again"
    );
    assert!(ended.duration_since(started) >= Duration::from_secs(2));
    assert!(within(2 * SECOND, || service.hears(0.25)), "on a new pipe");
    assert_eq!(service.attaches(), 2);
}

// A recorder that fails at once isn't started again and again: after the fallback timer, whose wait
// doubles.
fn a_recorder_that_fails_at_once_is_started_again_only_after_the_timer() {
    let dir = case("fail");
    fs::write(dir.join("mode"), "fail").expect("the mode");
    let bus = bus(&dir);
    let _service = FakeService::start(&bus, true);
    let helper = Helper::start(
        &bus,
        config(&dir, Duration::from_millis(200), Duration::from_millis(800)),
    );
    thread::sleep(Duration::from_millis(1700));
    // At 0, 0.2, 0.6 and 1.4 s.
    let started = runs(&dir).len();
    assert!((3..=5).contains(&started), "{started} starts");
    assert!(helper.log.count("pw-record ended") >= 1);
}

fn a_missing_pw_record_is_logged_once() {
    let dir = case("missing");
    let bus = bus(&dir);
    let service = FakeService::start(&bus, true);
    let mut settings = config(&dir, Duration::from_millis(100), Duration::from_millis(400));
    settings.program = dir.join("no-pw-record-here");
    let helper = Helper::start(&bus, settings);
    thread::sleep(Duration::from_millis(1500));
    assert!(service.attaches() >= 3, "{} tries", service.attaches());
    assert_eq!(
        helper
            .log
            .count("pw-record not found (package pipewire-audio)"),
        1
    );
    // The pipe it got was closed: the service hears it end.
    assert!(within(SECOND, || service.knobs.audio.holder().is_none()));
}

// The service leaving the bus stops the recorder; a new service is watched and attached to.
fn the_service_leaving_stops_the_recorder_and_its_return_is_watched_again() {
    let dir = case("service");
    let bus = bus(&dir);
    let service = FakeService::start(&bus, true);
    let helper = Helper::start(&bus, config(&dir, Duration::from_millis(300), 3 * SECOND));
    assert!(within(2 * SECOND, || runs(&dir).len() == 1));
    let pid = runs(&dir)[0].0;
    drop(service);
    assert!(within(SECOND, || gone(pid)), "stopped when it left");
    assert!(within(SECOND, || helper.log.count("isn't running") == 1));
    let back = FakeService::start(&bus, true);
    assert!(
        within(2 * SECOND, || runs(&dir).len() == 2),
        "attached again"
    );
    assert!(within(2 * SECOND, || back.hears(0.25)));
    assert_eq!(back.knobs.watches.load(Ordering::SeqCst), 1);
}

// A StateChanged that doesn't come from the service's owner is ignored: another process can't
// start the capture.
fn a_state_change_from_another_connection_is_ignored() {
    let dir = case("forged");
    let bus = bus(&dir);
    let service = FakeService::start(&bus, false);
    let _helper = Helper::start(&bus, config(&dir, Duration::from_millis(300), 3 * SECOND));
    assert!(within(SECOND, || !service
        .knobs
        .watchers
        .lock()
        .expect("the watchers")
        .is_empty()));
    let helper_name = service.knobs.watchers.lock().expect("the watchers")[0].clone();
    let forger = zbus::block_on(async {
        connection::Builder::address(bus.address())
            .build()
            .await
            .expect("a connection")
    });
    let wanted = State {
        audio_wanted: Some(true),
        ..State::default()
    };
    zbus::block_on(forger.emit_signal(
        Some(helper_name.as_str()),
        OBJECT_PATH,
        INTERFACE,
        "StateChanged",
        &(wanted.to_dict(),),
    ))
    .expect("sent");
    thread::sleep(Duration::from_millis(500));
    assert_eq!(service.attaches(), 0);
    assert!(runs(&dir).is_empty());
}
