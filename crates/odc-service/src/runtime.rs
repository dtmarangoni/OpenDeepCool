//! The threads around the engine (Phase 7 plan, Task 6; Recommendation, one thread per job). The
//! engine thread runs the 100 ms tick on absolute deadlines and never waits for I/O; the device
//! thread writes the newest frame to the cooler; the sensor thread reads the sensors once a
//! second and on request. They meet only through one-slot mailboxes, so a stalled USB write or a
//! slow driver delays only its own thread, and nothing queues up.

use std::io;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use odc_device::{Connector, DisplayPort, Link, Lp360};
use odc_protocol::Frame;
use odc_sensors::{Binding, Clock, FanInput, Gpu, Metric, PciAddr, Readings, Registry, Snapshot};

use crate::engine::{Apply, Choices, Engine, Published, Requests};
use crate::settings::SavedInput;

/// One frame per 100 ms tick (`docs/spec/protocol.md`, Timing).
pub const TICK: Duration = Duration::from_millis(100);

/// The sensors are read once a second (`docs/spec/sensors.md`).
pub const SNAPSHOT_EVERY: Duration = Duration::from_secs(1);

/// The watchdog is fed only while the device and sensor threads both progressed this recently
/// (Recommendation, Supervision); the unit's `WatchdogSec=15` then restarts a stuck service.
pub const PROGRESS_WINDOW: Duration = Duration::from_secs(10);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A one-slot mailbox: a post replaces what's there, so the reader only ever takes the newest.
pub struct Slot<T> {
    value: Mutex<Option<T>>,
    posted: Condvar,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot {
            value: Mutex::new(None),
            posted: Condvar::new(),
        }
    }
}

impl<T> Slot<T> {
    pub fn post(&self, value: T) {
        *lock(&self.value) = Some(value);
        self.posted.notify_all();
    }

    /// Posts `value`, merged with what's there and not yet taken.
    pub fn merge(&self, value: T, merge: impl FnOnce(T, T) -> T) {
        let mut slot = lock(&self.value);
        *slot = Some(match slot.take() {
            Some(older) => merge(older, value),
            None => value,
        });
        self.posted.notify_all();
    }

    pub fn take(&self) -> Option<T> {
        lock(&self.value).take()
    }

    /// Takes the value once one is posted, waiting at most `timeout`.
    pub fn wait(&self, timeout: Duration) -> Option<T> {
        let slot = lock(&self.value);
        let (mut slot, _) = self
            .posted
            .wait_timeout_while(slot, timeout, |value| value.is_none())
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        slot.take()
    }
}

/// When the device and sensor threads last made progress, on the monotonic clock: a finished
/// write attempt or wait, a finished snapshot.
pub struct Progress {
    device: Mutex<Instant>,
    sensors: Mutex<Instant>,
}

impl Progress {
    pub fn new(now: Instant) -> Progress {
        Progress {
            device: Mutex::new(now),
            sensors: Mutex::new(now),
        }
    }

    pub fn device(&self, now: Instant) {
        *lock(&self.device) = now;
    }

    pub fn sensors(&self, now: Instant) {
        *lock(&self.sensors) = now;
    }

    /// Whether both threads progressed within the window before `now`.
    pub fn healthy(&self, now: Instant) -> bool {
        let fresh = |at: Instant| now.saturating_duration_since(at) <= PROGRESS_WINDOW;
        fresh(*lock(&self.device)) && fresh(*lock(&self.sensors))
    }
}

/// The clock the engine's loop runs on: the system's monotonic clock, or a fake for the tests.
pub trait Ticker {
    fn now(&self) -> Instant;
    /// Returns at `deadline` or later.
    fn sleep_until(&self, deadline: Instant);
}

/// The system's monotonic clock (`Instant`, `CLOCK_MONOTONIC` on Linux), which a suspend doesn't
/// advance.
pub struct SystemTicker;

impl Ticker for SystemTicker {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep_until(&self, deadline: Instant) {
        let now = Instant::now();
        if deadline > now {
            std::thread::sleep(deadline - now);
        }
    }
}

/// What the engine's loop sends out besides frames.
pub trait Outputs {
    /// The tick's frame, turned, for the device thread.
    fn frame(&mut self, tick: u64, frame: Frame);
    /// What the engine asks of the sensor thread.
    fn requests(&mut self, requests: Requests);
    /// The engine after its tick, for the status.
    fn status(&mut self, engine: &Engine);
    /// A line to systemd, `WATCHDOG=1`.
    fn notify(&mut self, message: &str);
    /// A deadline was missed by more than one tick; the frames between were skipped.
    fn skipped(&mut self, from: u64, to: u64);
}

/// The engine's loop: tick `n` is due at `start + n × 100 ms`. A late tick doesn't shift the ones
/// after it; deadlines missed by more than one tick are skipped, never bunched up (S22). Once a
/// second it feeds the watchdog while both other threads progress. It returns when `stop` is
/// set, or after `limit` ticks.
#[allow(clippy::too_many_arguments)]
pub fn run_engine(
    mut engine: Engine,
    ticker: &dyn Ticker,
    apply: &Slot<Apply>,
    published: &Slot<Published>,
    progress: &Progress,
    outputs: &mut dyn Outputs,
    stop: &AtomicBool,
    limit: Option<u64>,
) -> Engine {
    let start = ticker.now();
    let mut next: u64 = 0;
    while !stop.load(Ordering::Relaxed) && limit.is_none_or(|limit| next < limit) {
        let deadline = start + TICK * u32::try_from(next).expect("ticks within u32");
        ticker.sleep_until(deadline);
        let now = ticker.now();
        let due = u64::try_from(now.saturating_duration_since(start).as_nanos() / TICK.as_nanos())
            .expect("ticks within u64");
        if due > next {
            outputs.skipped(next, due - 1);
            while next < due {
                engine.tick(None, None, true);
                next += 1;
            }
        }
        let out = engine.tick(apply.take(), published.take(), false);
        if let Some(frame) = out.frame {
            outputs.frame(out.number, frame);
        }
        if out.requests != Requests::default() {
            outputs.requests(out.requests);
        }
        outputs.status(&engine);
        if out.number.is_multiple_of(10) && progress.healthy(now) {
            outputs.notify("WATCHDOG=1");
        }
        next += 1;
    }
    engine
}

/// The device thread's work: it owns the cooler and writes each frame it's given.
pub struct Device<C: Connector> {
    lp360: Lp360<C>,
}

impl<C: Connector> Device<C> {
    pub fn new(lp360: Lp360<C>) -> Self {
        Device { lp360 }
    }

    /// Writes `frame`; returns the link, and whether it changed since the last write.
    pub fn write(&mut self, frame: &Frame) -> (Link, bool) {
        let link = self.lp360.show(frame);
        (link, self.lp360.state_changed())
    }

    /// After a wake-up from standby: a new conversation, `03 08` first, even when the device
    /// node survived (owner, 2026-10-26).
    pub fn wake(&mut self) {
        self.lp360.reset_connection();
    }

    pub fn lp360(&self) -> &Lp360<C> {
        &self.lp360
    }
}

/// What the device thread is told besides frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceControl {
    Wake,
}

/// The device thread: the newest frame from `frames`, written as it comes; a frame posted during
/// a slow write replaces the unsent one. `on_write` gets each written frame's tick, the link, and
/// whether the link changed.
pub fn run_device<C: Connector>(
    device: &mut Device<C>,
    frames: &Slot<(u64, Frame)>,
    control: &Receiver<DeviceControl>,
    progress: &Progress,
    on_write: &mut dyn FnMut(u64, &Link, bool),
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Relaxed) {
        while let Ok(DeviceControl::Wake) = control.try_recv() {
            device.wake();
        }
        if let Some((tick, frame)) = frames.wait(TICK) {
            let (link, changed) = device.write(&frame);
            on_write(tick, &link, changed);
        }
        progress.device(Instant::now());
    }
}

/// The sensors the sensor thread reads: the registry, or a fake.
pub trait Sensors: Send {
    fn snapshot(&mut self) -> Snapshot;
    fn restart_intervals(&mut self);
    /// The GPU and fan inputs to read from now on.
    fn choose(&mut self, choices: &Choices);
    fn bindings(&self) -> Vec<(Metric, Binding)>;
}

/// The registry over the built-in rules, with the GPUs found at start.
pub struct RegistrySensors<C: Clock> {
    pub registry: Registry<C>,
    pub gpus: Vec<Gpu>,
}

fn fan_input(saved: &Option<SavedInput>) -> Option<FanInput> {
    saved.as_ref().map(|s| FanInput {
        chip: s.chip.clone(),
        device: PathBuf::from(&s.device_path),
        input: s.input,
        label: None,
    })
}

impl<C: Clock + Send> Sensors for RegistrySensors<C> {
    fn snapshot(&mut self) -> Snapshot {
        self.registry.snapshot()
    }

    fn restart_intervals(&mut self) {
        self.registry.restart_intervals();
    }

    fn choose(&mut self, choices: &Choices) {
        let saved = choices
            .gpu
            .as_deref()
            .and_then(|pci| PciAddr::from_str(pci).ok());
        self.registry
            .choose_gpu(odc_sensors::choose(&self.gpus, saved));
        self.registry.choose_fans(
            fan_input(&choices.cooler_fan_input),
            fan_input(&choices.pump_input),
        );
    }

    fn bindings(&self) -> Vec<(Metric, Binding)> {
        self.registry.bindings()
    }
}

/// What the sensor thread is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SensorRequest {
    /// A snapshot at once, answering this request.
    Snapshot(u64),
    Choices(Choices),
    /// After a wake-up: start every reading averaged over the interval again.
    Restart,
}

impl SensorRequest {
    /// The engine's requests, one message each.
    pub fn of(requests: Requests) -> Vec<SensorRequest> {
        let mut out = Vec::new();
        if requests.restart {
            out.push(SensorRequest::Restart);
        }
        if let Some(choices) = requests.choices {
            out.push(SensorRequest::Choices(choices));
        }
        if let Some(id) = requests.snapshot {
            out.push(SensorRequest::Snapshot(id));
        }
        out
    }
}

/// The sensor thread's work.
pub struct SensorLoop<S: Sensors> {
    pub sensors: S,
    answers: u64,
}

impl<S: Sensors> SensorLoop<S> {
    pub fn new(sensors: S) -> Self {
        SensorLoop {
            sensors,
            answers: 0,
        }
    }

    /// Takes one request; `true` when it asks for a snapshot at once.
    pub fn handle(&mut self, request: SensorRequest) -> bool {
        match request {
            SensorRequest::Snapshot(id) => {
                self.answers = self.answers.max(id);
                true
            }
            SensorRequest::Choices(choices) => {
                self.sensors.choose(&choices);
                false
            }
            SensorRequest::Restart => {
                self.sensors.restart_intervals();
                false
            }
        }
    }

    pub fn snapshot(&mut self) -> Published {
        Published {
            snapshot: self.sensors.snapshot(),
            answers: self.answers,
        }
    }
}

/// The sensor thread: a snapshot once a second on its own monotonic timer, and at once when
/// asked; each one published, the newest replacing an older one the engine hasn't taken.
pub fn run_sensors<S: Sensors>(
    work: &mut SensorLoop<S>,
    requests: &Receiver<SensorRequest>,
    published: &Slot<Published>,
    progress: &Progress,
    on_bindings: &mut dyn FnMut(Vec<(Metric, Binding)>),
    stop: &AtomicBool,
) {
    let mut due = Instant::now() + SNAPSHOT_EVERY;
    while !stop.load(Ordering::Relaxed) {
        let mut now_please = false;
        match requests.recv_timeout(due.saturating_duration_since(Instant::now())) {
            Ok(request) => {
                now_please |= work.handle(request);
                while let Ok(request) = requests.try_recv() {
                    now_please |= work.handle(request);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = Instant::now();
        if now_please || now >= due {
            published.post(work.snapshot());
            on_bindings(work.sensors.bindings());
            progress.sensors(Instant::now());
            due = Instant::now() + SNAPSHOT_EVERY;
        }
    }
}

/// Sends `message` to systemd over the `sd_notify` protocol: one datagram to the socket named by
/// `$NOTIFY_SOCKET`, which systemd sets for a `Type=notify` service; nothing without it. A name
/// starting with `@` is in the abstract namespace (sd_notify(3)).
pub fn sd_notify(message: &str) -> io::Result<()> {
    let Some(socket) = std::env::var_os("NOTIFY_SOCKET") else {
        return Ok(());
    };
    let socket = socket.to_string_lossy().into_owned();
    let address = match socket.strip_prefix('@') {
        Some(name) => SocketAddr::from_abstract_name(name.as_bytes())?,
        None => SocketAddr::from_pathname(&socket)?,
    };
    let sender = UnixDatagram::unbound()?;
    sender.send_to_addr(message.as_bytes(), &address)?;
    Ok(())
}

/// Makes any panic end the whole service, so systemd restarts it, instead of leaving a live
/// process with a dead tick (owner, 2026-10-26; solution review OV-03): the message goes to the
/// journal at priority 2 (critical, `sd-daemon(3)`), then the process aborts. The release profile
/// sets `panic = "abort"` too.
pub fn abort_on_panic() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("<2>{info}");
        std::process::abort();
    }));
}

/// Forwards the engine's requests to the sensor thread.
pub fn forward(requests: Requests, to: &Sender<SensorRequest>) {
    for request in SensorRequest::of(requests) {
        // The sensor thread only ends with the service.
        let _ = to.send(request);
    }
}
