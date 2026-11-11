//! The Start at boot switch's side of the client (Phase 11 plan, Task 3;
//! `docs/design/settings-app.md`, Start at boot). It reads the four facts from systemd, which
//! needs no password: `opendeepcool.service` on the system manager (the system bus, or the bus
//! `--service-address` names for the tests) and `opendeepcool-audio.service` on the user's own
//! manager (the session bus, or `--user-bus-address`); and logind's word on this session. It runs
//! a flip: `pkexec --disable-internal-agent` with the program, then the sound in this session
//! through the user's own manager, waiting for the job's end with `Subscribe()` on that manager
//! alone. Everything runs on GLib's main loop, one read at a time, never polling.

use std::cell::{Cell, RefCell};
use std::ffi::OsStr;
use std::future::{Future, poll_fn};
use std::path::PathBuf;
use std::pin::pin;
use std::rc::Rc;
use std::task::Poll;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};
use odc_boot::{HELPER, PROGRAM, SERVICE};
use zbus::message::Type;
use zbus::wire::{OwnedObjectPath, OwnedVariant};
use zbus::{Connection, MatchRule, MessageStream};

use crate::boot::{self, Boot, Direction, Exit, Facts, Failure, Flip, Known, Step, UnitFile};
use crate::client::next;
use crate::ui::window::Window;

/// pkexec, from the polkit package.
pub const PKEXEC: &str = "/usr/bin/pkexec";

const SYSTEMD: &str = "org.freedesktop.systemd1";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER: &str = "org.freedesktop.systemd1.Manager";
const UNIT: &str = "org.freedesktop.systemd1.Unit";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const LOGIND: &str = "org.freedesktop.login1";
const SESSION: &str = "org.freedesktop.login1.Session";

/// A read that hasn't answered within 1 s counts as failed (Where its state comes from, At
/// start).
const READ_WAIT: Duration = Duration::from_secs(1);
/// After a successful `on`, how long the flip waits for the service's bus name (The window
/// around a flip).
const NAME_WAIT: Duration = Duration::from_secs(5);
/// How long the sound's start or stop job may take; the unit is `Type=simple`, so its job ends at
/// once.
const JOB_WAIT: Duration = Duration::from_secs(30);

/// `future`'s output, or `None` when `duration` passes first, on GLib's main loop.
pub async fn within<F: Future>(duration: Duration, future: F) -> Option<F::Output> {
    let mut future = pin!(future);
    let mut timer = glib::timeout_future(duration);
    poll_fn(|cx| {
        if let Poll::Ready(output) = future.as_mut().poll(cx) {
            return Poll::Ready(Some(output));
        }
        if timer.as_mut().poll(cx).is_ready() {
            return Poll::Ready(None);
        }
        Poll::Pending
    })
    .await
}

fn log(line: &str) {
    eprintln!("opendeepcool: start at boot: {line}");
}

/// A call's reply, its value read as `T`.
async fn reply<T>(
    call: impl Future<Output = zbus::Result<zbus::Message>>,
    method: &str,
) -> Result<T, String>
where
    T: for<'d> zbus::wire::DynamicDeserialize<'d>,
{
    let reply = call.await.map_err(|e| format!("{method}: {e}"))?;
    reply
        .body()
        .deserialize::<T>()
        .map_err(|e| format!("{method}'s reply: {e}"))
}

/// A method of the manager at `/org/freedesktop/systemd1` on `conn`, taking `$args`.
macro_rules! manager {
    ($conn:expr, $method:literal, $args:expr) => {
        reply(
            $conn.call_method(Some(SYSTEMD), MANAGER_PATH, Some(MANAGER), $method, $args),
            $method,
        )
    };
}

/// A property of `path` on `interface`, from `destination`.
async fn property(
    conn: &Connection,
    destination: &str,
    path: &str,
    interface: &str,
    name: &str,
) -> Result<OwnedVariant, String> {
    let args = (interface, name);
    let call = conn.call_method(Some(destination), path, Some(PROPERTIES), "Get", &args);
    reply::<OwnedVariant>(call, name).await
}

async fn text_property(conn: &Connection, path: &str, name: &str) -> Result<String, String> {
    let value = property(conn, SYSTEMD, path, UNIT, name).await?;
    String::try_from(value).map_err(|e| format!("{name}: {e}"))
}

/// A unit's file state (`GetUnitFileState`), which fails for a unit that isn't installed.
async fn unit_file(conn: &Connection, unit: &str) -> Result<UnitFile, String> {
    let state: String = manager!(conn, "GetUnitFileState", &(unit,))
        .await
        .map_err(|e| format!("{unit}: {e}"))?;
    Ok(UnitFile::read(&state))
}

/// A unit's `ActiveState` and `SubState`, through `LoadUnit`: `GetUnit` fails for a unit systemd
/// hasn't loaded, and `LoadUnit` "will load the unit from disk if possible"
/// (org.freedesktop.systemd1(5); allowed without a password, checked on the development PC).
async fn unit_state(conn: &Connection, unit: &str) -> Result<(String, String), String> {
    let path: OwnedObjectPath = manager!(conn, "LoadUnit", &(unit,))
        .await
        .map_err(|e| format!("{unit}: {e}"))?;
    let active = text_property(conn, path.as_str(), "ActiveState")
        .await
        .map_err(|e| format!("{unit}: {e}"))?;
    let sub = text_property(conn, path.as_str(), "SubState")
        .await
        .map_err(|e| format!("{unit}: {e}"))?;
    Ok((active, sub))
}

/// logind's session of this app: `GetSession("auto")`.
pub async fn session_path(conn: &Connection) -> Option<OwnedObjectPath> {
    let reply = conn
        .call_method(
            Some(LOGIND),
            "/org/freedesktop/login1",
            Some("org.freedesktop.login1.Manager"),
            "GetSession",
            &("auto",),
        )
        .await
        .ok()?;
    reply.body().deserialize().ok()
}

/// The switch's side of the client.
pub struct Startup {
    window: Window,
    system: Connection,
    /// The user's own manager's bus; none when it couldn't be reached, which reads as failed.
    user: Option<Connection>,
    /// logind's session of this app; none without logind, when polkit alone decides.
    session: Option<OwnedObjectPath>,
    pkexec: PathBuf,
    boot: RefCell<Boot>,
    /// The service's bus name has an owner (the client follows it).
    owned: Cell<bool>,
    /// A read runs; another was asked meanwhile.
    reading: Cell<bool>,
    again: Cell<bool>,
    /// The wait after `on` for the service's name.
    name_wait: RefCell<Option<glib::SourceId>>,
}

impl Startup {
    pub fn new(
        window: Window,
        system: Connection,
        user: Option<Connection>,
        session: Option<OwnedObjectPath>,
        pkexec: PathBuf,
    ) -> Rc<Startup> {
        Rc::new(Startup {
            window,
            system,
            user,
            session,
            pkexec,
            boot: RefCell::new(Boot::default()),
            owned: Cell::new(false),
            reading: Cell::new(false),
            again: Cell::new(false),
            name_wait: RefCell::new(None),
        })
    }

    fn show(&self) {
        // Cloned first, so no borrow is held while the window draws it.
        let boot = self.boot.borrow().clone();
        self.window.boot(boot);
    }

    /// The service's bus name got or lost an owner: a flip waiting for it ends, and the facts
    /// are read again.
    pub fn owner(self: &Rc<Self>, owned: bool) {
        self.owned.set(owned);
        if owned {
            self.stop_name_wait();
            self.boot.borrow_mut().owned();
        }
        self.spawn_read();
    }

    /// Reads the facts again, on GLib's main loop.
    pub fn spawn_read(self: &Rc<Self>) {
        let startup = Rc::clone(self);
        glib::spawn_future_local(async move { startup.read().await });
    }

    /// Reads the four facts and the session, one read at a time, then shows them.
    pub async fn read(self: &Rc<Self>) {
        if self.reading.replace(true) {
            self.again.set(true);
            return;
        }
        loop {
            self.again.set(false);
            let known = match within(READ_WAIT, self.facts()).await {
                Some(Ok(facts)) => Known::Facts(facts),
                Some(Err(e)) => {
                    log(&format!("reading systemd failed: {e}"));
                    Known::Failed
                }
                None => {
                    log(&format!("systemd didn't answer within {READ_WAIT:?}"));
                    Known::Failed
                }
            };
            let may_flip = within(READ_WAIT, self.may_flip()).await.unwrap_or(true);
            {
                let mut boot = self.boot.borrow_mut();
                boot.read(known);
                boot.may_flip = may_flip;
            }
            self.show();
            if !self.again.get() {
                break;
            }
        }
        self.reading.set(false);
    }

    async fn facts(&self) -> Result<Facts, String> {
        let service_file = unit_file(&self.system, SERVICE).await?;
        let (active, sub) = unit_state(&self.system, SERVICE).await?;
        let service = boot::service_run(&active, &sub, self.owned.get())
            .ok_or_else(|| format!("{SERVICE}: ActiveState {active:?}"))?;
        let user = self
            .user
            .as_ref()
            .ok_or("the user's own manager: no session bus")?;
        let sound_file = unit_file(user, HELPER).await?;
        let (active, _) = unit_state(user, HELPER).await?;
        let sound =
            boot::sound_runs(&active).ok_or_else(|| format!("{HELPER}: ActiveState {active:?}"))?;
        Ok(Facts {
            service_file,
            service,
            sound_file,
            sound,
        })
    }

    /// Whether logind reports this session active and local; when it can't tell, polkit decides
    /// (Who may flip it).
    async fn may_flip(&self) -> bool {
        let Some(path) = &self.session else {
            return true;
        };
        let read = |name: &'static str| async move {
            property(&self.system, LOGIND, path.as_str(), SESSION, name)
                .await
                .ok()
                .and_then(|value| bool::try_from(value).ok())
        };
        match (read("Active").await, read("Remote").await) {
            (Some(active), Some(remote)) => active && !remote,
            _ => true,
        }
    }

    /// The user flipped the switch's handle to `on`.
    pub fn flip(self: &Rc<Self>, on: bool) {
        let to = if on { Direction::On } else { Direction::Off };
        let steps = self.boot.borrow_mut().begin(to);
        // Shown either way: a refused flip puts the handle back.
        self.show();
        let Some(steps) = steps else {
            return;
        };
        let startup = Rc::clone(self);
        glib::spawn_future_local(async move { startup.run(steps).await });
    }

    /// A flip's steps, the app held until it ends, so a closed window doesn't cut it short.
    async fn run(self: &Rc<Self>, steps: Vec<Step>) {
        let _hold = self.window.widget().application().map(|app| app.hold());
        let mut failure = None;
        for step in steps {
            failure = match step {
                Step::Pkexec(to) => self.pkexec(to).await,
                Step::StartSound => self.sound(true).await.err().map(|e| {
                    log(&format!("the sound didn't start in this session: {e}"));
                    Failure::SoundNotStarted
                }),
                Step::StopSound => self.sound(false).await.err().map(|e| {
                    log(&format!("the sound didn't stop in this session: {e}"));
                    Failure::SoundNotStopped
                }),
            };
            if failure.is_some() {
                break;
            }
        }
        self.boot.borrow_mut().end(failure, self.owned.get());
        self.read().await;
        if matches!(self.boot.borrow().flip, Flip::Waiting { .. }) {
            self.start_name_wait();
        }
    }

    fn start_name_wait(self: &Rc<Self>) {
        self.stop_name_wait();
        let weak = Rc::downgrade(self);
        let source = glib::timeout_add_local_once(NAME_WAIT, move || {
            if let Some(startup) = weak.upgrade() {
                startup.name_wait.borrow_mut().take();
                startup.boot.borrow_mut().owned();
                startup.spawn_read();
            }
        });
        *self.name_wait.borrow_mut() = Some(source);
    }

    fn stop_name_wait(&self) {
        if let Some(source) = self.name_wait.borrow_mut().take() {
            source.remove();
        }
    }

    /// `pkexec --disable-internal-agent /usr/lib/opendeepcool/opendeepcool-boot on|off`, as a
    /// subprocess, without blocking the main loop and with no time limit on the password dialog;
    /// its output goes to the app's log only.
    async fn pkexec(&self, to: Direction) -> Option<Failure> {
        let argv = [
            self.pkexec.as_os_str(),
            OsStr::new("--disable-internal-agent"),
            OsStr::new(PROGRAM),
            OsStr::new(to.arg()),
        ];
        let flags = gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_PIPE;
        let process = match gio::Subprocess::newv(&argv, flags) {
            Ok(process) => process,
            Err(e) => {
                log(&format!("pkexec can't start: {e}"));
                return boot::pkexec_failure(to, &Exit::NotStarted(e.to_string()), "");
            }
        };
        // It returns once the process has ended and its output is read (g_subprocess_communicate).
        let (stdout, stderr) = match process.communicate_utf8_future(None).await {
            Ok((stdout, stderr)) => (
                stdout.map(|s| s.to_string()).unwrap_or_default(),
                stderr.map(|s| s.to_string()).unwrap_or_default(),
            ),
            Err(e) => {
                log(&format!("pkexec's output: {e}"));
                let _ = process.wait_future().await;
                (String::new(), String::new())
            }
        };
        let exit = if process.has_exited() {
            Exit::Code(process.exit_status())
        } else if process.has_signaled() {
            Exit::Signal(process.term_sig())
        } else {
            Exit::NotStarted("it didn't end".to_owned())
        };
        log(&format!(
            "pkexec … {}: {exit:?}; {}{}",
            to.arg(),
            stdout.trim_end(),
            stderr.trim_end()
        ));
        boot::pkexec_failure(to, &exit, &stderr)
    }

    /// `StartUnit` or `StopUnit` of the sound on the user's own manager, then its job's end:
    /// `JobRemoved` for that job, after `Subscribe()` on that manager alone ("Signals are only
    /// sent out if at least one client invoked this method", org.freedesktop.systemd1(5)).
    async fn sound(&self, start: bool) -> Result<(), String> {
        let user = self
            .user
            .as_ref()
            .ok_or("the user's own manager: no session bus")?;
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(SYSTEMD)
            .path(MANAGER_PATH)
            .interface(MANAGER)
            .member("JobRemoved")
            .build()
            .map_err(|e| e.to_string())?;
        let mut jobs = MessageStream::for_match_rule(rule, user, None)
            .await
            .map_err(|e| e.to_string())?;
        let (): () = manager!(user, "Subscribe", &()).await?;
        let result = self.job(user, start, &mut jobs).await;
        let unsubscribed: Result<(), String> = manager!(user, "Unsubscribe", &()).await;
        if let Err(e) = unsubscribed {
            log(&format!("Unsubscribe: {e}"));
        }
        result
    }

    async fn job(
        &self,
        user: &Connection,
        start: bool,
        jobs: &mut MessageStream,
    ) -> Result<(), String> {
        let job: OwnedObjectPath = if start {
            manager!(user, "StartUnit", &(HELPER, "replace")).await?
        } else {
            manager!(user, "StopUnit", &(HELPER, "replace")).await?
        };
        let method = if start { "StartUnit" } else { "StopUnit" };
        let ended = within(JOB_WAIT, async {
            while let Some(message) = next(jobs).await {
                if let Ok((_, path, _, result)) =
                    message
                        .body()
                        .deserialize::<(u32, OwnedObjectPath, String, String)>()
                    && path == job
                {
                    return Some(result);
                }
            }
            None
        })
        .await;
        match ended {
            Some(Some(result)) if result == "done" => Ok(()),
            Some(Some(result)) => Err(format!("{method}'s job ended {result:?}")),
            Some(None) => Err("the session bus closed".to_owned()),
            None => Err(format!("{method}'s job didn't end within {JOB_WAIT:?}")),
        }
    }
}
