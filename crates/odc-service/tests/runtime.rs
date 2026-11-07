//! Layer 3, the threads and the clock (Phase 7 plan, Task 6): the tick on absolute deadlines, a
//! late tick, the watchdog, a tick that never waits on a slow write or snapshot, start-up,
//! reconnect and wake-up, with the fake cooler, fake readings and a fake clock.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use odc_device::fake::{FakeConnector, Presence};
use odc_device::{Connector, Link, Lp360, OpenError, Transport};
use odc_protocol::{Frame, Message, Report, decode};
use odc_sensors::fake::{FakeClock, FakeReadings};
use odc_sensors::{Binding, Clock, Exact, Fan, Metric, Reading, Readings, Snapshot, Unavailable};
use odc_service::engine::{Apply, Choices, Engine, Published, Requests};
use odc_service::runtime::{
    Device, DeviceControl, DeviceEvent, FrameTap, Outputs, Progress, SensorLoop, SensorRequest,
    Sensors, Slot, SystemTicker, TICK, Ticker, run_device, run_engine, run_sensors,
};
use odc_service::settings::Settings;

fn snapshot(cpu: Reading) -> Snapshot {
    Snapshot::new(
        |metric| match metric {
            Metric::CpuUsage => cpu.clone(),
            other => Reading::Value(Exact::whole(10 + other as i64)),
        },
        Fan::NotChosen,
        Fan::NotChosen,
    )
}

fn usage(n: i64) -> Snapshot {
    snapshot(Reading::Value(Exact::whole(n)))
}

/// A clock that jumps to each deadline, later by a planned delay at chosen sleeps.
struct FakeTicker {
    clock: FakeClock,
    sleeps: Mutex<u64>,
    delays: HashMap<u64, Duration>,
}

impl FakeTicker {
    fn new(delays: &[(u64, Duration)]) -> Self {
        FakeTicker {
            clock: FakeClock::new(),
            sleeps: Mutex::new(0),
            delays: delays.iter().copied().collect(),
        }
    }
}

impl Ticker for FakeTicker {
    fn now(&self) -> Instant {
        self.clock.now()
    }

    fn sleep_until(&self, deadline: Instant) {
        let now = self.clock.now();
        if deadline > now {
            self.clock.advance(deadline - now);
        }
        let mut sleeps = self.sleeps.lock().expect("the counter");
        if let Some(delay) = self.delays.get(&sleeps) {
            self.clock.advance(*delay);
        }
        *sleeps += 1;
    }
}

/// What the engine's loop sent, with the clock's time.
struct Recorder<'a> {
    ticker: &'a dyn Ticker,
    frames: Vec<(u64, Instant, Frame)>,
    notified: Vec<Instant>,
    skipped: Vec<(u64, u64)>,
    requests: Vec<Requests>,
    /// Keeps both threads' progress fresh until this time, every this often.
    progress: Option<(&'a Progress, Instant, Duration)>,
    last_progress: Option<Instant>,
    /// The frames posted again for `Frame` watchers, as the service does.
    tap: Option<FrameTap>,
}

impl<'a> Recorder<'a> {
    fn new(ticker: &'a dyn Ticker) -> Self {
        Recorder {
            ticker,
            frames: Vec::new(),
            notified: Vec::new(),
            skipped: Vec::new(),
            requests: Vec::new(),
            progress: None,
            last_progress: None,
            tap: None,
        }
    }
}

impl Outputs for Recorder<'_> {
    fn frame(&mut self, tick: u64, frame: Frame) {
        self.frames.push((tick, self.ticker.now(), frame));
        if let Some(tap) = &self.tap {
            tap.post(frame);
        }
    }

    fn requests(&mut self, requests: Requests) {
        self.requests.push(requests);
    }

    fn status(&mut self, _: &Engine) {
        let now = self.ticker.now();
        if let Some((progress, until, every)) = self.progress {
            let due = self.last_progress.is_none_or(|at| now - at >= every);
            if now <= until && due {
                progress.device(now);
                progress.sensors(now);
                self.last_progress = Some(now);
            }
        }
    }

    fn notify(&mut self, message: &str) {
        assert_eq!(message, "WATCHDOG=1");
        self.notified.push(self.ticker.now());
    }

    fn skipped(&mut self, from: u64, to: u64) {
        self.skipped.push((from, to));
    }
}

fn run(ticker: &FakeTicker, ticks: u64, recorder: &mut Recorder<'_>, progress: &Progress) {
    let engine = Engine::new(Settings::default(), usage(100));
    let (apply, published) = (Slot::<Apply>::default(), Slot::<Published>::default());
    let stop = AtomicBool::new(false);
    run_engine(
        engine,
        ticker,
        &apply,
        &published,
        progress,
        recorder,
        &stop,
        Some(ticks),
    );
}

// AC3: one frame per 100 ms tick on absolute deadlines: 50 ticks, 50 frames, each at
// start + n × 100 ms.
#[test]
fn fifty_ticks_give_fifty_frames_on_the_grid() {
    let ticker = FakeTicker::new(&[]);
    let progress = Progress::new(ticker.now());
    let start = ticker.now();
    let mut recorder = Recorder::new(&ticker);
    run(&ticker, 50, &mut recorder, &progress);
    assert_eq!(recorder.frames.len(), 50);
    for (n, (tick, at, _)) in recorder.frames.iter().enumerate() {
        assert_eq!(*tick, n as u64);
        assert_eq!(*at - start, TICK * n as u32, "tick {n}");
    }
    assert!(recorder.skipped.is_empty());
}

// Phase 10 plan, Task 2: the frames for `Frame` watchers never hold up a tick. With nobody taking
// them, as with a stalled bus, 50 ticks stay on the grid, and the tap keeps only the newest frame.
#[test]
fn frames_for_watchers_never_hold_up_a_tick() {
    let ticker = FakeTicker::new(&[]);
    let progress = Progress::new(ticker.now());
    let start = ticker.now();
    let tap = FrameTap::default();
    let mut recorder = Recorder::new(&ticker);
    recorder.tap = Some(tap.clone());
    run(&ticker, 50, &mut recorder, &progress);
    assert_eq!(recorder.frames.len(), 50);
    for (n, (_, at, _)) in recorder.frames.iter().enumerate() {
        assert_eq!(*at - start, TICK * n as u32, "tick {n}");
    }
    assert_eq!(tap.pulse.count(), 50, "one pulse a frame");
    assert_eq!(tap.frames.take(), recorder.frames.last().map(|f| f.2));
    assert_eq!(tap.frames.take(), None, "only the newest was kept");
}

// A tick delayed 250 ms gives one frame for the deadlines it missed, and the deadlines after it
// stay on the grid; the strip moves one column per frame sent (S22).
#[test]
fn a_late_tick_skips_the_missed_deadlines_and_keeps_the_grid() {
    let ticker = FakeTicker::new(&[(10, Duration::from_millis(250))]);
    let progress = Progress::new(ticker.now());
    let start = ticker.now();
    let mut recorder = Recorder::new(&ticker);
    run(&ticker, 30, &mut recorder, &progress);
    let ticks: Vec<u64> = recorder.frames.iter().map(|(t, _, _)| *t).collect();
    assert!(!ticks.contains(&10) && !ticks.contains(&11), "{ticks:?}");
    assert_eq!(recorder.skipped, [(10, 11)]);
    let at = |tick: u64| {
        recorder
            .frames
            .iter()
            .find(|(t, _, _)| *t == tick)
            .map(|(_, at, _)| *at - start)
            .expect("a frame")
    };
    assert_eq!(at(12), Duration::from_millis(1250), "one frame, late");
    assert_eq!(at(13), Duration::from_millis(1300), "back on the grid");
    // The strip moved one column between the frames sent around the gap: the frame after the
    // gap is the one a gap-free run draws one tick after its tick 9.
    let mut alone = Engine::new(Settings::default(), usage(100));
    let mut drawn = Vec::new();
    for _ in 0..12 {
        drawn.push(alone.tick(None, None, false).frame.expect("a frame"));
    }
    let after_gap = &recorder
        .frames
        .iter()
        .find(|(t, _, _)| *t == 12)
        .expect("tick 12")
        .2;
    assert_eq!(after_gap, &drawn[10]);
}

// The watchdog is fed once a second while both threads progress; it stops within 10 s of the
// device thread stalling; a slow thread that still progresses keeps it fed.
#[test]
fn the_watchdog_is_fed_only_while_both_threads_progress() {
    let ticker = FakeTicker::new(&[]);
    let start = ticker.now();
    let progress = Progress::new(start);
    let mut stalled = Recorder::new(&ticker);
    stalled.progress = Some((&progress, start + Duration::from_secs(5), TICK));
    run(&ticker, 300, &mut stalled, &progress);
    let last = *stalled.notified.last().expect("fed at first");
    assert!(
        last - start >= Duration::from_secs(14),
        "fed until the window ends"
    );
    assert!(
        last - start <= Duration::from_secs(15),
        "stopped 10 s after the stall"
    );
    assert_eq!(stalled.notified.len(), 16, "once a second, 0 s to 15 s");

    let ticker = FakeTicker::new(&[]);
    let start = ticker.now();
    let progress = Progress::new(start);
    let mut slow = Recorder::new(&ticker);
    slow.progress = Some((
        &progress,
        start + Duration::from_secs(60),
        Duration::from_secs(4),
    ));
    run(&ticker, 300, &mut slow, &progress);
    assert_eq!(slow.notified.len(), 30, "fed every second for 30 s");
}

/// A cooler connection that takes its first write 5 s, like a stalled USB device, and records
/// every report written with the time.
struct Slow {
    written: Arc<Mutex<Vec<Message>>>,
    first: bool,
}

struct SlowTransport {
    written: Arc<Mutex<Vec<Message>>>,
    stall: bool,
}

impl Connector for Slow {
    type Transport = SlowTransport;

    fn connect(&mut self) -> Result<Option<SlowTransport>, OpenError> {
        let stall = std::mem::replace(&mut self.first, false);
        Ok(Some(SlowTransport {
            written: Arc::clone(&self.written),
            stall,
        }))
    }
}

impl Transport for SlowTransport {
    fn write_report(&mut self, report: &Report) -> io::Result<()> {
        if std::mem::replace(&mut self.stall, false) {
            thread::sleep(Duration::from_secs(5));
        }
        let message = decode(report.as_bytes()).expect("a documented message");
        self.written.lock().expect("the log").push(message);
        Ok(())
    }
}

/// Sensors whose every snapshot takes 2 s, like a slow driver.
struct SlowSensors;

impl Sensors for SlowSensors {
    fn snapshot(&mut self) -> Snapshot {
        thread::sleep(Duration::from_secs(2));
        usage(100)
    }
    fn restart_intervals(&mut self) {}
    fn choose(&mut self, _: &Choices) {}
    fn bindings(&self) -> Vec<(Metric, Binding)> {
        Vec::new()
    }
}

/// The engine's frames go to the device thread's mailbox; the requests to the sensor thread.
struct ToThreads<'a> {
    frames: &'a Slot<(u64, Frame)>,
    times: Vec<(u64, Instant)>,
}

impl Outputs for ToThreads<'_> {
    fn frame(&mut self, tick: u64, frame: Frame) {
        self.times.push((tick, Instant::now()));
        self.frames.post((tick, frame));
    }
    fn requests(&mut self, _: Requests) {}
    fn status(&mut self, _: &Engine) {}
    fn notify(&mut self, _: &str) {}
    fn skipped(&mut self, from: u64, to: u64) {
        panic!("ticks {from}–{to} skipped: the tick waited");
    }
}

// OV-03: neither a 5 s write nor 2 s snapshots move a tick off its grid; frames posted during the
// slow write replace each other, and the device thread then writes the newest one.
#[test]
fn the_tick_never_waits_for_a_slow_write_or_snapshot() {
    let frames = Arc::new(Slot::<(u64, Frame)>::default());
    let published = Arc::new(Slot::<Published>::default());
    let stop = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(Progress::new(Instant::now()));
    let written = Arc::new(Mutex::new(Vec::new()));
    let ticks_written = Arc::new(Mutex::new(Vec::new()));
    let (_control_tx, control) = mpsc::channel::<DeviceControl>();
    let device = {
        let (frames, stop, progress, written, ticks) = (
            Arc::clone(&frames),
            Arc::clone(&stop),
            Arc::clone(&progress),
            Arc::clone(&written),
            Arc::clone(&ticks_written),
        );
        thread::spawn(move || {
            let mut device = Device::new(Lp360::new(Slow {
                written,
                first: true,
            }));
            let mut on_event = |event: DeviceEvent<'_>| {
                if let DeviceEvent::Written { tick, .. } = event {
                    ticks.lock().expect("ticks").push(tick);
                }
            };
            run_device(
                &mut device,
                &frames,
                &control,
                &progress,
                &mut on_event,
                &stop,
            );
        })
    };
    let (_requests_tx, requests) = mpsc::channel::<SensorRequest>();
    let sensors = {
        let (published, stop, progress) = (
            Arc::clone(&published),
            Arc::clone(&stop),
            Arc::clone(&progress),
        );
        thread::spawn(move || {
            let mut work = SensorLoop::new(SlowSensors);
            run_sensors(
                &mut work,
                &requests,
                &published,
                &progress,
                &mut |_, _| {},
                &stop,
            );
        })
    };
    let apply = Slot::<Apply>::default();
    let mut outputs = ToThreads {
        frames: &frames,
        times: Vec::new(),
    };
    let start = Instant::now();
    let engine = Engine::new(Settings::default(), usage(100));
    run_engine(
        engine,
        &SystemTicker,
        &apply,
        &published,
        &progress,
        &mut outputs,
        &AtomicBool::new(false),
        Some(60),
    );
    stop.store(true, Ordering::Relaxed);
    device.join().expect("the device thread");
    sensors.join().expect("the sensor thread");
    for (tick, at) in &outputs.times {
        let late = (*at - start).saturating_sub(TICK * *tick as u32);
        assert!(
            late < Duration::from_millis(50),
            "tick {tick}: {late:?} late"
        );
    }
    let ticks = ticks_written.lock().expect("ticks").clone();
    assert_eq!(ticks[0], 0, "the first frame, after the 5 s write");
    assert!(ticks[1] >= 45, "then the newest, not tick 1: {ticks:?}");
    let written = written.lock().expect("the log");
    assert_eq!(written[0], Message::Startup);
}

/// The frames a fake cooler's connections received, without the start-up command, and whether
/// each connection began with it.
fn connections(log: &odc_device::fake::FakeLog) -> Vec<(bool, Vec<Frame>)> {
    log.connections()
        .into_iter()
        .map(|messages| {
            let starts = messages.first() == Some(&Message::Startup);
            let frames = messages
                .into_iter()
                .filter_map(|m| match m {
                    Message::Frame(f) => Some(f),
                    Message::Startup => None,
                })
                .collect();
            (starts, frames)
        })
        .collect()
}

// Start-up (AC4): the cooler gets 03 08, then the saved mode's picture in the first frame.
#[test]
fn start_up_sends_03_08_then_the_saved_mode() {
    let connector = FakeConnector::new([Presence::Present]);
    let log = connector.log();
    let mut device = Device::new(Lp360::new(connector));
    let saved = Settings {
        mode: 3,
        rotation: 1,
        ..Settings::default()
    };
    let mut engine = Engine::new(saved.clone(), usage(42));
    let first = engine.tick(None, None, false).frame.expect("a frame");
    assert_eq!(device.write(0, &first).expect("written").0, Link::Connected);
    let got = connections(&log);
    assert_eq!(got.len(), 1);
    assert!(got[0].0, "03 08 first");
    assert_eq!(got[0].1, [first]);
}

/// Every frame of an engine showing `100%`, tick by tick.
fn scrolling(ticks: usize) -> Vec<Frame> {
    let mut engine = Engine::new(Settings::default(), usage(100));
    (0..ticks)
        .map(|_| engine.tick(None, None, false).frame.expect("a frame"))
        .collect()
}

// S18: the cooler vanishes for 5 ticks while 100% scrolls, then returns: the schedule loses no
// tick, the strip's position follows the ticks, and the returning connection starts with 03 08.
#[test]
fn a_reconnect_starts_with_03_08_and_the_strip_follows_the_ticks() {
    let connector = FakeConnector::new([
        Presence::FailsAfter(6),
        Presence::Absent,
        Presence::Absent,
        Presence::Absent,
        Presence::Absent,
        Presence::Present,
    ]);
    let log = connector.log();
    let mut device = Device::new(Lp360::new(connector));
    let frames = scrolling(30);
    let links: Vec<Link> = (0..)
        .zip(&frames)
        .map(|(tick, f)| device.write(tick, f).expect("written").0)
        .collect();
    // 03 08 and 5 frames, a failed write, 4 ticks away, then back.
    assert!(links[..5].iter().all(|l| *l == Link::Connected));
    assert!(
        links[5..10].iter().all(|l| *l != Link::Connected),
        "{links:?}"
    );
    assert_eq!(links[10], Link::Connected);
    let got = connections(&log);
    let back = got.last().expect("a connection");
    assert!(back.0, "the returning connection starts with 03 08");
    assert_eq!(back.1[0], frames[10], "the position followed the ticks");
    assert_eq!(back.1[..], frames[10..]);
}

/// Fake sensors over `FakeReadings`, which count the restarts.
struct FakeSensors(Arc<Mutex<FakeReadings>>);

impl Sensors for FakeSensors {
    fn snapshot(&mut self) -> Snapshot {
        self.0.lock().expect("the readings").snapshot()
    }
    fn restart_intervals(&mut self) {
        self.0.lock().expect("the readings").restart_intervals();
    }
    fn choose(&mut self, _: &Choices) {}
    fn bindings(&self) -> Vec<(Metric, Binding)> {
        Vec::new()
    }
}

// S18, a wake-up with the device present all along (owner, 2026-10-26): the next report is 03 08,
// then that tick's frame; the strip's position continues; the readings start their intervals
// over, so a rate metric shows dashes for its first second, then its value.
#[test]
fn a_wake_up_starts_a_new_conversation_and_new_intervals() {
    let connector = FakeConnector::new([Presence::Present]);
    let log = connector.log();
    let mut device = Device::new(Lp360::new(connector));
    let readings = Arc::new(Mutex::new(FakeReadings::new([
        snapshot(Reading::Unavailable(Unavailable::NotYet)),
        usage(100),
    ])));
    let mut sensors = SensorLoop::new(FakeSensors(Arc::clone(&readings)));
    let mut engine = Engine::new(Settings::default(), usage(100));
    let mut alone = Engine::new(Settings::default(), usage(100));
    for tick in 0..7 {
        let f = engine.tick(None, None, false).frame.expect("a frame");
        alone.tick(None, None, false);
        device.write(tick, &f);
    }
    // The wake-up: the D-Bus side tells the device and sensor threads, and the engine.
    device.wake();
    let woke = engine.tick(
        Some(Apply {
            wake: true,
            ..Apply::default()
        }),
        None,
        false,
    );
    for request in SensorRequest::of(woke.requests) {
        sensors.handle(request);
    }
    assert_eq!(readings.lock().expect("the readings").restarts(), 1);
    let frame = woke.frame.expect("a frame");
    assert_eq!(
        frame,
        alone.tick(None, None, false).frame.expect("a frame"),
        "it continues"
    );
    device.write(7, &frame);
    let got = connections(&log);
    assert_eq!(got.len(), 2, "a new conversation");
    assert!(
        got[1].0 && got[1].1 == [frame],
        "03 08, then that tick's frame"
    );
    // The first second after the wake-up: dashes where the digits were, the strip still moving
    // (S11: the movement rules don't change); then the value again.
    let not_yet = sensors.snapshot();
    let mut values = Engine::new(Settings::default(), usage(100));
    for _ in 0..8 {
        values.tick(None, None, false);
    }
    let first = engine
        .tick(None, Some(not_yet.clone()), false)
        .frame
        .expect("a frame");
    let twin = alone
        .tick(None, Some(not_yet), false)
        .frame
        .expect("a frame");
    assert_eq!(first, twin);
    assert_ne!(
        first,
        values.tick(None, None, false).frame.expect("a frame"),
        "dashes show"
    );
    let back = sensors.snapshot();
    let value = engine.tick(None, Some(back.clone()), false).frame;
    assert_eq!(value, alone.tick(None, Some(back), false).frame);
    assert_eq!(
        value,
        values.tick(None, None, false).frame,
        "digits again, in step"
    );
}

// A wake-up while the cooler is away changes nothing but the readings.
#[test]
fn a_wake_up_without_a_cooler_changes_nothing_on_the_device() {
    let connector = FakeConnector::new([Presence::Absent]);
    let log = connector.log();
    let mut device = Device::new(Lp360::new(connector));
    let frames = scrolling(3);
    device.write(0, &frames[0]);
    device.wake();
    let (link, changed) = device.write(1, &frames[1]).expect("tried");
    assert!(matches!(link, Link::Disconnected(_)));
    assert!(!changed, "still away, no new state");
    assert!(log.connections().is_empty());
}

/// Records the engine's frames by tick.
struct Frames(Vec<(u64, Frame)>);

impl Outputs for Frames {
    fn frame(&mut self, tick: u64, frame: Frame) {
        self.0.push((tick, frame));
    }
    fn requests(&mut self, _: Requests) {}
    fn status(&mut self, _: &Engine) {}
    fn notify(&mut self, _: &str) {}
    fn skipped(&mut self, _: u64, _: u64) {}
}

// A changed reading shows within 10 ticks: the sensor thread reads once a second, and the engine
// adopts the newest snapshot at its next tick.
#[test]
fn a_changed_reading_shows_within_ten_ticks() {
    let published = Arc::new(Slot::<Published>::default());
    let stop = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(Progress::new(Instant::now()));
    let (_requests_tx, requests) = mpsc::channel::<SensorRequest>();
    let sensors = {
        let (published, stop, progress) = (
            Arc::clone(&published),
            Arc::clone(&stop),
            Arc::clone(&progress),
        );
        thread::spawn(move || {
            let readings = Arc::new(Mutex::new(FakeReadings::new([usage(57)])));
            let mut work = SensorLoop::new(FakeSensors(readings));
            run_sensors(
                &mut work,
                &requests,
                &published,
                &progress,
                &mut |_, _| {},
                &stop,
            );
        })
    };
    let mut frames = Frames(Vec::new());
    run_engine(
        Engine::new(Settings::default(), usage(42)),
        &SystemTicker,
        &Slot::<Apply>::default(),
        &published,
        &progress,
        &mut frames,
        &AtomicBool::new(false),
        Some(15),
    );
    stop.store(true, Ordering::Relaxed);
    sensors.join().expect("the sensor thread");
    let want = Engine::new(Settings::default(), usage(57))
        .tick(None, None, false)
        .frame
        .expect("a frame");
    let first = frames
        .0
        .iter()
        .find(|(_, f)| *f == want)
        .map(|(t, _)| *t)
        .expect("57% shows");
    assert!(first <= 11, "57% first at tick {first}");
}
