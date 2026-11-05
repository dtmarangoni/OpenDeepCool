//! Layer 3, the engine's tick logic (Phase 7 plan, Task 6): every scenario of
//! `docs/spec/frames/scenarios/` whose modes this build draws, through `Engine`, which turns each
//! frame itself, in all four rotations; the newest settings; Refresh and another GPU waiting for
//! their snapshot; availability with late sources; dashes. The threads and the clock are tested in
//! `runtime.rs`.

use odc_protocol::Rotation;
use odc_render::{Mode, draws};
use odc_sensors::{Exact, Fan, Metric, Reading, Snapshot, Unavailable};
use odc_service::engine::{Apply, Engine, LATE_SOURCE_TICKS, Override, Published, availability};
use odc_service::settings::{Settings, TemperatureUnit};
use odc_testkit::diff;
use odc_testkit::expected::{Change, Reset, Scenario, Unit, every_metric, mode_scenarios};

/// A snapshot with the readings named and every other metric's filler; an unavailable reading is
/// `NotYet`, which shows dashes.
fn snapshot(readings: &[(String, Option<Exact>)]) -> Snapshot {
    let all = every_metric(readings);
    Snapshot::new(
        |metric| {
            let (_, value) = all
                .iter()
                .find(|(m, _)| *m == metric)
                .expect("every metric");
            match value {
                Some(v) => Reading::Value(*v),
                None => Reading::Unavailable(Unavailable::NotYet),
            }
        },
        Fan::NotChosen,
        Fan::NotChosen,
    )
}

fn fresh(readings: &[(String, Option<Exact>)]) -> Published {
    Published {
        snapshot: snapshot(readings),
        answers: u64::MAX,
    }
}

/// The settings' unit for a unit of the expected files.
fn unit(unit: Unit) -> TemperatureUnit {
    match unit {
        Unit::Celsius => TemperatureUnit::Celsius,
        Unit::Fahrenheit => TemperatureUnit::Fahrenheit,
    }
}

/// Whether this build draws every mode `scenario` shows: the one it starts in and each it switches
/// into. A scenario that shows another waits for that mode's commit (Phase 8 plan, Task 1).
fn drawn(scenario: &Scenario) -> bool {
    let switches = scenario
        .ticks
        .iter()
        .flat_map(|t| &t.changes)
        .filter_map(|c| match c {
            Change::Reset(Reset::Mode(m)) => Some(*m),
            _ => None,
        });
    std::iter::once(scenario.mode)
        .chain(switches)
        .all(|m| Mode::new(m).is_some_and(draws))
}

/// Replays `scenario` through the engine from rotation `base`, comparing every frame.
fn replay(scenario: &Scenario, base: u8) {
    let mut readings: Vec<(String, Option<Exact>)> = Vec::new();
    for change in &scenario.ticks[0].changes {
        if let Change::Reading { metric, value } = change {
            readings.push((metric.clone(), *value));
        }
    }
    let start = Settings {
        mode: scenario.mode,
        rotation: base,
        temperature_unit: unit(scenario.unit),
        ..Settings::default()
    };
    let mut engine = Engine::new(start, snapshot(&readings));
    let mut gpus = 0;
    for tick in &scenario.ticks {
        let mut settings = engine.settings().clone();
        let (mut set, mut refresh, mut wake, mut missed, mut read) =
            (false, false, false, false, false);
        if tick.n > 0 {
            for change in &tick.changes {
                match change {
                    Change::Reading { metric, value } => {
                        readings.retain(|(m, _)| m != metric);
                        readings.push((metric.clone(), *value));
                        read = true;
                    }
                    Change::Reset(Reset::Mode(n)) => {
                        settings.mode = *n;
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
                    Change::Reset(Reset::StartUp) => {
                        panic!("{}: a second start-up", scenario.heading)
                    }
                    // The same call either way; the engine decides whether the picture resets.
                    Change::UnitSaved(new) | Change::Reset(Reset::Unit(new)) => {
                        settings.temperature_unit = unit(*new);
                        set = true;
                    }
                    // Choosing what's already set sends the same settings.
                    Change::AlreadySet(_) => set = true,
                    Change::Wake => wake = true,
                    Change::Missed => missed = true,
                    // A refused change never reaches the engine; the cooler's comings and
                    // goings are the device thread's.
                    Change::Refused { .. } | Change::CoolerAway | Change::CoolerBack => {}
                }
            }
        }
        let apply = (set || refresh || wake).then(|| Apply {
            settings: set.then(|| settings.clone()),
            refresh,
            wake,
            readings: None,
        });
        let published = read.then(|| fresh(&readings));
        let out = engine.tick(apply, published, missed);
        assert_eq!(out.number, u64::from(tick.n), "{}", scenario.heading);
        match (&tick.frame, out.frame) {
            (None, None) => {}
            (Some(expected), Some(frame)) => {
                let rotation = Rotation::from_setting(engine.settings().rotation).expect("0–3");
                let want = expected.to_frame().expect("no spectrum").rotated(rotation);
                if frame != want {
                    let back = Rotation::from_setting((4 - rotation.setting()) % 4).expect("0–3");
                    diff::assert_matches(
                        expected,
                        &frame.rotated(back),
                        &format!(
                            "{}, from rotation {base}, tick {}",
                            scenario.heading, tick.n
                        ),
                    );
                    panic!(
                        "{}: tick {}: the turned frame differs",
                        scenario.heading, tick.n
                    );
                }
            }
            (expected, frame) => panic!(
                "{}, tick {}: expected {} frame, drew {}",
                scenario.heading,
                tick.n,
                if expected.is_some() { "a" } else { "no" },
                if frame.is_some() { "one" } else { "none" }
            ),
        }
    }
}

// The scenarios S1–S26 of each mode with values, the change in the top value, the bottom one
// and both, every k, in each unit where a temperature is shown: S14–S18 and S21–S26 above all,
// which only the engine can play out.
#[test]
fn every_scenario_of_the_drawn_modes_plays_out_through_the_engine_in_all_four_rotations() {
    let (mut replayed, mut skipped) = (0, 0);
    for mode in 1..=12 {
        let scenarios = mode_scenarios(mode);
        assert!(!scenarios.is_empty(), "mode {mode}: no scenario");
        for scenario in &scenarios {
            if !drawn(scenario) {
                skipped += 1;
                continue;
            }
            for base in 0..4 {
                replay(scenario, base);
            }
            replayed += 1;
        }
    }
    println!("{replayed} scenarios replayed, {skipped} showing a mode not drawn yet skipped");
    assert_eq!(replayed + skipped, 10_403, "the scenarios of modes 1–12");
    assert!(replayed >= 2499, "Phase 7's Load scenarios at least");
    if (1..=12).all(|m| Mode::new(m).is_some_and(draws)) {
        assert_eq!(skipped, 0);
    }
}

fn readings(pairs: &[(&str, Option<i64>)]) -> Vec<(String, Option<Exact>)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.map(Exact::whole)))
        .collect()
}

fn usage(cpu: i64) -> Vec<(String, Option<Exact>)> {
    readings(&[
        ("CPU Usage", Some(cpu)),
        ("Gpu usage", Some(67)),
        ("Memory Load", Some(85)),
    ])
}

/// The picture of `mode` at home with `readings`, as a fresh engine draws it.
fn home(mode: u16, readings: &[(String, Option<Exact>)]) -> odc_protocol::Frame {
    let settings = Settings {
        mode,
        ..Settings::default()
    };
    Engine::new(settings, snapshot(readings))
        .tick(None, None, false)
        .frame
        .expect("a frame")
}

fn readings_override(id: u64, readings: Option<&[(String, Option<Exact>)]>) -> Option<Apply> {
    Some(Apply {
        readings: Some(Override {
            id,
            readings: readings.map(snapshot),
        }),
        ..Apply::default()
    })
}

// The dev copy's override (`docs/spec/dbus.md`, Dev interface; owner, 2026-10-29): it's drawn from
// the tick that takes it, which the engine reports; real snapshots replace only what's behind it,
// so they still answer Refresh, carrying the override's values; its end brings the last real
// readings back at once.
#[test]
fn an_override_is_drawn_from_the_tick_that_takes_it_until_its_end() {
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(42)));
    engine.tick(None, None, false);
    let taken = engine.tick(readings_override(1, Some(&usage(77))), None, false);
    assert_eq!(engine.adopted(), Some((1, taken.number)));
    assert_eq!(taken.frame, Some(home(1, &usage(77))), "from that tick");
    // A real snapshot changes nothing drawn.
    let real = engine.tick(None, Some(fresh(&usage(43))), false);
    assert_eq!(real.frame, Some(home(1, &usage(77))));
    // Refresh still waits for a real snapshot answering it, and resets with the override's values.
    let asked = engine.tick(
        Some(Apply {
            refresh: true,
            ..Apply::default()
        }),
        None,
        false,
    );
    let id = asked.requests.snapshot.expect("a snapshot request");
    let answered = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&usage(44)),
            answers: id,
        }),
        false,
    );
    assert_eq!(answered.frame, Some(home(1, &usage(77))));
    // The end: the last real readings, at once.
    let ended = engine.tick(readings_override(2, None), None, false);
    assert_eq!(engine.adopted(), Some((2, ended.number)));
    assert_eq!(ended.frame, Some(home(1, &usage(44))));
}

// Two overrides before one tick: the newer is drawn, and its number reported, so the caller of
// the older one learns the tick too.
#[test]
fn only_the_newest_override_is_taken() {
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(42)));
    let older = readings_override(1, Some(&usage(55))).expect("an override");
    let newer = readings_override(2, Some(&usage(66))).expect("an override");
    let out = engine.tick(Some(older.then(newer)), None, false);
    assert_eq!(engine.adopted(), Some((2, out.number)));
    assert_eq!(out.frame, Some(home(1, &usage(66))));
}

// P7-15: two changes before one tick: only the second is adopted; a change and a change back
// adopt nothing new, so the strip keeps its place.
#[test]
fn only_the_newest_settings_are_adopted() {
    let mode = |m: u16| Apply {
        settings: Some(Settings {
            mode: m,
            ..Settings::default()
        }),
        ..Apply::default()
    };
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(42)));
    engine.tick(None, None, false);
    let out = engine.tick(Some(mode(2).then(mode(3))), None, false);
    assert_eq!(engine.settings().mode, 3);
    assert_eq!(out.frame, Some(home(3, &usage(42))));

    // 1 → 2 → 1 while 100% scrolls: no reset; the strip moves on.
    let mut scrolling = Engine::new(Settings::default(), snapshot(&usage(100)));
    let mut alone = Engine::new(Settings::default(), snapshot(&usage(100)));
    for _ in 0..5 {
        scrolling.tick(None, None, false);
        alone.tick(None, None, false);
    }
    let back = scrolling.tick(Some(mode(2).then(mode(1))), None, false);
    assert_eq!(back.frame, alone.tick(None, None, false).frame);
}

// P7-06: Refresh's reset lands on the first tick that has the fresh snapshot, never on an older
// one that arrives meanwhile.
#[test]
fn refresh_resets_on_the_first_tick_with_its_snapshot() {
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(100)));
    for _ in 0..5 {
        engine.tick(None, None, false);
    }
    let asked = engine.tick(
        Some(Apply {
            refresh: true,
            ..Apply::default()
        }),
        None,
        false,
    );
    let id = asked.requests.snapshot.expect("a snapshot request");
    // An older snapshot, with a new reading: only an update, the strip keeps moving.
    let old = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&usage(101)),
            answers: id - 1,
        }),
        false,
    );
    assert_ne!(old.frame, Some(home(1, &usage(101))));
    let reset = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&usage(102)),
            answers: id,
        }),
        false,
    );
    assert_eq!(reset.frame, Some(home(1, &usage(102))));
}

// Another GPU waits for a snapshot of it, so the first frame after the switch never shows the old
// GPU's values (Phase 7 plan, Task 6).
#[test]
fn another_gpu_resets_with_the_new_gpu_s_values() {
    let gpu = |usage: i64| {
        readings(&[
            ("CPU Usage", Some(42)),
            ("Gpu usage", Some(usage)),
            ("Memory Load", Some(85)),
        ])
    };
    let mode2 = Settings {
        mode: 2,
        ..Settings::default()
    };
    let mut engine = Engine::new(mode2.clone(), snapshot(&gpu(67)));
    engine.tick(None, None, false);
    let switch = engine.tick(
        Some(Apply {
            settings: Some(Settings {
                gpu_choice: Some("0000:0b:00.0".to_owned()),
                ..mode2
            }),
            ..Apply::default()
        }),
        None,
        false,
    );
    let request = switch.requests.snapshot.expect("a snapshot of the new GPU");
    let choices = switch
        .requests
        .choices
        .expect("the new choice for the sensor thread");
    assert_eq!(choices.gpu.as_deref(), Some("0000:0b:00.0"));
    let next = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&gpu(12)),
            answers: request,
        }),
        false,
    );
    assert_eq!(next.frame, Some(home(2, &gpu(12))));
}

fn with(cpu: Reading, gpu: Reading) -> Snapshot {
    Snapshot::new(
        |metric| match metric {
            Metric::CpuUsage => cpu.clone(),
            Metric::GpuUsage | Metric::GpuTemperature | Metric::GpuPower => gpu.clone(),
            _ => Reading::Value(Exact::whole(50)),
        },
        Fan::NotChosen,
        Fan::NotChosen,
    )
}

fn value(n: i64) -> Reading {
    Reading::Value(Exact::whole(n))
}

/// The modes among `allowed`, which the readings let the service show, that this build draws:
/// Phase 8 draws modes 7–12 one commit at a time.
fn drawn_among(allowed: &[u32]) -> Vec<u32> {
    allowed
        .iter()
        .copied()
        .filter(|&m| u16::try_from(m).ok().and_then(Mode::new).is_some_and(draws))
        .collect()
}

// P6-04: a metric with no device stays available for the first two minutes after start-up, while
// the registry still looks for late drivers; after that its modes leave AvailableModes, and come
// back once it's found.
#[test]
fn late_sources_keep_their_modes_for_two_minutes() {
    let no_gpu = with(value(42), Reading::Unavailable(Unavailable::NoDevice));
    let mut engine = Engine::new(Settings::default(), no_gpu.clone());
    for _ in 0..300 {
        engine.tick(None, None, false);
    }
    let early = engine.availability();
    for mode in [2, 4, 6] {
        assert!(early.modes.contains(&mode), "mode {mode}: {early:?}");
    }
    // Found 30 s after start: it never left.
    let found = Published {
        snapshot: with(value(42), value(67)),
        answers: 0,
    };
    let mut engine2 = Engine::new(Settings::default(), no_gpu.clone());
    for t in 0..LATE_SOURCE_TICKS + 100 {
        let p = (t == 300).then(|| found.clone());
        engine2.tick(None, p, false);
        assert!(engine2.availability().modes.contains(&2), "tick {t}");
    }
    // Still missing after two minutes: modes 2, 4 and 6 go, with the reason; found later, back.
    for _ in 300..LATE_SOURCE_TICKS {
        engine.tick(None, None, false);
    }
    let late = engine.availability();
    for mode in [2, 4, 6] {
        assert!(!late.modes.contains(&mode), "mode {mode}");
        assert!(late.reasons[&mode].contains("no device"), "{late:?}");
    }
    assert!(late.modes.contains(&1) && late.modes.contains(&3) && late.modes.contains(&5));
    engine.tick(None, Some(found), false);
    assert!(engine.availability().modes.contains(&4));
    // A wake-up starts the two minutes again.
    engine.tick(
        Some(Apply {
            wake: true,
            ..Apply::default()
        }),
        Some(Published {
            snapshot: no_gpu,
            answers: 0,
        }),
        false,
    );
    assert!(engine.availability().modes.contains(&2), "after a wake-up");
}

// A GPU metric losing its device mid-run: dashes, the mode stays, and its modes leave
// AvailableModes once the late-source window is over; a lasting reason counts at once.
#[test]
fn a_reading_lost_mid_run_draws_dashes_and_the_mode_stays() {
    let mode4 = Settings {
        mode: 4,
        ..Settings::default()
    };
    let mut engine = Engine::new(mode4, with(value(42), value(67)));
    for _ in 0..LATE_SOURCE_TICKS {
        engine.tick(None, None, false);
    }
    let lost = with(value(42), Reading::Unavailable(Unavailable::NoDevice));
    let out = engine.tick(
        None,
        Some(Published {
            snapshot: lost.clone(),
            answers: 0,
        }),
        false,
    );
    assert_eq!(engine.mode().number(), 4);
    let dashes = readings(&[("CPU Usage", Some(42)), ("Gpu usage", None)]);
    let want = Engine::new(
        Settings {
            mode: 4,
            ..Settings::default()
        },
        snapshot(&dashes),
    )
    .tick(None, None, false)
    .frame;
    assert_eq!(out.frame, want, "42% over dashes");
    // Every mode without a GPU metric: `with` gives the GPU's temperature and power the same
    // reading as its usage.
    let modes = engine.availability().modes;
    assert_eq!(modes, drawn_among(&[1, 3, 5, 7, 10]));
    // A reading the PC may not give at all counts at once, settled or not.
    let denied = with(
        Reading::Unavailable(Unavailable::PermissionDenied {
            path: "/sys/fake".into(),
        }),
        value(67),
    );
    let early = availability(&denied, false);
    assert_eq!(early.modes, drawn_among(&[2, 3, 6, 7, 8, 9, 10, 11, 12]));
}

// P8-03: only modes this build draws are ever available, whatever the readings.
#[test]
fn only_modes_this_build_draws_are_available() {
    let all = with(value(42), value(67));
    for settled in [false, true] {
        let a = availability(&all, settled);
        let every: Vec<u32> = (1..=12).collect();
        assert_eq!(a.modes, drawn_among(&every));
        assert!(a.modes.starts_with(&[1, 2, 3, 4, 5, 6, 7]));
        assert!(!a.modes.contains(&13));
        assert_eq!(a.reasons[&13], "comes in a later version");
    }
}

// Phase 8 plan, Task 4: a CPU whose driver has no temperature rule (k10temp before family 17h,
// for example) takes mode 7 out of AvailableModes with its reason, settled or not; the other
// modes stay.
#[test]
fn mode_7_leaves_available_modes_without_a_cpu_temperature_rule() {
    let no_rule = Snapshot::new(
        |metric| match metric {
            Metric::CpuTemperature => Reading::Unavailable(Unavailable::NoRule {
                driver: "made-up-temp".to_owned(),
            }),
            _ => value(50),
        },
        Fan::NotChosen,
        Fan::NotChosen,
    );
    for settled in [false, true] {
        let a = availability(&no_rule, settled);
        assert!(!a.modes.contains(&7), "{:?}", a.modes);
        assert_eq!(a.reasons[&7], "CPU Temperature: no reading for this driver");
        assert!(a.modes.starts_with(&[1, 2, 3, 4, 5, 6]));
    }
}

// Phase 8 plan, Task 5: mode 8 leaves AvailableModes when no GPU is found once the first two
// minutes are over, and at once when its driver has no temperature rule, each with its reason.
#[test]
fn mode_8_leaves_available_modes_without_a_gpu_or_a_gpu_temperature_rule() {
    let gpu = |reading: Reading| {
        Snapshot::new(
            |metric| match metric {
                Metric::GpuTemperature => reading.clone(),
                _ => value(50),
            },
            Fan::NotChosen,
            Fan::NotChosen,
        )
    };
    let no_gpu = gpu(Reading::Unavailable(Unavailable::NoDevice));
    assert!(
        availability(&no_gpu, false).modes.contains(&8),
        "still looked for"
    );
    let settled = availability(&no_gpu, true);
    assert!(!settled.modes.contains(&8), "{:?}", settled.modes);
    assert_eq!(settled.reasons[&8], "Gpu temperature: no device found");
    let no_rule = gpu(Reading::Unavailable(Unavailable::NoRule {
        driver: "made-up-gpu".to_owned(),
    }));
    for settled in [false, true] {
        let a = availability(&no_rule, settled);
        assert!(!a.modes.contains(&8), "{:?}", a.modes);
        assert_eq!(a.reasons[&8], "Gpu temperature: no reading for this driver");
        assert!(a.modes.contains(&7), "the CPU's mode stays");
    }
}

// The first second of a reading averaged over the interval draws dashes, then digits.
#[test]
fn not_yet_draws_dashes_then_digits() {
    let first = with(Reading::Unavailable(Unavailable::NotYet), value(67));
    let mut engine = Engine::new(Settings::default(), first);
    let dashes = engine.tick(None, None, false).frame;
    assert_eq!(dashes, Some(home(1, &readings(&[("CPU Usage", None)]))));
    let every: Vec<u32> = (1..=12).collect();
    assert_eq!(
        engine.availability().modes,
        drawn_among(&every),
        "NotYet passes"
    );
    let digits = engine
        .tick(
            None,
            Some(Published {
                snapshot: with(value(42), value(67)),
                answers: 0,
            }),
            false,
        )
        .frame;
    assert_eq!(digits, Some(home(1, &usage(42))));
}
