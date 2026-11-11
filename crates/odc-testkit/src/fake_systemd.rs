//! A fake systemd and logind for the settings app's Start at boot switch (Phase 11 plan, Task 3):
//! the system manager, with `opendeepcool.service`, and logind's session of the app on the
//! private system bus; the user's own manager, with `opendeepcool-audio.service`, on a private
//! session bus. Only what the app calls, as org.freedesktop.systemd1(5) and
//! org.freedesktop.login1(5) describe it. It takes its state from a file the test writes and
//! logs every call, as the fake service does, so `odc-ui-e2e`, which links another major version
//! of zbus, steers it with files alone. The fake `pkexec` (`odc-fake-pkexec`) is the other half:
//! it never changes this state; the test writes what a run of the program would have left.
//!
//! `systemd.toml` is written whole, then moved into place:
//!
//! ```toml
//! serial = 3            # logged as `state 3` once the file is taken
//!
//! [service]             # opendeepcool.service on the system manager
//! file = "enabled"      # GetUnitFileState's answer; "" fails the call, as for a unit that
//!                       # isn't installed
//! active = "active"     # ActiveState
//! sub = "running"       # SubState
//! delay_ms = 0          # GetUnitFileState answers after this long
//!
//! [sound]               # opendeepcool-audio.service on the user's own manager
//! file = "enabled"
//! active = "active"
//! job = "done"          # JobRemoved's result for StartUnit and StopUnit; "" fails the call;
//!                       # "done" sets ActiveState to active or inactive
//!
//! [session]             # logind's session of the app
//! active = true
//! remote = false
//! ```
//!
//! Before a file is taken, it answers as a new install: both units disabled and inactive, jobs
//! done, the session active and local. `systemd.log` gets one line per call, such as
//! `user StartUnit("opendeepcool-audio.service", "replace") done`, and `state <serial>`, or
//! `state error: <why>`, for each file taken.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zbus::object_server::SignalEmitter;
use zbus::wire::{ObjectPath, OwnedObjectPath};
use zbus::{Connection, connection, fdo};

use crate::bus::sleep;

/// The file the test writes, and the log the fake writes, in the fake's folder.
pub const STATE_FILE: &str = "systemd.toml";
pub const LOG: &str = "systemd.log";

/// The units it knows.
pub const SERVICE: &str = "opendeepcool.service";
pub const SOUND: &str = "opendeepcool-audio.service";

const SYSTEMD: &str = "org.freedesktop.systemd1";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const LOGIND: &str = "org.freedesktop.login1";
const LOGIND_PATH: &str = "/org/freedesktop/login1";
/// The session `GetSession` gives.
pub const SESSION_PATH: &str = "/org/freedesktop/login1/session/fake";

/// How often the fake looks at the state file.
const POLL: Duration = Duration::from_millis(10);

/// What the state file says, and what the jobs changed since.
#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    serial: u64,
    service_file: String,
    service_active: String,
    service_sub: String,
    service_delay: Duration,
    sound_file: String,
    sound_active: String,
    job: String,
    session_active: bool,
    session_remote: bool,
}

impl Default for State {
    /// A new install.
    fn default() -> State {
        State {
            serial: 0,
            service_file: "disabled".to_owned(),
            service_active: "inactive".to_owned(),
            service_sub: "dead".to_owned(),
            service_delay: Duration::ZERO,
            sound_file: "disabled".to_owned(),
            sound_active: "inactive".to_owned(),
            job: "done".to_owned(),
            session_active: true,
            session_remote: false,
        }
    }
}

struct Shared {
    log: Mutex<File>,
    state: Mutex<State>,
    jobs: AtomicU32,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Shared {
    fn log(&self, line: &str) {
        let mut log = lock(&self.log);
        // A log the test can't read fails the test, which reads it.
        let _ = writeln!(log, "{line}");
        let _ = log.flush();
    }
}

/// Which manager: the system's, or the user's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Which {
    System,
    User,
}

impl Which {
    fn unit(self) -> &'static str {
        match self {
            Which::System => SERVICE,
            Which::User => SOUND,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Which::System => "system",
            Which::User => "user",
        }
    }
}

/// A unit's object path, escaped as systemd escapes it (`.` as `_2e`, `-` as `_2d`).
fn unit_path(unit: &str) -> OwnedObjectPath {
    let mut path = String::from("/org/freedesktop/systemd1/unit/");
    for byte in unit.bytes() {
        if byte.is_ascii_alphanumeric() {
            path.push(char::from(byte));
        } else {
            path.push_str(&format!("_{byte:02x}"));
        }
    }
    OwnedObjectPath::try_from(path).expect("an escaped path is valid")
}

fn no_unit(unit: &str) -> fdo::Error {
    fdo::Error::FileNotFound(format!("No such file or directory: {unit}"))
}

struct Manager {
    shared: Arc<Shared>,
    which: Which,
}

impl Manager {
    /// The unit this manager knows, or the error for any other.
    fn known(&self, unit: &str) -> fdo::Result<()> {
        if unit == self.which.unit() {
            Ok(())
        } else {
            Err(fdo::Error::Failed(format!("Unit {unit} not found.")))
        }
    }

    /// A start or stop job: logged, its result as the state file says, then `JobRemoved`.
    async fn job(
        &self,
        method: &str,
        unit: &str,
        mode: &str,
        emitter: &SignalEmitter<'_>,
    ) -> fdo::Result<OwnedObjectPath> {
        let result = lock(&self.shared.state).job.clone();
        let call = format!("{} {method}({unit:?}, {mode:?})", self.which.word());
        if result.is_empty() || self.known(unit).is_err() {
            self.shared.log(&format!("{call} failed"));
            return Err(fdo::Error::Failed(format!("{method} failed in the fake")));
        }
        if result == "done" {
            let mut state = lock(&self.shared.state);
            state.sound_active = if method == "StartUnit" {
                "active"
            } else {
                "inactive"
            }
            .to_owned();
        }
        self.shared.log(&format!("{call} {result}"));
        let id = self.shared.jobs.fetch_add(1, Ordering::Relaxed) + 1;
        let job = OwnedObjectPath::try_from(format!("/org/freedesktop/systemd1/job/{id}"))
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Manager::job_removed(emitter, id, (*job).clone(), unit, &result)
            .await
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(job)
    }
}

#[zbus::interface(name = "org.freedesktop.systemd1.Manager", introspection_docs = false)]
impl Manager {
    async fn get_unit_file_state(&self, file: &str) -> fdo::Result<String> {
        let (state, delay) = {
            let state = lock(&self.shared.state);
            match self.which {
                Which::System => (state.service_file.clone(), state.service_delay),
                Which::User => (state.sound_file.clone(), Duration::ZERO),
            }
        };
        self.shared
            .log(&format!("{} GetUnitFileState({file:?})", self.which.word()));
        if !delay.is_zero() {
            sleep(delay).await;
        }
        self.known(file)?;
        if state.is_empty() {
            return Err(no_unit(file));
        }
        Ok(state)
    }

    async fn load_unit(&self, name: &str) -> fdo::Result<OwnedObjectPath> {
        self.shared
            .log(&format!("{} LoadUnit({name:?})", self.which.word()));
        self.known(name)?;
        Ok(unit_path(name))
    }

    async fn start_unit(
        &self,
        name: &str,
        mode: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<OwnedObjectPath> {
        self.job("StartUnit", name, mode, &emitter).await
    }

    async fn stop_unit(
        &self,
        name: &str,
        mode: &str,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> fdo::Result<OwnedObjectPath> {
        self.job("StopUnit", name, mode, &emitter).await
    }

    async fn subscribe(&self) {
        self.shared
            .log(&format!("{} Subscribe()", self.which.word()));
    }

    async fn unsubscribe(&self) {
        self.shared
            .log(&format!("{} Unsubscribe()", self.which.word()));
    }

    #[zbus(signal)]
    async fn job_removed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        job: ObjectPath<'_>,
        unit: &str,
        result: &str,
    ) -> zbus::Result<()>;
}

/// A unit's two properties the app reads.
struct Unit {
    shared: Arc<Shared>,
    which: Which,
}

#[zbus::interface(name = "org.freedesktop.systemd1.Unit", introspection_docs = false)]
impl Unit {
    #[zbus(property)]
    async fn active_state(&self) -> String {
        let state = lock(&self.shared.state);
        match self.which {
            Which::System => state.service_active.clone(),
            Which::User => state.sound_active.clone(),
        }
    }

    #[zbus(property)]
    async fn sub_state(&self) -> String {
        let state = lock(&self.shared.state);
        match self.which {
            Which::System => state.service_sub.clone(),
            // The sound's substate isn't read; systemd's for a running simple service.
            Which::User if state.sound_active == "active" => "running".to_owned(),
            Which::User => "dead".to_owned(),
        }
    }
}

struct LoginManager {
    shared: Arc<Shared>,
}

#[zbus::interface(name = "org.freedesktop.login1.Manager", introspection_docs = false)]
impl LoginManager {
    async fn get_session(&self, session_id: &str) -> OwnedObjectPath {
        self.shared.log(&format!("GetSession({session_id:?})"));
        OwnedObjectPath::try_from(SESSION_PATH).expect("a valid path")
    }
}

struct Session {
    shared: Arc<Shared>,
}

#[zbus::interface(name = "org.freedesktop.login1.Session", introspection_docs = false)]
impl Session {
    #[zbus(property)]
    async fn active(&self) -> bool {
        lock(&self.shared.state).session_active
    }

    #[zbus(property)]
    async fn remote(&self) -> bool {
        lock(&self.shared.state).session_remote
    }
}

/// The fake, serving until dropped.
pub struct FakeSystemd {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeSystemd {
    /// Connects to the system bus at `system` and the session bus at `user`, owns systemd's
    /// name on both and logind's on the first, and serves there, with its files in `dir`;
    /// returns once it serves.
    ///
    /// # Errors
    ///
    /// When the log can't be created, or a bus can't be reached or its name taken.
    pub fn start(system: &str, user: &str, dir: &Path) -> io::Result<FakeSystemd> {
        fs::create_dir_all(dir)?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(LOG))?;
        let shared = Arc::new(Shared {
            log: Mutex::new(log),
            state: Mutex::new(State::default()),
            jobs: AtomicU32::new(0),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let (ready, started) = mpsc::channel();
        let (system, user, dir, flag) = (
            system.to_owned(),
            user.to_owned(),
            dir.to_owned(),
            Arc::clone(&stop),
        );
        let thread = thread::spawn(move || run(&system, &user, &dir, &shared, &flag, &ready));
        match started.recv() {
            Ok(Ok(())) => Ok(FakeSystemd {
                stop,
                thread: Some(thread),
            }),
            Ok(Err(e)) => Err(io::Error::other(e)),
            Err(_) => Err(io::Error::other("the fake systemd's thread ended")),
        }
    }
}

impl Drop for FakeSystemd {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Both connections, serving.
async fn connect(system: &str, user: &str, shared: &Arc<Shared>) -> zbus::Result<[Connection; 2]> {
    let unit = |which| Unit {
        shared: Arc::clone(shared),
        which,
    };
    let manager = |which| Manager {
        shared: Arc::clone(shared),
        which,
    };
    let system = connection::Builder::address(system)
        .serve_at(MANAGER_PATH, manager(Which::System))
        .serve_at(unit_path(SERVICE), unit(Which::System))
        .serve_at(
            LOGIND_PATH,
            LoginManager {
                shared: Arc::clone(shared),
            },
        )
        .serve_at(
            SESSION_PATH,
            Session {
                shared: Arc::clone(shared),
            },
        )
        .name(SYSTEMD)
        .name(LOGIND)
        .build()
        .await?;
    let user = connection::Builder::address(user)
        .serve_at(MANAGER_PATH, manager(Which::User))
        .serve_at(unit_path(SOUND), unit(Which::User))
        .name(SYSTEMD)
        .build()
        .await?;
    Ok([system, user])
}

/// The fake's thread: the connections, then the state file, looked at every 10 ms. zbus runs
/// the connections on threads of their own, so the calls are answered meanwhile.
fn run(
    system: &str,
    user: &str,
    dir: &Path,
    shared: &Arc<Shared>,
    stop: &AtomicBool,
    ready: &mpsc::Sender<Result<(), String>>,
) {
    let conns = match zbus::block_on(connect(system, user, shared)) {
        Ok(conns) => conns,
        Err(e) => {
            let _ = ready.send(Err(e.to_string()));
            return;
        }
    };
    let _ = ready.send(Ok(()));
    let path = dir.join(STATE_FILE);
    let mut seen = None;
    while !stop.load(Ordering::Relaxed) {
        let stamp = fs::metadata(&path)
            .ok()
            .map(|m| (m.ino(), m.mtime(), m.mtime_nsec(), m.len()));
        if stamp.is_some() && stamp != seen {
            seen = stamp;
            zbus::block_on(take(&conns[0], &path, shared));
        }
        thread::sleep(POLL);
    }
}

/// Takes the state file; a change of the session's properties is signalled, as logind does.
async fn take(system: &Connection, path: &Path, shared: &Shared) {
    let parsed = fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|text| parse(&text));
    let new = match parsed {
        Ok(new) => new,
        Err(why) => {
            shared.log(&format!("state error: {why}"));
            return;
        }
    };
    let serial = new.serial;
    let old = std::mem::replace(&mut *lock(&shared.state), new.clone());
    if (old.session_active, old.session_remote) != (new.session_active, new.session_remote)
        && let Ok(session) = system
            .object_server()
            .interface::<_, Session>(SESSION_PATH)
            .await
    {
        let emitter = session.signal_emitter();
        let session = session.get().await;
        let _ = session.active_changed(emitter).await;
        let _ = session.remote_changed(emitter).await;
    }
    shared.log(&format!("state {serial}"));
}

/// A state file.
fn parse(text: &str) -> Result<State, String> {
    let table: toml::Table = text.parse().map_err(|e| format!("{e}"))?;
    let section = |name: &str| {
        table
            .get(name)
            .and_then(toml::Value::as_table)
            .ok_or(format!("no [{name}] table"))
    };
    let text = |table: &toml::Table, section: &str, key: &str| {
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .ok_or(format!("[{section}] {key} isn't text"))
    };
    let flag = |table: &toml::Table, key: &str| {
        table
            .get(key)
            .and_then(toml::Value::as_bool)
            .ok_or(format!("[session] {key} isn't true or false"))
    };
    let serial = table
        .get("serial")
        .and_then(toml::Value::as_integer)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or("serial isn't a number")?;
    let service = section("service")?;
    let sound = section("sound")?;
    let session = section("session")?;
    let delay = match service.get("delay_ms") {
        None => 0,
        Some(value) => value
            .as_integer()
            .and_then(|n| u64::try_from(n).ok())
            .ok_or("[service] delay_ms isn't a number")?,
    };
    Ok(State {
        serial,
        service_file: text(service, "service", "file")?,
        service_active: text(service, "service", "active")?,
        service_sub: text(service, "service", "sub")?,
        service_delay: Duration::from_millis(delay),
        sound_file: text(sound, "sound", "file")?,
        sound_active: text(sound, "sound", "active")?,
        job: text(sound, "sound", "job")?,
        session_active: flag(session, "active")?,
        session_remote: flag(session, "remote")?,
    })
}

/// The fake `pkexec`'s files, in the fake systemd's folder: the call log it writes, and the
/// answer the test writes for it (`odc-fake-pkexec`).
pub const PKEXEC_LOG: &str = "pkexec.log";
pub const PKEXEC_ANSWER: &str = "pkexec.answer";
/// The name the app is given it by, a symbolic link in the folder, so it finds its folder.
pub const PKEXEC_LINK: &str = "pkexec";
/// How long the fake `pkexec` waits for its answer before it ends as no agent would.
pub const PKEXEC_WAIT: Duration = Duration::from_secs(30);

/// The fake `pkexec` (`odc-fake-pkexec`): appends `pkexec <args>` to the call log in its folder,
/// waits for the answer file, takes it (`exit <code>`, then the lines it writes to standard
/// error), and ends with that code; with no answer within [`PKEXEC_WAIT`], it ends with 127. The
/// folder is its first argument's, the link the app was given.
///
/// # Errors
///
/// When its folder or its files can't be used; the binary then ends with 127.
pub fn fake_pkexec(argv0: &Path, args: &[String]) -> Result<(i32, String), String> {
    let dir = argv0
        .parent()
        .ok_or_else(|| format!("{} has no folder", argv0.display()))?;
    let mut line = String::from("pkexec");
    for arg in args {
        line.push(' ');
        line.push_str(arg);
    }
    let log = dir.join(PKEXEC_LOG);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .and_then(|mut file| writeln!(file, "{line}"))
        .map_err(|e| format!("{}: {e}", log.display()))?;
    let answer = dir.join(PKEXEC_ANSWER);
    let started = std::time::Instant::now();
    loop {
        match fs::read_to_string(&answer) {
            Ok(text) => {
                fs::remove_file(&answer).map_err(|e| format!("{}: {e}", answer.display()))?;
                let (first, rest) = text.split_once('\n').unwrap_or((text.as_str(), ""));
                let code = first
                    .strip_prefix("exit ")
                    .and_then(|code| code.trim().parse().ok())
                    .ok_or_else(|| format!("{}: no \"exit <code>\" line", answer.display()))?;
                return Ok((code, rest.to_owned()));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", answer.display())),
        }
        if started.elapsed() > PKEXEC_WAIT {
            return Ok((127, "the fake pkexec got no answer\n".to_owned()));
        }
        thread::sleep(POLL);
    }
}
