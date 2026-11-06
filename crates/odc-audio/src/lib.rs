//! `opendeepcool-audio`, the session's sound helper (Phase 9 plan, Task 4; `docs/spec/dbus.md`,
//! `AttachAudio()`): while the display service shows a Music mode (its `AudioWanted`), the helper
//! asks it for a pipe and runs PipeWire's own `pw-record` on the default output's monitor,
//! writing straight into it. The helper only connects the sound; the service analyses it.
//!
//! It follows events and never polls (owner, 2026-10-26): the service's `StateChanged`, after a
//! `Watch()` that passed; the service's bus name changing owner; and logind's seat0 announcing a
//! new active session. A refused call is tried again on the next such change, or by a fallback
//! timer while the service wants sound, from 2 s doubling up to 60 s. It logs only changes of what
//! it's doing, never each try, and never any sound.

use std::future::{Future, poll_fn};
use std::io::ErrorKind;
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::pin::{Pin, pin};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

use futures_core::Stream;
use odc_ipc::{BUS_NAME, Error, INTERFACE, OBJECT_PATH, OpenDeepCoolProxy, State};
use zbus::fdo::DBusProxy;
use zbus::message::Type;
use zbus::names::BusName;
use zbus::wire::OwnedVariant;
use zbus::{Connection, MatchRule, Message, MessageStream};

/// PipeWire's recorder, found on `PATH`.
pub const PW_RECORD: &str = "pw-record";

/// logind, and the PC's one seat (owner, 2026-10-26: one user at a time on one seat).
const LOGIND: &str = "org.freedesktop.login1";
const SEAT: &str = "/org/freedesktop/login1/seat/seat0";

/// The bus itself, which alone sends `NameOwnerChanged`.
const BUS_DRIVER: &str = "org.freedesktop.DBus";

const USAGE: &str = "usage: opendeepcool-audio [--target <node>]";

/// `pw-record`'s arguments: raw `f32` stereo frames at 48 000 a second, 20 ms of latency, the
/// default output's monitor (`stream.capture.sink`), as a node named `opendeepcool-audio`,
/// written to standard output; `target` names another node, for the end-to-end test only.
pub fn pw_record_args(target: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = [
        "--raw",
        "--format",
        "f32",
        "--rate",
        "48000",
        "--channels",
        "2",
        "--latency",
        "20ms",
        "-P",
        r#"{ "stream.capture.sink": true, "node.name": "opendeepcool-audio", "node.description": "OpenDeepCool cooler display" }"#,
    ]
    .map(str::to_owned)
    .into();
    if let Some(node) = target {
        args.extend(["--target".to_owned(), node.to_owned()]);
    }
    args.push("-".to_owned());
    args
}

/// The helper's flags: only `--target <node>`, once; anything else is an error, so a typo can't
/// start a different helper.
///
/// # Errors
///
/// The usage, with what was wrong.
pub fn parse_flags(args: &[String]) -> Result<Option<String>, String> {
    match args {
        [] => Ok(None),
        [flag, node] if flag == "--target" && !node.is_empty() && !node.starts_with('-') => {
            Ok(Some(node.clone()))
        }
        [flag] if flag == "--target" => Err(format!("--target needs a node\n{USAGE}")),
        _ => Err(format!("unexpected arguments {args:?}\n{USAGE}")),
    }
}

/// What the helper runs, and how long it waits before trying again.
#[derive(Clone, Debug)]
pub struct Config {
    /// `pw-record`, or a test's fake.
    pub program: PathBuf,
    pub target: Option<String>,
    /// The fallback timer's first wait, and its longest.
    pub first_wait: Duration,
    pub longest_wait: Duration,
}

impl Config {
    /// `pw-record` from `PATH`, and the fallback timer from 2 s doubling to 60 s.
    pub fn new(target: Option<String>) -> Config {
        Config {
            program: PathBuf::from(PW_RECORD),
            target,
            first_wait: Duration::from_secs(2),
            longest_wait: Duration::from_secs(60),
        }
    }
}

/// How important a journal line is (sd-daemon(3)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Error = 3,
    Warning = 4,
    Notice = 5,
    Info = 6,
}

/// Where the helper's lines go: the journal, through standard error, or a test's record.
pub trait Log: Sync {
    fn write(&self, priority: Priority, line: &str);
}

/// Standard error, each line with its priority for the journal.
pub struct Stderr;

impl Log for Stderr {
    fn write(&self, priority: Priority, line: &str) {
        for part in line.lines() {
            eprintln!("<{}>{part}", priority as u8);
        }
    }
}

/// What the helper is doing; a line is logged only when it changes.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Doing {
    Starting,
    NoService,
    WatchRefused(String),
    NotWanted,
    Refused(String),
    NoRecorder,
    RecorderFailed(String),
    Ended(String),
    Capturing,
}

impl Doing {
    fn line(&self, target: Option<&str>) -> (Priority, String) {
        match self {
            Doing::Starting => (Priority::Info, "starting".into()),
            Doing::NoService => (
                Priority::Notice,
                "the display service isn't running: waiting for it".into(),
            ),
            Doing::WatchRefused(why) => (
                Priority::Notice,
                format!(
                    "the display service doesn't let this session watch it: {why}; trying again \
                     on a change"
                ),
            ),
            Doing::NotWanted => (
                Priority::Info,
                "the display shows no Music mode: not capturing".into(),
            ),
            Doing::Refused(why) => (
                Priority::Notice,
                format!(
                    "the display service refused this session's sound: {why}; trying again on a \
                     change"
                ),
            ),
            Doing::NoRecorder => (
                Priority::Warning,
                "pw-record not found (package pipewire-audio)".into(),
            ),
            Doing::RecorderFailed(why) => {
                (Priority::Warning, format!("pw-record can't start: {why}"))
            }
            Doing::Ended(status) => (Priority::Info, format!("pw-record ended ({status})")),
            Doing::Capturing => (
                Priority::Info,
                match target {
                    Some(node) => format!("capturing {node} for the display service"),
                    None => "capturing the default output's sound for the display service".into(),
                },
            ),
        }
    }
}

/// An event the loop wakes for.
enum Event {
    /// A `StateChanged` addressed to the helper.
    State(Message),
    /// The service's bus name changed owner.
    Owner(Message),
    /// logind's seat0 changed a property, its active session among them.
    Seat,
    /// The fallback timer.
    Timer,
    /// A message that couldn't be read: nothing to do.
    Nothing,
    /// The caller's stop.
    Stop,
    /// A stream ended: the connection is gone.
    Closed,
}

/// A wait on a thread of its own: zbus 6 has no public timer, and the helper has no async runtime
/// of its own.
struct Timer {
    at: Instant,
    fired: Arc<Mutex<(bool, Option<Waker>)>>,
}

impl Timer {
    fn at(at: Instant) -> Timer {
        let fired = Arc::new(Mutex::new((false, None::<Waker>)));
        let ring = Arc::clone(&fired);
        thread::spawn(move || {
            thread::sleep(at.saturating_duration_since(Instant::now()));
            let mut state = ring.lock().unwrap_or_else(|p| p.into_inner());
            state.0 = true;
            if let Some(waker) = state.1.take() {
                waker.wake();
            }
        });
        Timer { at, fired }
    }

    fn poll(&self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        let mut state = self.fired.lock().unwrap_or_else(|p| p.into_inner());
        if state.0 {
            return Poll::Ready(());
        }
        state.1 = Some(cx.waker().clone());
        Poll::Pending
    }
}

/// The running `pw-record` and when it started.
struct Recorder {
    child: Child,
    started: Instant,
}

/// The helper's state between events.
struct Helper<'a> {
    config: &'a Config,
    log: &'a dyn Log,
    service: OpenDeepCoolProxy<'a>,
    /// The service's unique name, while it's on the bus: only its `StateChanged` counts.
    owner: Option<String>,
    watching: bool,
    wanted: bool,
    recorder: Option<Recorder>,
    /// When to try again; due now when a watched change says so.
    retry_at: Option<Instant>,
    /// The fallback timer's next wait.
    wait: Duration,
    doing: Doing,
}

impl<'a> Helper<'a> {
    fn say(&mut self, doing: Doing) {
        if self.doing != doing {
            let (priority, line) = doing.line(self.config.target.as_deref());
            self.log.write(priority, &line);
            self.doing = doing;
        }
    }

    /// A watched change: try again now, the fallback timer from its first wait.
    fn now(&mut self) {
        self.retry_at = Some(Instant::now());
        self.wait = self.config.first_wait;
    }

    /// A try that failed: the next after the fallback timer's wait, which doubles.
    fn later(&mut self) {
        self.retry_at = Some(Instant::now() + self.wait);
        self.wait = (self.wait * 2).min(self.config.longest_wait);
    }

    fn stop_recorder(&mut self) {
        if let Some(mut recorder) = self.recorder.take() {
            // Already gone, it can't be killed; either way it's reaped.
            let _ = recorder.child.kill();
            let _ = recorder.child.wait();
        }
    }

    /// The recorder ended: its pipe closed, or it failed. While sound is still wanted it starts
    /// again, at once when it had run for the fallback timer's first wait, else after the timer,
    /// so one that fails at once isn't started again and again.
    fn ended(&mut self, why: String, ran: Duration) {
        self.say(Doing::Ended(why));
        if !self.wanted {
            return;
        }
        if ran >= self.config.first_wait {
            self.now();
        } else {
            self.later();
        }
    }

    /// Reaps a recorder that ended.
    fn reap(&mut self) {
        let Some(recorder) = self.recorder.as_mut() else {
            return;
        };
        if let Ok(Some(status)) = recorder.child.try_wait() {
            let ran = recorder.started.elapsed();
            self.recorder = None;
            self.ended(status.to_string(), ran);
        }
    }

    /// The keys of the service's state that the helper reads.
    fn take_state(&mut self, state: &State) {
        if let Some(wanted) = state.audio_wanted
            && wanted != self.wanted
        {
            self.wanted = wanted;
            if wanted {
                self.now();
            }
        }
    }

    /// `StateChanged` from the service: `AudioWanted`, and `AudioConnected` false while the
    /// helper records, which may say the service closed its pipe. Signals sent before a
    /// reply arrive after it, so the state is asked again before the recorder is stopped.
    async fn state_changed(&mut self, message: &Message) {
        let header = message.header();
        if header.sender().map(|s| s.as_str()) != self.owner.as_deref() {
            return;
        }
        let Ok((changes,)) = message
            .body()
            .deserialize::<(std::collections::HashMap<String, OwnedVariant>,)>()
        else {
            return;
        };
        let Ok(state) = State::from_dict(&changes) else {
            return;
        };
        self.take_state(&state);
        if state.audio_connected == Some(false) && self.recorder.is_some() {
            let now = self.current_state().await;
            if let Some(state) = &now {
                self.take_state(state);
            }
            if now.and_then(|s| s.audio_connected) == Some(false)
                && let Some(recorder) = self.recorder.as_ref()
            {
                let ran = recorder.started.elapsed();
                self.stop_recorder();
                self.ended("its pipe closed".into(), ran);
            }
        }
    }

    async fn current_state(&self) -> Option<State> {
        let dict = self.service.get_state().await.ok()?;
        State::from_dict(&dict).ok()
    }

    /// The service's name changed owner: a new service starts with no watcher and no pipe.
    fn owner_changed(&mut self, message: &Message) {
        let header = message.header();
        if header.sender().map(|s| s.as_str()) != Some(BUS_DRIVER) {
            return;
        }
        let Ok((name, _, new_owner)) = message.body().deserialize::<(String, String, String)>()
        else {
            return;
        };
        if name != BUS_NAME {
            return;
        }
        self.stop_recorder();
        self.watching = false;
        self.wanted = false;
        self.owner = (!new_owner.is_empty()).then_some(new_owner);
        self.now();
    }

    /// Brings the helper in line with what it knows: watching the service, and recording while
    /// it wants sound.
    async fn settle(&mut self) {
        self.reap();
        if self.owner.is_none() {
            self.stop_recorder();
            self.retry_at = None;
            self.say(Doing::NoService);
            return;
        }
        let due = self.retry_at.is_some_and(|at| Instant::now() >= at);
        if !self.watching {
            if !due {
                return;
            }
            if let Err(e) = self.service.watch().await {
                self.say(Doing::WatchRefused(reason(&e)));
                self.later();
                return;
            }
            self.watching = true;
            self.retry_at = None;
            match self.current_state().await {
                Some(state) => {
                    self.wanted = state.audio_wanted == Some(true);
                    if self.wanted {
                        self.now();
                    }
                }
                None => self.wanted = false,
            }
        }
        if !self.wanted {
            self.stop_recorder();
            self.retry_at = None;
            self.say(Doing::NotWanted);
            return;
        }
        if self.recorder.is_some() {
            self.retry_at = None;
            return;
        }
        match self.retry_at {
            None => self.now(),
            Some(at) if Instant::now() < at => return,
            Some(_) => {}
        }
        self.retry_at = None;
        match self.service.attach_audio().await {
            Ok(pipe) => match self.start(OwnedFd::from(pipe)) {
                Ok(child) => {
                    self.recorder = Some(Recorder {
                        child,
                        started: Instant::now(),
                    });
                    self.say(Doing::Capturing);
                }
                // The pipe is dropped: the service hears it close.
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    self.say(Doing::NoRecorder);
                    self.later();
                }
                Err(e) => {
                    self.say(Doing::RecorderFailed(e.to_string()));
                    self.later();
                }
            },
            Err(Error::AudioNotWanted(_)) => {
                self.wanted = false;
                self.say(Doing::NotWanted);
            }
            Err(e) => {
                self.say(Doing::Refused(reason(&e)));
                self.later();
            }
        }
    }

    /// Starts `pw-record` writing into `pipe`; the helper's own copy is closed once the child
    /// has it, so the pipe closes when the child ends.
    fn start(&self, pipe: OwnedFd) -> std::io::Result<Child> {
        let mut command = Command::new(&self.config.program);
        command
            .args(pw_record_args(self.config.target.as_deref()))
            .stdin(Stdio::null())
            .stdout(Stdio::from(pipe));
        let child = command.spawn();
        drop(command);
        child
    }
}

/// An error's text for the journal: the service's own reason when it gave one.
fn reason(error: &Error) -> String {
    use zbus::DBusError;
    match error.description() {
        Some(text) => text.to_owned(),
        None => error.to_string(),
    }
}

async fn next(stream: &mut MessageStream) -> Option<zbus::Result<Message>> {
    poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await
}

/// Runs the helper on `conn` until `until` completes or the connection ends; its `pw-record`, if
/// any, is stopped then.
///
/// # Errors
///
/// When the bus refuses a subscription.
pub async fn run(
    conn: &Connection,
    config: &Config,
    log: &dyn Log,
    until: impl Future<Output = ()>,
) -> zbus::Result<()> {
    // Subscribed first, so no change is missed.
    let rule = |sender: &'static str, path: &'static str, interface: &'static str, member| {
        MatchRule::builder()
            .msg_type(Type::Signal)
            .sender(sender)
            .path(path)
            .interface(interface)
            .member(member)
    };
    let mut states = MessageStream::for_match_rule(
        rule(BUS_NAME, OBJECT_PATH, INTERFACE, "StateChanged").build()?,
        conn,
        None,
    )
    .await?;
    let mut owners = MessageStream::for_match_rule(
        rule(
            BUS_DRIVER,
            "/org/freedesktop/DBus",
            BUS_DRIVER,
            "NameOwnerChanged",
        )
        .arg(0, BUS_NAME)
        .build()?,
        conn,
        None,
    )
    .await?;
    // A look-alike only makes the helper try again, which the service still decides.
    let mut seats = MessageStream::for_match_rule(
        rule(
            LOGIND,
            SEAT,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
        )
        .build()?,
        conn,
        None,
    )
    .await?;
    let dbus = DBusProxy::new(conn).await?;
    let owner = match BusName::try_from(BUS_NAME) {
        Ok(name) => dbus.get_name_owner(name).await.ok().map(|o| o.to_string()),
        Err(_) => None,
    };
    let mut helper = Helper {
        config,
        log,
        service: OpenDeepCoolProxy::new(conn).await?,
        owner,
        watching: false,
        wanted: false,
        recorder: None,
        retry_at: Some(Instant::now()),
        wait: config.first_wait,
        doing: Doing::Starting,
    };
    let mut until = pin!(until);
    let mut timer: Option<Timer> = None;
    loop {
        helper.settle().await;
        if timer.as_ref().map(|t| t.at) != helper.retry_at {
            timer = helper.retry_at.map(Timer::at);
        }
        let event = {
            let (mut state, mut owner, mut seat) = (
                pin!(next(&mut states)),
                pin!(next(&mut owners)),
                pin!(next(&mut seats)),
            );
            poll_fn(|cx| {
                if until.as_mut().poll(cx).is_ready() {
                    return Poll::Ready(Event::Stop);
                }
                if let Poll::Ready(message) = state.as_mut().poll(cx) {
                    return Poll::Ready(match message {
                        Some(Ok(m)) => Event::State(m),
                        Some(Err(_)) => Event::Nothing,
                        None => Event::Closed,
                    });
                }
                if let Poll::Ready(message) = owner.as_mut().poll(cx) {
                    return Poll::Ready(match message {
                        Some(Ok(m)) => Event::Owner(m),
                        Some(Err(_)) => Event::Nothing,
                        None => Event::Closed,
                    });
                }
                if let Poll::Ready(message) = seat.as_mut().poll(cx) {
                    return Poll::Ready(match message {
                        Some(_) => Event::Seat,
                        None => Event::Closed,
                    });
                }
                if let Some(timer) = &timer
                    && timer.poll(cx).is_ready()
                {
                    return Poll::Ready(Event::Timer);
                }
                Poll::Pending
            })
            .await
        };
        match event {
            Event::Stop | Event::Closed => break,
            Event::State(message) => helper.state_changed(&message).await,
            Event::Owner(message) => helper.owner_changed(&message),
            Event::Seat => helper.now(),
            Event::Timer => timer = None,
            Event::Nothing => {}
        }
    }
    helper.stop_recorder();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Phase 9 plan, Task 4: the exact command line of the plan's Recommendation.
    #[test]
    fn pw_record_gets_exactly_these_arguments() {
        let props = r#"{ "stream.capture.sink": true, "node.name": "opendeepcool-audio", "node.description": "OpenDeepCool cooler display" }"#;
        let want = |extra: &[&str]| -> Vec<String> {
            let mut args = vec![
                "--raw",
                "--format",
                "f32",
                "--rate",
                "48000",
                "--channels",
                "2",
                "--latency",
                "20ms",
                "-P",
                props,
            ];
            args.extend(extra);
            args.push("-");
            args.into_iter().map(str::to_owned).collect()
        };
        assert_eq!(pw_record_args(None), want(&[]));
        assert_eq!(
            pw_record_args(Some("odc-e2e")),
            want(&["--target", "odc-e2e"])
        );
    }

    #[test]
    fn only_target_is_a_flag() {
        let args = |list: &[&str]| -> Vec<String> { list.iter().map(|&a| a.to_owned()).collect() };
        assert_eq!(parse_flags(&[]), Ok(None));
        assert_eq!(
            parse_flags(&args(&["--target", "odc-e2e"])),
            Ok(Some("odc-e2e".to_owned()))
        );
        for bad in [
            &["--target"][..],
            &["--target", ""],
            &["--target", "--dev"],
            &["--dev"],
            &["--target", "a", "--target", "b"],
            &["odc-e2e"],
            &["--target=odc-e2e"],
        ] {
            assert!(parse_flags(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
