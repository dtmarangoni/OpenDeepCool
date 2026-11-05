//! What the service checks show, as plans (Phase 7 plan, Task 9): a mode, a rotation, the
//! readings at the start, and for each tick its changes and the frame expected. A Load case of
//! `docs/spec/frames/` is a plan without changes; a scenario of `scenarios/mode-NNN.txt` is a plan
//! as written. Two players run them: the real dev service over D-Bus, its frames read back by tick
//! from its recording, and the engine in process, for a PC without the dev service.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use odc_ipc::Error;
use odc_protocol::{Frame, Message, Rotation, decode};
use odc_render::{Mode, mode_info};
use odc_sensors::{Exact, Fan, Metric, Reading, Snapshot, Unavailable};
use odc_service::engine::{Apply, Engine, Published};
use odc_service::settings::{Settings, TemperatureUnit};
use odc_testkit::capture::{Recorded, parse_recorded};
use odc_testkit::diff;
use odc_testkit::expected::{Case, Change, Kind, Picture, Reset, Scenario, Unit, every_metric};
use odc_testkit::hardware::DEV_RECORDING;

use crate::session::Session;

/// One frame per 100 ms tick (`docs/spec/protocol.md`, Timing).
pub const TICK: Duration = Duration::from_millis(100);

/// How far into a tick's window a change is posted, so it lands on the next tick.
const INTO_WINDOW: Duration = Duration::from_millis(30);

/// The dev recording is flushed once a second.
pub(crate) const FLUSHED_WITHIN: Duration = Duration::from_millis(1300);

/// An expected frame: the picture as the files draw it, and the quarter turns to apply to it.
#[derive(Clone, Debug)]
pub struct Expected {
    pub picture: Picture,
    pub turns: u8,
}

impl Expected {
    fn frame(&self) -> Result<Frame, String> {
        Ok(self
            .picture
            .to_frame()
            .ok_or("an expected picture with spectrum pixels")?
            .rotated(rotation(self.turns)))
    }

    /// Why `frame` isn't this, or `None` when it is.
    fn differs(&self, frame: &Frame) -> Result<Option<String>, String> {
        if *frame == self.frame()? {
            return Ok(None);
        }
        let back = frame.rotated(rotation((4 - self.turns % 4) % 4));
        Ok(Some(diff::describe(&self.picture, &back).unwrap_or_else(
            || format!("the frame differs once turned {}", self.turns),
        )))
    }
}

fn rotation(turns: u8) -> Rotation {
    Rotation::from_setting(turns % 4).expect("0–3")
}

/// One plan: the mode and rotation it starts in, the readings at its start, and ticks 0, 1, 2 …
#[derive(Clone, Debug)]
pub struct Plan {
    pub name: String,
    pub mode: u16,
    pub rotation: u8,
    pub start: Vec<(String, Option<Exact>)>,
    /// Each tick's changes (tick 0's are its start) and its frame; `None` for a missed tick.
    pub ticks: Vec<(Vec<Change>, Option<Expected>)>,
}

impl Plan {
    /// A Load case: its picture, already turned, for a few ticks, or its whole scrolling cycle
    /// and its first frame again.
    pub fn of_case(mode: u16, case: &Case) -> Result<Plan, String> {
        let pictures: Vec<Picture> = match &case.kind {
            Kind::Static(picture) => vec![picture.clone(); 5],
            Kind::Scrolling(cycle) => cycle.iter().chain(cycle.first()).cloned().collect(),
            Kind::Loop(_) => return Err(format!("{}: Load has no loop", case.heading)),
        };
        Ok(Plan {
            name: case.heading.trim_start_matches("## ").to_owned(),
            mode,
            rotation: case.rotation.setting(),
            start: case.readings.clone(),
            ticks: pictures
                .into_iter()
                .map(|picture| (Vec::new(), Some(Expected { picture, turns: 0 })))
                .collect(),
        })
    }

    /// A scenario as written, in rotation 0: each picture turned by the rotation its tick is in.
    pub fn of_scenario(scenario: &Scenario) -> Plan {
        let mut start = Vec::new();
        let mut turns = 0;
        let mut ticks = Vec::new();
        for tick in &scenario.ticks {
            let mut changes = Vec::new();
            for change in &tick.changes {
                match change {
                    Change::Reading { metric, value } if tick.n == 0 => {
                        start.push((metric.clone(), *value));
                    }
                    Change::Reset(Reset::StartUp) => {}
                    Change::Reset(Reset::Rotation(n)) => {
                        turns = (turns + n) % 4;
                        changes.push(change.clone());
                    }
                    _ => changes.push(change.clone()),
                }
            }
            let expected = tick
                .frame
                .clone()
                .map(|picture| Expected { picture, turns });
            ticks.push((changes, expected));
        }
        Plan {
            name: scenario.heading.trim_start_matches("## ").to_owned(),
            mode: scenario.mode,
            rotation: 0,
            start,
            ticks,
        }
    }

    /// The modes it asks for and the service must refuse as unavailable.
    fn refused_unavailable(&self) -> Vec<u16> {
        self.ticks
            .iter()
            .flat_map(|(changes, _)| changes)
            .filter_map(|c| match c {
                Change::Refused { mode, reason } if reason.contains("unavailable") => Some(*mode),
                _ => None,
            })
            .collect()
    }
}

/// The readings `OverrideReadings` takes: a value or `not-yet` for each metric, and `no-rule` for
/// each metric of a mode the plan needs refused as unavailable that its own mode doesn't show.
pub(crate) fn override_args(
    readings: &[(String, Option<Exact>)],
    mode: u16,
    no_rule: &[u16],
) -> (HashMap<String, (i64, u64)>, HashMap<String, String>) {
    let metrics = |m: u16| Mode::new(m).map_or(&[][..], |m| mode_info(m).metrics);
    let shown = metrics(mode);
    let never: Vec<Metric> = no_rule
        .iter()
        .flat_map(|&m| metrics(m).iter().copied())
        .filter(|metric| !shown.contains(metric))
        .collect();
    let (mut values, mut unavailable) = (HashMap::new(), HashMap::new());
    for (metric, value) in every_metric(readings) {
        let name = metric.spec_name().to_owned();
        match value {
            _ if never.contains(&metric) => {
                unavailable.insert(name, "no-rule".to_owned());
            }
            Some(exact) => {
                values.insert(name, (exact.num(), exact.den()));
            }
            None => {
                unavailable.insert(name, "not-yet".to_owned());
            }
        }
    }
    (values, unavailable)
}

/// The settings a real play may change, as the run found them, so each play starts from them.
#[derive(Clone, Debug)]
pub struct Current {
    pub unit: String,
    /// The GPU the user chose; `None` for Automatic.
    pub chosen_gpu: Option<String>,
    /// The service's GPUs by PCI address, best first: Automatic reads the first.
    pub gpus: Vec<String>,
}

/// The GPU to switch to from `now` (S25): the first other one in `gpus`, so two alternate.
pub fn other_gpu<'a>(gpus: &'a [String], now: Option<&str>) -> Option<&'a str> {
    gpus.iter()
        .map(String::as_str)
        .find(|pci| Some(*pci) != now)
}

/// Where a real play reaches the cooler's USB switch, for S18's disconnect.
pub type Switch<'a> = Option<&'a Path>;

/// Plays `plan` on the real dev service, inside `session`.
///
/// The service's ticks fall on absolute deadlines, and `OverrideReadings` returns just after the
/// tick that takes it: its return fixes the deadlines on this PC's clock. Each later change is
/// posted [`INTO_WINDOW`] into the window before the tick it must land on; a reading change's
/// returned tick proves it landed there, and the recorded frames prove the rest.
pub fn play_real(
    session: &Session,
    plan: &Plan,
    current: &Current,
    switch: Switch<'_>,
) -> Result<String, String> {
    let refused = plan.refused_unavailable();
    let other = (1..=6)
        .find(|m| *m != plan.mode && !refused.contains(m))
        .ok_or("no other Load mode")?;
    let mut readings = plan.start.clone();
    let (values, unavailable) = override_args(&readings, plan.mode, &refused);
    let dev = session.dev();
    let main = session.main();
    let said = |call: &str, e: Error| format!("{}: {call}: {e}", plan.name);
    zbus::block_on(dev.override_readings(values.clone(), unavailable.clone()))
        .map_err(|e| said("OverrideReadings", e))?;
    zbus::block_on(main.set_rotation(plan.rotation)).map_err(|e| said("SetRotation", e))?;
    zbus::block_on(main.set_mode(u32::from(other))).map_err(|e| said("SetMode", e))?;
    let synced = zbus::block_on(dev.override_readings(values, unavailable))
        .map_err(|e| said("OverrideReadings", e))?;
    let deadline = Instant::now();
    // The plan's tick 0: its mode, from home, on the tick after `synced`.
    zbus::block_on(main.set_mode(u32::from(plan.mode))).map_err(|e| said("SetMode", e))?;
    let first = synced + 1;
    let mut rotation_now = plan.rotation;
    let mut away = None;
    // The GPU read now: the chosen one, or Automatic's, the first.
    let mut gpu_now = current
        .chosen_gpu
        .as_deref()
        .or(current.gpus.first().map(String::as_str));
    let mut switched = false;
    for (n, (changes, _)) in plan.ticks.iter().enumerate().skip(1) {
        if changes.is_empty() {
            continue;
        }
        let at = deadline + TICK * u32::try_from(n).expect("a short plan") + INTO_WINDOW;
        if let Some(wait) = at.checked_duration_since(Instant::now()) {
            thread::sleep(wait);
        }
        let mut read = false;
        // Settings first: they return at once, while an override waits for its tick.
        for change in changes {
            match change {
                Change::Reading { metric, value } => {
                    readings.retain(|(m, _)| m != metric);
                    readings.push((metric.clone(), *value));
                    read = true;
                }
                Change::Reset(Reset::Mode(m)) => {
                    zbus::block_on(main.set_mode(u32::from(*m))).map_err(|e| said("SetMode", e))?;
                }
                Change::Reset(Reset::Rotation(turns)) => {
                    rotation_now = (rotation_now + turns) % 4;
                    if *turns == 1 {
                        zbus::block_on(main.rotate_screen())
                            .map_err(|e| said("RotateScreen", e))?;
                    } else {
                        zbus::block_on(main.set_rotation(rotation_now))
                            .map_err(|e| said("SetRotation", e))?;
                    }
                }
                Change::Reset(Reset::Refresh) => {
                    zbus::block_on(main.refresh()).map_err(|e| said("Refresh", e))?;
                }
                // Like Refresh: its reset waits for the new GPU's first snapshot.
                Change::Reset(Reset::AnotherGpu) => {
                    let other = other_gpu(&current.gpus, gpu_now)
                        .ok_or_else(|| format!("{}: no other GPU to switch to", plan.name))?;
                    zbus::block_on(main.choose_gpu(other)).map_err(|e| said("ChooseGpu", e))?;
                    gpu_now = Some(other);
                    switched = true;
                }
                // The same call either way; the service decides whether the picture resets.
                Change::UnitSaved(unit) | Change::Reset(Reset::Unit(unit)) => {
                    let unit = match unit {
                        Unit::Celsius => "celsius",
                        Unit::Fahrenheit => "fahrenheit",
                    };
                    zbus::block_on(main.set_temperature_unit(unit))
                        .map_err(|e| said("SetTemperatureUnit", e))?;
                }
                Change::AlreadySet(what) => match what.as_str() {
                    "mode" => zbus::block_on(main.set_mode(u32::from(current_mode(plan, n))))
                        .map_err(|e| said("SetMode", e))?,
                    "unit" => zbus::block_on(main.set_temperature_unit(&current.unit))
                        .map_err(|e| said("SetTemperatureUnit", e))?,
                    "rotation" => zbus::block_on(main.set_rotation(rotation_now))
                        .map_err(|e| said("SetRotation", e))?,
                    // Automatic is chosen by an empty address (owner, 2026-11-04).
                    "GPU" => {
                        zbus::block_on(main.choose_gpu(current.chosen_gpu.as_deref().unwrap_or("")))
                            .map_err(|e| said("ChooseGpu", e))?
                    }
                    other => return Err(format!("{}: already set: {other}?", plan.name)),
                },
                Change::Refused { mode, reason } => {
                    let refused = zbus::block_on(main.set_mode(u32::from(*mode)));
                    let right = match &refused {
                        Err(Error::NotYetAvailable(_)) => reason.contains("not yet"),
                        Err(Error::ModeUnavailable(_)) => reason.contains("unavailable"),
                        _ => false,
                    };
                    if !right {
                        return Err(format!(
                            "{}, tick {n}: SetMode({mode}) should be refused ({reason}): {refused:?}",
                            plan.name
                        ));
                    }
                }
                Change::CoolerAway | Change::CoolerBack => {
                    let switch = switch.ok_or("the cooler's USB switch isn't usable")?;
                    let state = if *change == Change::CoolerAway {
                        "0"
                    } else {
                        "1"
                    };
                    fs::write(switch, state).map_err(|e| format!("{}: {e}", switch.display()))?;
                    if *change == Change::CoolerAway {
                        away = Some(n);
                    }
                }
                Change::Reset(Reset::StartUp) | Change::Wake | Change::Missed => {
                    return Err(format!("{}: {change:?} can't be driven here", plan.name));
                }
            }
        }
        if read {
            let (values, unavailable) = override_args(&readings, plan.mode, &refused);
            let tick = zbus::block_on(dev.override_readings(values, unavailable))
                .map_err(|e| said("OverrideReadings", e))?;
            let want = first + n as u64;
            if tick != want {
                return Err(format!(
                    "{}: the change for tick {n} landed {} ticks late; the service missed its \
                     window",
                    plan.name,
                    tick.saturating_sub(want)
                ));
            }
        }
    }
    let end =
        deadline + TICK * u32::try_from(plan.ticks.len() + 1).expect("short") + FLUSHED_WITHIN;
    if let Some(wait) = end.checked_duration_since(Instant::now()) {
        thread::sleep(wait);
    }
    let result = recorded_by_tick(first, plan.ticks.len())
        .and_then(|recorded| compare(plan, first, &recorded, away));
    // The next play starts from the run's GPU choice; a play that fails earlier leaves it to the
    // run's `Restore`.
    if switched {
        zbus::block_on(main.choose_gpu(current.chosen_gpu.as_deref().unwrap_or("")))
            .map_err(|e| said("ChooseGpu", e))?;
    }
    result
}

/// The mode shown at tick `n`: the plan's, or the last mode switch's before it.
fn current_mode(plan: &Plan, n: usize) -> u16 {
    plan.ticks[..n]
        .iter()
        .flat_map(|(changes, _)| changes)
        .filter_map(|c| match c {
            Change::Reset(Reset::Mode(m)) => Some(*m),
            _ => None,
        })
        .next_back()
        .unwrap_or(plan.mode)
}

/// The recording's lines, the service's dev recording's unless `text` is given; a last line
/// without its newline, which the service may be writing, is left out.
pub(crate) fn recording_lines(text: Option<&str>) -> Result<Vec<Recorded>, String> {
    let read;
    let text = match text {
        Some(text) => text,
        None => {
            read =
                fs::read_to_string(DEV_RECORDING).map_err(|e| format!("{DEV_RECORDING}: {e}"))?;
            &read
        }
    };
    text.split_inclusive('\n')
        .filter_map(|l| l.strip_suffix('\n'))
        .enumerate()
        .map(|(i, line)| parse_recorded(i + 1, line).map_err(|why| format!("recording: {why}")))
        .collect()
}

/// A recorded report's message.
pub(crate) fn message(report: &Recorded) -> Result<Message, String> {
    let bytes: [u8; 64] = report
        .bytes
        .as_slice()
        .try_into()
        .map_err(|_| format!("recording line {}: not 64 bytes", report.line))?;
    decode(&bytes).map_err(|e| format!("recording line {}: {e}", report.line))
}

/// The dev recording's reports with ticks `first` to `first + len - 1`, each tick's in order.
fn recorded_by_tick(first: u64, len: usize) -> Result<Vec<Vec<Message>>, String> {
    let mut ticks = vec![Vec::new(); len];
    for report in recording_lines(None)? {
        let Some(n) = report.tick.checked_sub(first) else {
            continue;
        };
        if let Some(slot) = usize::try_from(n).ok().and_then(|n| ticks.get_mut(n)) {
            slot.push(message(&report)?);
        }
    }
    Ok(ticks)
}

/// Every tick's reports against the plan: one frame each, as expected. While the cooler is away
/// (S18), ticks may have none; the first back starts with `03 08`.
fn compare(
    plan: &Plan,
    first: u64,
    recorded: &[Vec<Message>],
    away: Option<usize>,
) -> Result<String, String> {
    let mut gone = false;
    let mut back_at = None;
    for (n, ((_, expected), reports)) in plan.ticks.iter().zip(recorded).enumerate() {
        let tick = first + n as u64;
        let frames: Vec<&Frame> = reports
            .iter()
            .filter_map(|m| match m {
                Message::Frame(f) => Some(f),
                Message::Startup => None,
            })
            .collect();
        let startup = reports.first() == Some(&Message::Startup);
        if away.is_some_and(|a| n >= a) && back_at.is_none() {
            if reports.is_empty() {
                gone = true;
                continue;
            }
            if gone {
                if !startup {
                    return Err(format!("{}, tick {n}: back without 03 08", plan.name));
                }
                back_at = Some(n);
            }
        } else if startup {
            return Err(format!(
                "{}, tick {n} ({tick}): an unexpected 03 08",
                plan.name
            ));
        }
        match (expected, frames.as_slice()) {
            (None, []) => {}
            (Some(want), [frame]) => {
                if let Some(why) = want.differs(frame)? {
                    return Err(format!("{}, tick {n} ({tick}): {why}", plan.name));
                }
            }
            (want, got) => {
                return Err(format!(
                    "{}, tick {n} ({tick}): {} frames recorded, {} expected",
                    plan.name,
                    got.len(),
                    usize::from(want.is_some())
                ));
            }
        }
    }
    match (away, back_at) {
        (Some(_), None) => Err(format!(
            "{}: the cooler didn't come back within the scenario",
            plan.name
        )),
        (Some(a), Some(b)) => Ok(format!(
            "{} ticks as expected; away from tick {a}, back at tick {b} with 03 08",
            plan.ticks.len()
        )),
        _ => Ok(format!("{} ticks as expected", plan.ticks.len())),
    }
}

/// Plays `plan` through the engine in process, tick by tick, as layer 3 does.
pub fn play_engine(plan: &Plan) -> Result<String, String> {
    let mut readings = plan.start.clone();
    let start = Settings {
        mode: plan.mode,
        rotation: plan.rotation,
        ..Settings::default()
    };
    let mut engine = Engine::new(start, snapshot(&readings));
    let mut gpus = 0;
    for (n, (changes, expected)) in plan.ticks.iter().enumerate() {
        let mut settings = engine.settings().clone();
        let (mut set, mut refresh, mut wake, mut missed, mut read) =
            (false, false, false, false, false);
        for change in changes {
            match change {
                Change::Reading { metric, value } => {
                    readings.retain(|(m, _)| m != metric);
                    readings.push((metric.clone(), *value));
                    read = true;
                }
                Change::Reset(Reset::Mode(m)) => {
                    settings.mode = *m;
                    set = true;
                }
                Change::Reset(Reset::Rotation(turns)) => {
                    settings.rotation = (settings.rotation + turns) % 4;
                    set = true;
                }
                Change::Reset(Reset::AnotherGpu) => {
                    gpus += 1;
                    settings.gpu_choice = Some(format!("0000:{:02x}:00.0", 0x0a + gpus));
                    set = true;
                }
                Change::Reset(Reset::Refresh) => refresh = true,
                Change::UnitSaved(unit) | Change::Reset(Reset::Unit(unit)) => {
                    settings.temperature_unit = match unit {
                        Unit::Celsius => TemperatureUnit::Celsius,
                        Unit::Fahrenheit => TemperatureUnit::Fahrenheit,
                    };
                    set = true;
                }
                Change::AlreadySet(_) => set = true,
                Change::Wake => wake = true,
                Change::Missed => missed = true,
                Change::Reset(Reset::StartUp)
                | Change::Refused { .. }
                | Change::CoolerAway
                | Change::CoolerBack => {}
            }
        }
        let apply = (set || refresh || wake).then(|| Apply {
            settings: set.then(|| settings.clone()),
            refresh,
            wake,
            readings: None,
        });
        let published = read.then(|| Published {
            snapshot: snapshot(&readings),
            answers: u64::MAX,
        });
        let out = engine.tick(apply, published, missed);
        match (expected, out.frame) {
            (None, None) => {}
            (Some(want), Some(frame)) => {
                if let Some(why) = want.differs(&frame)? {
                    return Err(format!("{}, tick {n}: {why}", plan.name));
                }
            }
            (want, frame) => {
                return Err(format!(
                    "{}, tick {n}: expected {} frame, drew {}",
                    plan.name,
                    if want.is_some() { "a" } else { "no" },
                    if frame.is_some() { "one" } else { "none" }
                ));
            }
        }
    }
    Ok(format!("{} ticks as expected", plan.ticks.len()))
}

/// The engine's snapshot of `readings`: each metric named or its own filler, `None` as not yet
/// read.
fn snapshot(readings: &[(String, Option<Exact>)]) -> Snapshot {
    let all = every_metric(readings);
    Snapshot::new(
        |metric| {
            let value = all.iter().find(|(m, _)| *m == metric).and_then(|(_, v)| *v);
            match value {
                Some(exact) => Reading::Value(exact),
                None => Reading::Unavailable(Unavailable::NotYet),
            }
        },
        Fan::NotChosen,
        Fan::NotChosen,
    )
}
