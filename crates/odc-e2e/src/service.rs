//! The service checks (Phase 7 plan, Task 9), `--only service`. The real, sandboxed dev service
//! drives the cooler through one example of each Load mode and of each test type, one variant of
//! each scenario type a run can drive, a five-minute timing run and the start check, unattended
//! and compared with the expected frames; afterwards the owner's settings are as they were found.
//! Without a dev service, the examples and scenarios run through the engine in process, the
//! timing through the engine's own loop, and the start check against a made-up journal and
//! recording (rule 10: one from the owner's PC would name its drivers).

use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use odc_ipc::State;
use odc_protocol::{Frame, Message, encode_frame, startup_report};
use odc_render::{Marquee, Mode as RenderMode, Unit as RenderUnit, Values, picture};
use odc_sensors::{Exact, Metric, Reading};
use odc_service::engine::{Apply, Engine, Published, Requests};
use odc_service::runtime::{Outputs, Progress, Slot, SystemTicker, run_engine};
use odc_service::settings::Settings;
use odc_service::status::{FIRST_READINGS, parse_first_readings};
use odc_testkit::expected::{Case, Kind, load_mode, load_scenarios, load_unavailable};
use odc_testkit::hardware::{Capability, DEV_UNIT, Mode, Probe, Roots, Summary, probe};
use odc_testkit::sysfs::{LP360_PRODUCT, LP360_VENDOR, usb_device};

use crate::plays::{
    Current, FLUSHED_WITHIN, Plan, TICK, message, override_args, play_engine, play_real,
    recording_lines,
};
use crate::session::Session;

/// The timing run (owner, 2026-10-27; solution review P7-13): about 5 minutes at 100 ms.
const TIMING_TICKS: u64 = 3000;

/// DeepCreative's captured mean interval (`docs/spec/protocol.md`, Timing), in ms.
const MEAN_WITHIN: (f64, f64) = (99.98, 100.06);

/// Every interval between two frames, in ms: no frame dropped or doubled.
const EACH_WITHIN: (f64, f64) = (50.0, 150.0);

pub fn run(roots: &Roots, tick: Duration, summary: &mut Summary) {
    match probe(Capability::DevService, roots) {
        Probe::Real => {
            summary.mode(Capability::DevService, Mode::Real);
            real(roots, summary);
        }
        Probe::Absent { reason } => {
            summary.mode(Capability::DevService, Mode::Fake { reason });
            fake(tick, summary);
        }
        Probe::Unusable { reason, fix } => {
            summary.mode(
                Capability::DevService,
                Mode::Failed {
                    reason: reason.clone(),
                },
            );
            summary.check("service", Err(format!("{reason}; fix: {fix}")));
        }
    }
}

/// Where a scenario type runs on the real service, or why it runs only in process.
pub(crate) fn real_or_why(
    id: &str,
    gpus: usize,
    chosen: bool,
    switch: bool,
) -> Result<(), &'static str> {
    match id {
        "S17" => Err("a start-up needs a restart, which needs root: the start check covers it"),
        "S22" => Err("a missed tick needs a stalled engine"),
        "S25" if gpus < 2 => Err("this PC has one GPU"),
        "S25" if !chosen => Err("nobody chose a GPU, and a choice can't be taken back"),
        "S18" if !switch => Err("the cooler's USB switch isn't usable"),
        _ => Ok(()),
    }
}

/// The owner's settings as the run found them, set back when it ends, however it ends.
pub(crate) struct Restore<'a> {
    pub(crate) session: &'a Session,
    pub(crate) state: State,
}

impl Drop for Restore<'_> {
    fn drop(&mut self) {
        let main = self.session.main();
        let dev = self.session.dev();
        let mut said = Vec::new();
        if let Err(e) = zbus::block_on(dev.clear_override()) {
            said.push(format!("ClearOverride: {e}"));
        }
        if let Some(unit) = self.state.temperature_unit
            && let Err(e) = zbus::block_on(main.set_temperature_unit(unit.as_str()))
        {
            said.push(format!("SetTemperatureUnit: {e}"));
        }
        if let Some(rotation) = self.state.rotation
            && let Err(e) = zbus::block_on(main.set_rotation(rotation))
        {
            said.push(format!("SetRotation: {e}"));
        }
        if let Some(mode) = self.state.mode
            && let Err(e) = zbus::block_on(main.set_mode(mode))
        {
            said.push(format!("SetMode: {e}"));
        }
        for line in said {
            eprintln!("odc-e2e: the owner's settings weren't all set back: {line}");
        }
    }
}

pub(crate) fn state(session: &Session) -> Result<State, String> {
    let dict = zbus::block_on(session.main().get_state()).map_err(|e| format!("GetState: {e}"))?;
    State::from_dict(&dict).map_err(|e| format!("GetState: {e}"))
}

fn real(roots: &Roots, summary: &mut Summary) {
    let session = match Session::begin() {
        Ok(session) => session,
        Err(e) => return summary.check("service", Err(e)),
    };
    let found = match state(&session) {
        Ok(found) => found,
        Err(e) => return summary.check("service", Err(e)),
    };
    // First, before anything changes: the instance's own start.
    summary.check("service_start", start_real(&found));
    let current = Current {
        unit: found
            .temperature_unit
            .map_or("celsius", |u| u.as_str())
            .to_owned(),
        chosen_gpu: found.chosen_gpu.clone().filter(|pci| !pci.is_empty()),
    };
    let gpus = found.gpus.as_ref().map_or(0, Vec::len);
    let _restore = Restore {
        session: &session,
        state: found,
    };
    summary.check("service_timing", timing_real(&session));
    summary.check("service_durations", durations(&session));

    let switch = usb_device(&roots.sysfs, LP360_VENDOR, LP360_PRODUCT)
        .map(|usb| usb.join("authorized"))
        .filter(|_| probe(Capability::Lp360Reconnect, roots) == Probe::Real);
    let started = Instant::now();
    for (name, plan) in examples() {
        let result = plan.and_then(|plan| play_real(&session, &plan, &current, None));
        summary.check(&name, result);
    }
    for (id, plan) in scenario_plans() {
        let name = format!("service_{id}");
        let result = match real_or_why(&id, gpus, current.chosen_gpu.is_some(), switch.is_some()) {
            Ok(()) => play_real(&session, &plan, &current, switch.as_deref()),
            Err(why) => play_engine(&plan).map(|note| format!("in process: {why}; {note}")),
        };
        summary.check(&name, result);
    }
    println!(
        "odc-e2e: the service's examples and scenarios took {} s",
        started.elapsed().as_secs()
    );
}

fn fake(tick: Duration, summary: &mut Summary) {
    summary.check("service_start", start_fake());
    summary.check("service_timing", timing_fake(tick));
    for (name, plan) in examples() {
        let result = plan.and_then(|plan| play_engine(&plan));
        summary.check(&name, result);
    }
    for (id, plan) in scenario_plans() {
        summary.check(&format!("service_{id}"), play_engine(&plan));
    }
}

/// One example of each Load mode, value set 2 upright, and of each further test type: a rotated
/// case, a scrolling case's whole cycle, and a dash case (Phase 7 plan, Task 9).
fn examples() -> Vec<(String, Result<Plan, String>)> {
    let case = |mode: u16, rotation: u8, index: usize| -> Result<Plan, String> {
        let file = load_mode(mode);
        let case: &Case = file
            .cases
            .iter()
            .filter(|c| c.rotation.setting() == rotation)
            .nth(index)
            .ok_or_else(|| format!("mode-{mode:03}.txt: no value set {}", index + 1))?;
        Plan::of_case(mode, case)
    };
    let mut examples: Vec<(String, Result<Plan, String>)> = (1..=6)
        .map(|mode| (format!("service_mode_{mode}"), case(mode, 0, 1)))
        .collect();
    examples.push(("service_rotated".into(), case(1, 1, 1)));
    let scrolling = (1..=6)
        .find_map(|mode| {
            let file = load_mode(mode);
            file.cases
                .iter()
                .find(|c| c.rotation.setting() == 0 && matches!(c.kind, Kind::Scrolling(_)))
                .map(|c| Plan::of_case(mode, c))
        })
        .unwrap_or_else(|| Err("no Load mode has a scrolling case".into()));
    examples.push(("service_scrolling".into(), scrolling));
    let dashes = load_unavailable()
        .into_iter()
        .find(|(_, c)| c.rotation.setting() == 0)
        .map_or_else(
            || Err("unavailable.txt has no upright Load case".into()),
            |(mode, c)| Plan::of_case(mode, &c),
        );
    examples.push(("service_dashes".into(), dashes));
    examples
}

/// The first variant of each scenario type of `scenarios/load.txt`, in file order.
pub(crate) fn scenario_plans() -> Vec<(String, Plan)> {
    let mut seen = Vec::new();
    let mut plans = Vec::new();
    for scenario in load_scenarios("load.txt") {
        let id = scenario.heading.trim_start_matches("## ");
        let id = id.split(' ').next().unwrap_or(id).to_owned();
        if seen.contains(&id) {
            continue;
        }
        seen.push(id.clone());
        plans.push((id, Plan::of_scenario(&scenario)));
    }
    plans
}

fn millis(ns: u64) -> f64 {
    ns as f64 / 1e6
}

/// p50, p99 and the maximum of `values` in ms.
fn spread(values: &mut [f64]) -> String {
    values.sort_by(f64::total_cmp);
    if values.is_empty() {
        return "none".into();
    }
    let at = |q: usize| values[(values.len() * q / 100).min(values.len() - 1)];
    format!(
        "p50 {:.2} ms, p99 {:.2} ms, max {:.2} ms",
        at(50),
        at(99),
        values[values.len() - 1]
    )
}

/// The intervals between frames written at `times` (ns), checked against DeepCreative's.
fn intervals(times: &[u64], bounds_mean: (f64, f64)) -> Result<String, String> {
    let mut gaps: Vec<f64> = times.windows(2).map(|w| millis(w[1] - w[0])).collect();
    if gaps.is_empty() {
        return Err("fewer than two frames".into());
    }
    let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
    if let Some(bad) = gaps
        .iter()
        .find(|g| !(EACH_WITHIN.0..=EACH_WITHIN.1).contains(*g))
    {
        return Err(format!(
            "an interval of {bad:.2} ms, outside {}–{} ms: a frame dropped or doubled",
            EACH_WITHIN.0, EACH_WITHIN.1
        ));
    }
    if !(bounds_mean.0..=bounds_mean.1).contains(&mean) {
        return Err(format!(
            "mean interval {mean:.3} ms, outside {}–{} ms",
            bounds_mean.0, bounds_mean.1
        ));
    }
    Ok(format!(
        "{} frames, mean {mean:.3} ms; {}",
        times.len(),
        spread(&mut gaps)
    ))
}

/// About 5 minutes of mode 1 at fixed readings; then the intervals between the frames' write
/// times, and the delay from the instance's `03 08` to its first frame.
fn timing_real(session: &Session) -> Result<String, String> {
    let readings = vec![("CPU Usage".to_owned(), Some(Exact::whole(42)))];
    let (values, unavailable) = override_args(&readings, 1, &[]);
    zbus::block_on(
        session
            .dev()
            .override_readings(values.clone(), unavailable.clone()),
    )
    .map_err(|e| format!("OverrideReadings: {e}"))?;
    zbus::block_on(session.main().set_mode(1)).map_err(|e| format!("SetMode: {e}"))?;
    let first = zbus::block_on(session.dev().override_readings(values, unavailable))
        .map_err(|e| format!("OverrideReadings: {e}"))?;
    let last = first + TIMING_TICKS;
    std::thread::sleep(TICK * u32::try_from(TIMING_TICKS + 2).expect("ticks") + FLUSHED_WITHIN);
    let lines: Vec<_> = recording_lines(None)?
        .into_iter()
        .filter(|r| (first..last).contains(&r.tick))
        .collect();
    let ticks: Vec<u64> = lines.iter().map(|r| r.tick).collect();
    if ticks != (first..last).collect::<Vec<_>>() {
        return Err(format!(
            "ticks {first}–{}: {} reports, not one each",
            last - 1,
            ticks.len()
        ));
    }
    let times: Vec<u64> = lines.iter().map(|r| r.ns).collect();
    let note = intervals(&times, MEAN_WITHIN)?;
    let start = recording_lines(None)?;
    let delay = match (start.first(), start.get(1)) {
        (Some(a), Some(b)) if message(a)? == Message::Startup => {
            format!("{:.2} ms", millis(b.ns - a.ns))
        }
        _ => "unknown: the recording doesn't start with 03 08 (rotated?)".to_owned(),
    };
    Ok(format!(
        "{note}; 03 08 to the first frame at the start: {delay}"
    ))
}

/// The engine's own loop on this PC, for as long as `tick` scales the real run.
fn timing_fake(tick: Duration) -> Result<String, String> {
    let ticks = (TIMING_TICKS * tick.as_micros() as u64 / TICK.as_micros() as u64).max(10);
    struct Times(Vec<Instant>);
    impl Outputs for Times {
        fn frame(&mut self, _: u64, _: Frame) {
            self.0.push(Instant::now());
        }
        fn requests(&mut self, _: Requests) {}
        fn status(&mut self, _: &Engine) {}
        fn notify(&mut self, _: &str) {}
        fn skipped(&mut self, _: u64, _: u64) {}
    }
    let snapshot = odc_sensors::Snapshot::new(
        |_| Reading::Value(Exact::whole(42)),
        odc_sensors::Fan::NotChosen,
        odc_sensors::Fan::NotChosen,
    );
    let mut times = Times(Vec::new());
    let began = Instant::now();
    run_engine(
        Engine::new(Settings::default(), snapshot),
        &SystemTicker,
        &Slot::<Apply>::default(),
        &Slot::<Published>::default(),
        &Progress::new(began),
        &mut times,
        &AtomicBool::new(false),
        Some(ticks),
    );
    let ns: Vec<u64> = times
        .0
        .iter()
        .map(|t| u64::try_from(t.duration_since(began).as_nanos()).unwrap_or(u64::MAX))
        .collect();
    // A short run on fakes: the mean only within 1 ms.
    intervals(&ns, (99.0, 101.0)).map(|note| format!("the engine's own loop: {note}"))
}

/// The writes' and the snapshots' durations since the session began.
fn durations(session: &Session) -> Result<String, String> {
    let (writes, snapshots) =
        zbus::block_on(session.dev().durations()).map_err(|e| format!("Durations: {e}"))?;
    if writes.is_empty() || snapshots.is_empty() {
        return Err(format!(
            "{} writes and {} snapshots timed",
            writes.len(),
            snapshots.len()
        ));
    }
    let ms = |list: &[u64]| -> Vec<f64> { list.iter().map(|&us| us as f64 / 1000.0).collect() };
    Ok(format!(
        "{} writes: {}; {} snapshots: {}",
        writes.len(),
        spread(&mut ms(&writes)),
        snapshots.len(),
        spread(&mut ms(&snapshots))
    ))
}

/// The units a start must never wait for (owner, 2026-10-25): a login, a session, the desktop.
fn waits_for_a_login(unit: &str) -> bool {
    [
        "graphical.target",
        "display-manager.service",
        "systemd-logind.service",
        "systemd-user-sessions.service",
    ]
    .contains(&unit)
        || unit.starts_with("user@")
        || unit.starts_with("session-")
        || unit.starts_with("user-runtime-dir@")
}

fn systemctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("systemctl")
        .args(args)
        .output()
        .map_err(|e| format!("systemctl: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// The start check (Phase 7 plan, Task 9; no reboot, owner 2026-10-25): enabled at boot, wanted
/// by `multi-user.target` and waiting for no login; and the instance's own start, `03 08` then
/// the saved mode's picture drawn from the readings it journalled.
fn start_real(found: &State) -> Result<String, String> {
    let enabled = systemctl(&["is-enabled", DEV_UNIT])?;
    if enabled != "enabled" {
        return Err(format!("{DEV_UNIT} is {enabled}, not enabled"));
    }
    let show = systemctl(&[
        "show",
        "--property=WantedBy,After,Requires,Wants,InvocationID",
        DEV_UNIT,
    ])?;
    let value = |key: &str| -> String {
        show.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
            .unwrap_or("")
            .to_owned()
    };
    if !value("WantedBy")
        .split(' ')
        .any(|u| u == "multi-user.target")
    {
        return Err(format!("{DEV_UNIT} isn't wanted by multi-user.target"));
    }
    for key in ["After", "Requires", "Wants"] {
        if let Some(unit) = value(key).split(' ').find(|u| waits_for_a_login(u)) {
            return Err(format!(
                "{DEV_UNIT} has {key}={unit}: it would wait for a login"
            ));
        }
    }
    let invocation = value("InvocationID");
    let out = Command::new("journalctl")
        .args([
            "--no-pager",
            "--output=cat",
            "--unit",
            DEV_UNIT,
            &format!("_SYSTEMD_INVOCATION_ID={invocation}"),
        ])
        .output()
        .map_err(|e| format!("journalctl: {e}"))?;
    let journal = String::from_utf8_lossy(&out.stdout).into_owned();
    if journal.trim().is_empty() {
        return Err(
            "the journal shows nothing of this instance: this account needs to read the system \
             journal (the wheel, adm or systemd-journal group)"
                .into(),
        );
    }
    let saved = (
        found.mode.ok_or("GetState gave no Mode")?,
        found.rotation.ok_or("GetState gave no Rotation")?,
        found
            .temperature_unit
            .is_some_and(|u| u.as_str() == "fahrenheit"),
    );
    let lines = recording_lines(None)?;
    check_start(&journal, &lines, saved).map(|note| {
        format!(
            "enabled, wanted by multi-user.target, waits for no login; {note} (drawn as the \
             settings are now)"
        )
    })
}

/// The instance's recording starts with `03 08`, then the saved mode's picture at home, drawn
/// from the readings its `first readings` journal line gives.
fn check_start(
    journal: &str,
    lines: &[odc_testkit::capture::Recorded],
    (mode, rotation, fahrenheit): (u32, u8, bool),
) -> Result<String, String> {
    let line = journal
        .lines()
        .find(|l| l.starts_with(FIRST_READINGS))
        .ok_or("no `first readings` line in the instance's journal")?;
    let readings = parse_first_readings(line).ok_or_else(|| format!("unreadable: {line}"))?;
    let (Some(a), Some(b)) = (lines.first(), lines.get(1)) else {
        return Err("the recording has fewer than two reports".into());
    };
    if message(a)? != Message::Startup {
        return Err("the recording doesn't start with 03 08 (it rotated since the start?)".into());
    }
    let Message::Frame(frame) = message(b)? else {
        return Err("the second report isn't a frame".into());
    };
    let mut values = Values::new();
    for (metric, reading) in &readings {
        values.set(
            *metric,
            match reading {
                Reading::Value(exact) => Some(*exact),
                Reading::Unavailable(_) => None,
            },
        );
    }
    let mode = u16::try_from(mode).map_err(|_| format!("mode {mode}"))?;
    let unit = if fahrenheit {
        RenderUnit::Fahrenheit
    } else {
        RenderUnit::Celsius
    };
    let render = RenderMode::new(mode).ok_or_else(|| format!("no mode {mode}"))?;
    let p = picture(render, &values, unit).map_err(|_| format!("mode {mode} isn't drawn"))?;
    let want = Marquee::new(p)
        .tick()
        .rotated(odc_protocol::Rotation::from_setting(rotation).ok_or("a rotation over 3")?);
    if frame != want {
        return Err(format!(
            "the first frame isn't mode {mode}'s picture, rotation {rotation}, of the journalled \
             readings ({line})"
        ));
    }
    let shown: Vec<&str> = readings
        .iter()
        .filter(|(m, _)| odc_render::mode_info(render).metrics.contains(m))
        .map(|(m, _)| m.spec_name())
        .collect();
    Ok(format!(
        "03 08, then mode {mode}'s picture of its first readings ({})",
        shown.join(", ")
    ))
}

/// The start check on a made-up instance: its journal names a fake chip, its recording is
/// `03 08` and mode 1's `42%` as the oracle drew it.
fn start_fake() -> Result<String, String> {
    let file = load_mode(1);
    let case = file
        .cases
        .iter()
        .find(|c| {
            c.rotation.setting() == 0
                && c.readings == [("CPU Usage".to_owned(), Some(Exact::whole(42)))]
        })
        .ok_or("mode-001.txt has no upright 42% case")?;
    let Kind::Static(picture42) = &case.kind else {
        return Err("mode 1's 42% scrolls?".into());
    };
    let frame = picture42.to_frame().ok_or("spectrum pixels")?;
    let readings: Vec<String> = Metric::ALL
        .iter()
        .map(|&m| {
            let value = if m == Metric::CpuUsage {
                42
            } else {
                10 + m as i64
            };
            format!("{} {value}/1", m.spec_name())
        })
        .collect();
    let journal = format!(
        "CPU Temperature: fakechip temp1_input (Package), /sys/devices/platform/fakechip.0/temp1_input\n\
         {FIRST_READINGS}{}\nstarted, with the dev interface\n",
        readings.join(", ")
    );
    let hex = |bytes: &[u8]| -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() };
    let recording = format!(
        "1000 0 out 64 {}\n7000000 0 out 64 {}\n",
        hex(startup_report().as_bytes()),
        hex(encode_frame(&frame).as_bytes())
    );
    check_start(&journal, &recording_lines(Some(&recording))?, (1, 0, false))
        .map(|note| format!("on a made-up journal and recording: {note}"))
}
