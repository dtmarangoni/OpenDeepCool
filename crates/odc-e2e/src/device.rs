//! The device checks (Phase 5 plan, Task 9): `test_images`, `reconnect` and `reset`, each on the
//! real LP360 where one is usable and through fakes where none is attached. Every report written
//! is recorded and compared with the expected frames; nobody looks at the cooler.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use odc_device::fake::{FakeConnector, Presence};
use odc_device::{
    Connector, DisplayPort, HidrawConnector, Link, Lp360, OpenError, Recording, Transport,
};
use odc_protocol::{Frame, Message, Report, Rotation, decode};
use odc_testkit::capture;
use odc_testkit::diff;
use odc_testkit::expected::{Kind, Picture, load_mode};
use odc_testkit::fake_tree::{FakeTree, LP360_HID_ID};
use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};
use odc_testkit::sysfs::{LP360_PRODUCT, LP360_VENDOR, usb_device};

/// Before any probe (owner, 2026-10-27; solution review P5-09): a run killed during the reconnect
/// check skips its guard, and the cooler would stay switched off and probe as absent. Switch it
/// on again, with a note.
pub fn restore_switch(roots: &Roots) -> Result<Option<String>, String> {
    let Some(usb) = usb_device(&roots.sysfs, LP360_VENDOR, LP360_PRODUCT) else {
        return Ok(None);
    };
    let switch = usb.join("authorized");
    let state = fs::read_to_string(&switch).map_err(|e| format!("{}: {e}", switch.display()))?;
    if state.trim() != "0" {
        return Ok(None);
    }
    fs::write(&switch, "1").map_err(|e| format!("{}: {e}", switch.display()))?;
    Ok(Some(format!(
        "{}: the LP360 was switched off, perhaps by a run that was killed; switched it on",
        switch.display()
    )))
}

pub fn run(roots: &Roots, out_dir: &Path, tick: Duration, summary: &mut Summary) {
    let result = test_images(roots, out_dir, tick, summary);
    summary.check("test_images", result);
    let result = reconnect(roots, out_dir, tick, summary);
    summary.check("reconnect", result);
    let result = reset(roots, out_dir, tick, summary);
    summary.check("reset", result);
}

/// Mode 1's `42%`, upright and in rotation 1, from `docs/spec/frames/mode-001.txt`.
fn expected_42() -> Result<(Picture, Picture), String> {
    let mode = load_mode(1);
    let picture = |rotation: u8| {
        mode.cases
            .iter()
            .find(|c| {
                c.rotation.setting() == rotation && c.readings == [("CPU Usage".to_owned(), 42)]
            })
            .and_then(|c| match &c.kind {
                Kind::Static(p) => Some(p.clone()),
                _ => None,
            })
            .ok_or_else(|| format!("mode-001.txt has no static 42% case in rotation {rotation}"))
    };
    Ok((picture(0)?, picture(1)?))
}

fn upright_42() -> Result<(Frame, Picture), String> {
    let (picture, _) = expected_42()?;
    let frame = picture
        .to_frame()
        .ok_or("mode 1's 42% has spectrum pixels")?;
    Ok((frame, picture))
}

/// `<out_dir>/<name>-<unix ns>.log`.
fn recording_path(out_dir: &Path, name: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let unix_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    Ok(out_dir.join(format!("{name}-{unix_ns}.log")))
}

fn read_recording(path: &Path) -> Result<Vec<Message>, String> {
    capture::read(path)
        .map(|c| {
            let bytes: [u8; 64] = c
                .bytes
                .as_slice()
                .try_into()
                .map_err(|_| format!("{}:{}: not 64 bytes", path.display(), c.line))?;
            decode(&bytes).map_err(|e| format!("{}:{}: {e}", path.display(), c.line))
        })
        .collect()
}

/// A fixed schedule: the nth wait returns at start + n × tick.
struct Pace {
    start: Instant,
    tick: Duration,
    n: u32,
}

impl Pace {
    fn new(tick: Duration) -> Self {
        Pace {
            start: Instant::now(),
            tick,
            n: 0,
        }
    }

    fn wait(&mut self) {
        let due = self.start + self.tick * self.n;
        let now = Instant::now();
        if due > now {
            thread::sleep(due - now);
        }
        self.n += 1;
    }
}

/// Wraps a connector to time each write; Phase 7 sizes its device thread and watchdog from them.
struct Timed<C> {
    inner: C,
    durations: Arc<Mutex<Vec<Duration>>>,
}

struct TimedTransport<T> {
    inner: T,
    durations: Arc<Mutex<Vec<Duration>>>,
}

impl<C: Connector> Connector for Timed<C> {
    type Transport = TimedTransport<C::Transport>;

    fn connect(&mut self) -> Result<Option<Self::Transport>, OpenError> {
        Ok(self.inner.connect()?.map(|inner| TimedTransport {
            inner,
            durations: Arc::clone(&self.durations),
        }))
    }
}

impl<T: Transport> Transport for TimedTransport<T> {
    fn write_report(&mut self, report: &Report) -> std::io::Result<()> {
        let start = Instant::now();
        let result = self.inner.write_report(report);
        self.durations
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(start.elapsed());
        result
    }
}

fn millis(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// p50, p99 and the maximum of the write durations.
fn write_times(durations: &[Duration]) -> String {
    let mut sorted = durations.to_vec();
    sorted.sort();
    if sorted.is_empty() {
        return "no writes".to_owned();
    }
    let at = |q: usize| sorted[(sorted.len() * q / 100).min(sorted.len() - 1)];
    format!(
        "writes p50 {:.2} ms, p99 {:.2} ms, max {:.2} ms",
        millis(at(50)),
        millis(at(99)),
        millis(sorted[sorted.len() - 1])
    )
}

/// A fake tree with an LP360 stand-in, for a PC with no cooler attached.
fn fake_lp360(out_dir: &Path, name: &str) -> Result<FakeTree, String> {
    let tree = FakeTree::create(out_dir.join(name)).map_err(|e| format!("a fake tree: {e}"))?;
    tree.hidraw(0, LP360_HID_ID)
        .map_err(|e| format!("a fake node: {e}"))?;
    Ok(tree)
}

fn hidraw(roots: &Roots) -> HidrawConnector {
    HidrawConnector::new(&roots.sysfs, &roots.dev)
}

/// One example of the rotation test type on the cooler (owner, 2026-10-26): 20 frames of `42%`
/// upright, then 20 turned 90°, at a fixed interval; every frame must reach the cooler, and the
/// recording must hold one start-up and exactly those 40 frames.
fn test_images(
    roots: &Roots,
    out_dir: &Path,
    tick: Duration,
    summary: &mut Summary,
) -> Result<String, String> {
    match probe(Capability::Lp360, roots) {
        Probe::Real => {
            summary.mode(Capability::Lp360, Mode::Real);
            images_on(hidraw(roots), out_dir, tick)
        }
        Probe::Absent { reason } => {
            summary.mode(Capability::Lp360, Mode::Fake { reason });
            let tree = fake_lp360(out_dir, "fake-images")?;
            images_on(hidraw(&tree.roots()), out_dir, tick)
        }
        Probe::Unusable { reason, fix } => {
            summary.mode(
                Capability::Lp360,
                Mode::Failed {
                    reason: reason.clone(),
                },
            );
            Err(format!("{reason}; fix: {fix}"))
        }
    }
}

fn images_on<C: Connector>(connector: C, out_dir: &Path, tick: Duration) -> Result<String, String> {
    let (upright_picture, turned_picture) = expected_42()?;
    let upright = upright_picture
        .to_frame()
        .ok_or("mode 1's 42% has spectrum pixels")?;
    let turned = upright.rotated(Rotation::from_setting(1).expect("setting 1"));
    let durations = Arc::new(Mutex::new(Vec::new()));
    let path = recording_path(out_dir, "hardware")?;
    let timed = Timed {
        inner: connector,
        durations: Arc::clone(&durations),
    };
    let recording =
        Recording::create(timed, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lp360 = Lp360::new(recording);
    let mut pace = Pace::new(tick);
    for i in 0..40 {
        let frame = if i < 20 { &upright } else { &turned };
        pace.wait();
        let link = lp360.show(frame);
        if link != Link::Connected {
            return Err(format!("frame {}: {link:?}", i + 1));
        }
    }
    if let Some(failure) = lp360.connector().failure() {
        return Err(failure);
    }
    let messages = read_recording(&path)?;
    let startups = messages.iter().filter(|m| **m == Message::Startup).count();
    if messages.first() != Some(&Message::Startup) || startups != 1 || messages.len() != 41 {
        return Err(format!(
            "{}: {} messages, {startups} start-ups; expected 03 08 then 40 frames",
            path.display(),
            messages.len()
        ));
    }
    for (i, message) in messages[1..].iter().enumerate() {
        let want = if i < 20 {
            &upright_picture
        } else {
            &turned_picture
        };
        let Message::Frame(frame) = message else {
            return Err(format!("message {} isn't a frame", i + 2));
        };
        if let Some(report) = diff::describe(want, frame) {
            return Err(format!("frame {}: {report}", i + 1));
        }
    }
    let times = write_times(&durations.lock().unwrap_or_else(|p| p.into_inner()));
    Ok(format!("03 08, then 40 frames as expected; {times}"))
}

/// Switches the cooler on again on every way out of the reconnect check, a panic included.
struct SwitchOn(PathBuf);

impl Drop for SwitchOn {
    fn drop(&mut self) {
        if let Err(e) = fs::write(&self.0, "1") {
            eprintln!(
                "odc-e2e: {}: couldn't switch the LP360 on again: {e}",
                self.0.display()
            );
        }
    }
}

/// One example of the reconnect test type: the cooler switched off and on again through its USB
/// `authorized` switch, and the conversation started anew with `03 08`.
fn reconnect(
    roots: &Roots,
    out_dir: &Path,
    tick: Duration,
    summary: &mut Summary,
) -> Result<String, String> {
    let node = probe(Capability::Lp360, roots);
    let switch = probe(Capability::Lp360Reconnect, roots);
    match (node, switch) {
        (Probe::Real, Probe::Real) => {
            summary.mode(Capability::Lp360Reconnect, Mode::Real);
            reconnect_real(roots, out_dir, tick)
        }
        (Probe::Unusable { reason, fix }, _) | (_, Probe::Unusable { reason, fix }) => {
            summary.mode(
                Capability::Lp360Reconnect,
                Mode::Failed {
                    reason: reason.clone(),
                },
            );
            Err(format!("{reason}; fix: {fix}"))
        }
        (Probe::Absent { reason }, _) | (_, Probe::Absent { reason }) => {
            summary.mode(Capability::Lp360Reconnect, Mode::Fake { reason });
            reconnect_fake(out_dir, tick)
        }
    }
}

fn reconnect_real(roots: &Roots, out_dir: &Path, tick: Duration) -> Result<String, String> {
    let usb = usb_device(&roots.sysfs, LP360_VENDOR, LP360_PRODUCT)
        .ok_or("the LP360's USB device is gone")?;
    let switch = usb.join("authorized");
    let _switch_on = SwitchOn(switch.clone());
    let path = recording_path(out_dir, "reconnect")?;
    let recording =
        Recording::create(hidraw(roots), &path).map_err(|e| format!("{}: {e}", path.display()))?;
    let set =
        |state: &str| fs::write(&switch, state).map_err(|e| format!("{}: {e}", switch.display()));
    let still_listed = || {
        let listed = usb_device(&roots.sysfs, LP360_VENDOR, LP360_PRODUCT);
        if listed.as_deref() == Some(usb.as_path()) {
            Ok(())
        } else {
            Err("switched off, the USB device isn't listed with its IDs".to_owned())
        }
    };
    let (mut lp360, note) =
        reconnect_sequence(recording, tick, || set("0"), still_listed, || set("1"))?;
    OpenOptions::new().write(true).open(&switch).map_err(|e| {
        format!(
            "{}: not writable after the reconnect ({e}); the reconnect rule needs another trigger",
            switch.display()
        )
    })?;
    check_two_conversations(&mut lp360, &path)?;
    Ok(note)
}

fn reconnect_fake(out_dir: &Path, tick: Duration) -> Result<String, String> {
    let fake = FakeConnector::new([
        Presence::FailsAfter(6),
        Presence::Absent,
        Presence::Absent,
        Presence::Absent,
        Presence::Present,
    ]);
    let path = recording_path(out_dir, "fake-reconnect")?;
    let recording =
        Recording::create(fake, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (mut lp360, note) = reconnect_sequence(recording, tick, || Ok(()), || Ok(()), || Ok(()))?;
    check_two_conversations(&mut lp360, &path)?;
    Ok(note)
}

/// Five frames; off, then `show` every tick until it's disconnected (2 s at most); on, then
/// `show` every tick until it's connected again (10 s at most); five more frames.
fn reconnect_sequence<C: Connector>(
    connector: C,
    tick: Duration,
    mut off: impl FnMut() -> Result<(), String>,
    while_off: impl FnOnce() -> Result<(), String>,
    mut on: impl FnMut() -> Result<(), String>,
) -> Result<(Lp360<C>, String), String> {
    let (frame, _) = upright_42()?;
    let mut lp360 = Lp360::new(connector);
    let mut pace = Pace::new(tick);
    let steady = |lp360: &mut Lp360<C>, pace: &mut Pace| -> Result<(), String> {
        for i in 0..5 {
            pace.wait();
            let link = lp360.show(&frame);
            if link != Link::Connected {
                return Err(format!("steady frame {}: {link:?}", i + 1));
            }
        }
        Ok(())
    };
    steady(&mut lp360, &mut pace)?;
    off()?;
    let since = Instant::now();
    let gone_after = loop {
        pace.wait();
        match lp360.show(&frame) {
            Link::Disconnected(_) => break since.elapsed(),
            Link::Connected if since.elapsed() < Duration::from_secs(2) => {}
            Link::Connected => return Err("still connected 2 s after it was switched off".into()),
        }
    };
    while_off()?;
    on()?;
    let since = Instant::now();
    let back_after = loop {
        pace.wait();
        match lp360.show(&frame) {
            Link::Connected => break since.elapsed(),
            Link::Disconnected(_) if since.elapsed() < Duration::from_secs(10) => {}
            Link::Disconnected(reason) => {
                return Err(format!(
                    "not back 10 s after it was switched on: {reason:?}"
                ));
            }
        }
    };
    steady(&mut lp360, &mut pace)?;
    let note = format!(
        "gone after {:.1} s, back after {:.1} s, with 03 08 first again",
        gone_after.as_secs_f64(),
        back_after.as_secs_f64()
    );
    Ok((lp360, note))
}

/// The recording holds two conversations: `03 08`, frames, then a second `03 08` directly before
/// the first frame after the new connection, and every frame is the expected `42%`.
fn check_two_conversations<C>(lp360: &mut Lp360<Recording<C>>, path: &Path) -> Result<(), String>
where
    C: Connector,
{
    if let Some(failure) = lp360.connector().failure() {
        return Err(failure);
    }
    let (_, picture) = upright_42()?;
    let messages = read_recording(path)?;
    let startups: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, m)| **m == Message::Startup)
        .map(|(i, _)| i)
        .collect();
    let second = match startups.as_slice() {
        [0, second] => *second,
        _ => {
            return Err(format!(
                "{}: start-ups at {startups:?}; expected one first and one more",
                path.display()
            ));
        }
    };
    if !matches!(messages.get(second + 1), Some(Message::Frame(_))) {
        return Err(format!(
            "{}: no frame after the second 03 08",
            path.display()
        ));
    }
    for (i, message) in messages.iter().enumerate() {
        if let Message::Frame(frame) = message
            && let Some(report) = diff::describe(&picture, frame)
        {
            return Err(format!("message {}: {report}", i + 1));
        }
    }
    Ok(())
}

/// The device side of the wake-up from standby (owner, 2026-10-26): `reset_connection()` starts a
/// new conversation on the same connection.
fn reset(
    roots: &Roots,
    out_dir: &Path,
    tick: Duration,
    summary: &mut Summary,
) -> Result<String, String> {
    match probe(Capability::Lp360, roots) {
        Probe::Real => {
            summary.mode(Capability::Lp360, Mode::Real);
            reset_on(hidraw(roots), out_dir, tick)
        }
        Probe::Absent { reason } => {
            summary.mode(Capability::Lp360, Mode::Fake { reason });
            reset_on(FakeConnector::new([Presence::Present]), out_dir, tick)
        }
        Probe::Unusable { reason, fix } => {
            summary.mode(
                Capability::Lp360,
                Mode::Failed {
                    reason: reason.clone(),
                },
            );
            Err(format!("{reason}; fix: {fix}"))
        }
    }
}

fn reset_on<C: Connector>(connector: C, out_dir: &Path, tick: Duration) -> Result<String, String> {
    let (frame, _) = upright_42()?;
    let path = recording_path(out_dir, "reset")?;
    let recording =
        Recording::create(connector, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lp360 = Lp360::new(recording);
    let mut pace = Pace::new(tick);
    for i in 0..10 {
        if i == 5 {
            lp360.reset_connection();
        }
        pace.wait();
        let link = lp360.show(&frame);
        if link != Link::Connected {
            return Err(format!("frame {}: {link:?}", i + 1));
        }
    }
    check_two_conversations(&mut lp360, &path)?;
    Ok("a new conversation, 03 08 first, on the same connection".to_owned())
}
