//! Layer 3, the engine's tick logic (Phase 7 plan, Task 6): every scenario of
//! `docs/spec/frames/scenarios/` whose modes this build draws, through `Engine`, which turns each
//! frame itself, in all four rotations; the newest settings; Refresh and another GPU waiting for
//! their snapshot; availability with late sources; dashes. From Phase 9 the replay plays each
//! scenario's sound and Music settings (Task 3). The threads and the clock are tested in
//! `runtime.rs`.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use odc_protocol::Rotation;
use odc_render::{Mode, WINDOW, draws};
use odc_sensors::{Exact, Fan, Metric, Reading, Snapshot, Unavailable};
use odc_service::audio::{AudioSource, FakeAudio};
use odc_service::engine::{
    Apply, Drawn, Engine, LATE_SOURCE_TICKS, Override, Published, availability,
};
use odc_service::settings::{Fall, Settings, TemperatureUnit};
use odc_testkit::diff;
use odc_testkit::expected::{Change, Reset, Scenario, Unit, every_metric, mode_scenarios};
use odc_testkit::spectrum::signals;

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

/// Every signal of `spectrum/signals.txt` by its name, as each tick's window.
fn windows() -> &'static HashMap<String, Vec<[f32; WINDOW]>> {
    static WINDOWS: OnceLock<HashMap<String, Vec<[f32; WINDOW]>>> = OnceLock::new();
    WINDOWS.get_or_init(|| {
        signals()
            .into_iter()
            .map(|s| (s.name.clone(), s.windows()))
            .collect()
    })
}

/// A falloff stop of the scenario files, by its name.
fn fall(name: &str) -> Fall {
    Fall::from_name(name).unwrap_or_else(|| panic!("{name:?} isn't a stop"))
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
    replay_drawing(scenario, base, draws);
}

/// [`replay`], with the engine drawing the modes `drawn` lets it: the sound plays from the tick
/// that names it, one window a tick, then silence.
fn replay_drawing(scenario: &Scenario, base: u8, drawn: Drawn) {
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
    let audio = Arc::new(FakeAudio::new());
    let mut engine = Engine::with_audio(
        start,
        snapshot(&readings),
        Arc::clone(&audio) as Arc<dyn AudioSource>,
        drawn,
    );
    let mut gpus = 0;
    // The signal playing and the tick it started at.
    let mut playing: Option<(&[[f32; WINDOW]], u32)> = None;
    for tick in &scenario.ticks {
        for change in &tick.changes {
            if let Change::Audio(name) = change {
                let signal = windows()
                    .get(name)
                    .unwrap_or_else(|| panic!("{}: no signal {name:?}", scenario.heading));
                playing = Some((signal, tick.n));
            }
        }
        let window = playing.and_then(|(signal, from)| {
            usize::try_from(tick.n - from)
                .ok()
                .and_then(|i| signal.get(i))
        });
        audio.set(window.copied().unwrap_or([0.0; WINDOW]));
        let mut settings = engine.settings().clone();
        let (mut set, mut refresh, mut wake, mut missed, mut read) =
            (false, false, false, false, false);
        let mut gpus_asked = false;
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
                        gpus_asked = true;
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
                    // goings are the device thread's; the sound is the source's, above.
                    Change::Refused { .. }
                    | Change::CoolerAway
                    | Change::CoolerBack
                    | Change::Audio(_) => {}
                    // A Music setting is a settings change, never a reset (music.md point 7).
                    Change::MusicPeaks(on) => {
                        settings.music_peaks = *on;
                        set = true;
                    }
                    Change::MusicBarFall(stop) => {
                        settings.music_bar_fall = fall(stop);
                        set = true;
                    }
                    Change::MusicPeakFall(stop) => {
                        settings.music_peak_fall = fall(stop);
                        set = true;
                    }
                }
            }
        }
        // Refresh's and another GPU's reset waits for a snapshot answering the D-Bus side's request,
        // which it sends as the call arrives (owner, 2026-11-06): the answer is there by the next
        // tick, with the readings unchanged when the tick gives none.
        let apply = (set || refresh || wake).then(|| Apply {
            settings: set.then(|| settings.clone()),
            refresh,
            asked: (refresh || gpus_asked).then_some(u64::from(tick.n) + 1),
            wake,
            readings: None,
        });
        let published = (read || refresh || gpus_asked).then(|| fresh(&readings));
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
// which only the engine can play out. From Phase 9 also those of each Music mode drawn so far,
// with their sound and Music settings, and the switches into it from modes 1–12.
#[test]
fn every_scenario_of_the_drawn_modes_plays_out_through_the_engine_in_all_four_rotations() {
    let (mut replayed, mut skipped) = (0, 0);
    for mode in 1..=23 {
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
    assert_eq!(
        replayed + skipped,
        10_403 + 15_054,
        "the scenarios of modes 1–12 and 13–23"
    );
    assert!(replayed >= 10_403, "Phase 8's modes 1–12 at least");
    if Mode::all().all(draws) {
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
    engine.tick(Some(refresh(1)), None, false);
    let answered = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&usage(44)),
            answers: 1,
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

/// Refresh, with the D-Bus side's snapshot request `asked`.
fn refresh(asked: u64) -> Apply {
    Apply {
        refresh: true,
        asked: Some(asked),
        ..Apply::default()
    }
}

// P7-06: Refresh's reset lands on the first tick that has the fresh snapshot, never on an older
// one that arrives meanwhile: when its answer is late, a tick or more after the Refresh.
#[test]
fn refresh_resets_on_the_first_tick_with_its_snapshot() {
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(100)));
    for _ in 0..5 {
        engine.tick(None, None, false);
    }
    let taken = engine.tick(Some(refresh(2)), None, false);
    assert_ne!(taken.frame, Some(home(1, &usage(100))), "no answer yet");
    // An older snapshot, with a new reading: only an update, the strip keeps moving.
    let old = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&usage(101)),
            answers: 1,
        }),
        false,
    );
    assert_ne!(old.frame, Some(home(1, &usage(101))));
    let reset = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&usage(102)),
            answers: 2,
        }),
        false,
    );
    assert_eq!(reset.frame, Some(home(1, &usage(102))));
}

// S26 (owner, 2026-11-06): the D-Bus side asks for the snapshot as the call arrives, so its answer
// normally comes before the tick that takes the Refresh, which resets: the next frame is home.
// An answer the engine took as a plain snapshot a tick before the Refresh reached it counts too.
#[test]
fn refresh_resets_on_the_tick_that_takes_it_when_its_answer_came_first() {
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(100)));
    for _ in 0..5 {
        engine.tick(None, None, false);
    }
    let answer = |answers, n| Published {
        snapshot: snapshot(&usage(n)),
        answers,
    };
    let same = engine.tick(Some(refresh(1)), Some(answer(1, 101)), false);
    assert_eq!(same.frame, Some(home(1, &usage(101))));
    for _ in 0..5 {
        engine.tick(None, None, false);
    }
    let first = engine.tick(None, Some(answer(2, 102)), false);
    assert_ne!(first.frame, Some(home(1, &usage(102))), "only an update");
    let taken = engine.tick(Some(refresh(2)), None, false);
    assert_eq!(taken.frame, Some(home(1, &usage(102))));
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
    let choose = |pci: &str, asked| Apply {
        settings: Some(Settings {
            gpu_choice: Some(pci.to_owned()),
            ..mode2.clone()
        }),
        asked: Some(asked),
        ..Apply::default()
    };
    let answer = |answers, usage| Published {
        snapshot: snapshot(&gpu(usage)),
        answers,
    };
    let mut engine = Engine::new(mode2.clone(), snapshot(&gpu(67)));
    engine.tick(None, None, false);
    // Its answer late: no reset until it comes.
    let switch = engine.tick(Some(choose("0000:0b:00.0", 1)), None, false);
    assert_eq!(switch.frame, Some(home(2, &gpu(67))));
    let next = engine.tick(None, Some(answer(1, 12)), false);
    assert_eq!(next.frame, Some(home(2, &gpu(12))));
    // Its answer on time, as the D-Bus side asks at once (owner, 2026-11-06): the next frame.
    let mut engine = Engine::new(mode2.clone(), snapshot(&gpu(100)));
    for _ in 0..5 {
        engine.tick(None, None, false);
    }
    let switch = engine.tick(Some(choose("0000:0b:00.0", 1)), Some(answer(1, 101)), false);
    assert_eq!(switch.frame, Some(home(2, &gpu(101))));
    // P7-15: another GPU and back before a tick is no change: no reset, though answered.
    let at_b = || {
        let mut engine = Engine::new(
            Settings {
                gpu_choice: Some("0000:0b:00.0".to_owned()),
                ..mode2.clone()
            },
            snapshot(&gpu(100)),
        );
        for _ in 0..5 {
            engine.tick(None, None, false);
        }
        engine
    };
    let (mut changed, mut alone) = (at_b(), at_b());
    let back = changed.tick(Some(choose("0000:0b:00.0", 2)), Some(answer(2, 101)), false);
    assert_ne!(back.frame, Some(home(2, &gpu(101))), "no reset");
    assert_eq!(
        back.frame,
        alone.tick(None, Some(answer(2, 101)), false).frame
    );
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
    assert_eq!(
        modes,
        drawn_among(&[1, 3, 5, 7, 10, 13, 14, 15, 16, 17, 18, 20, 22, 23])
    );
    // A reading the PC may not give at all counts at once, settled or not.
    let denied = with(
        Reading::Unavailable(Unavailable::PermissionDenied {
            path: "/sys/fake".into(),
        }),
        value(67),
    );
    let early = availability(&denied, false);
    assert_eq!(
        early.modes,
        drawn_among(&[
            2, 3, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 21, 22, 23
        ])
    );
}

// P8-03: only modes this build draws are ever available, whatever the readings.
#[test]
fn only_modes_this_build_draws_are_available() {
    let all = with(value(42), value(67));
    for settled in [false, true] {
        let a = availability(&all, settled);
        let every: Vec<u32> = (1..=23).collect();
        assert_eq!(a.modes, drawn_among(&every));
        assert!(a.modes.starts_with(&[1, 2, 3, 4, 5, 6, 7]));
        // The first mode a later commit draws, while there is one.
        if let Some(later) = Mode::all().find(|&m| !draws(m)) {
            let later = u32::from(later.number());
            assert!(!a.modes.contains(&later));
            assert_eq!(a.reasons[&later], "comes in a later version");
        }
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

// Phase 8 plan, Task 6: mode 9 leaves AvailableModes when either temperature has no rule, with
// the first one's reason, top first.
#[test]
fn mode_9_leaves_available_modes_when_either_temperature_has_no_rule() {
    let no_rule = |driver: &str| {
        Reading::Unavailable(Unavailable::NoRule {
            driver: driver.to_owned(),
        })
    };
    let temperatures = |cpu: Reading, gpu: Reading| {
        Snapshot::new(
            |metric| match metric {
                Metric::CpuTemperature => cpu.clone(),
                Metric::GpuTemperature => gpu.clone(),
                _ => value(50),
            },
            Fan::NotChosen,
            Fan::NotChosen,
        )
    };
    let cases = [
        (no_rule("made-up-temp"), value(38), "CPU Temperature"),
        (value(57), no_rule("made-up-gpu"), "Gpu temperature"),
        (
            no_rule("made-up-temp"),
            no_rule("made-up-gpu"),
            "CPU Temperature",
        ),
    ];
    for (cpu, gpu, first) in cases {
        let a = availability(&temperatures(cpu, gpu), true);
        assert!(!a.modes.contains(&9), "{:?}", a.modes);
        assert_eq!(
            a.reasons[&9],
            format!("{first}: no reading for this driver")
        );
    }
    assert!(
        availability(&temperatures(value(57), value(38)), true)
            .modes
            .contains(&9)
    );
}

// Phase 8 plan, Task 7: without the udev rule that lets the service read the CPU's energy
// counter, mode 10 leaves AvailableModes at once, with its reason; it's never drawn as 0 W.
#[test]
fn mode_10_leaves_available_modes_when_the_cpu_power_counter_is_denied() {
    let denied = Snapshot::new(
        |metric| match metric {
            Metric::CpuPower => Reading::Unavailable(Unavailable::PermissionDenied {
                path: "/sys/fake/energy_uj".into(),
            }),
            _ => value(50),
        },
        Fan::NotChosen,
        Fan::NotChosen,
    );
    for settled in [false, true] {
        let a = availability(&denied, settled);
        assert!(!a.modes.contains(&10), "{:?}", a.modes);
        assert_eq!(a.reasons[&10], "CPU Power: no permission to read it");
        assert!(a.modes.contains(&7), "the other CPU modes stay");
    }
}

// Phase 8 plan, Task 8 (owner, 2026-10-27; solution review P8-06): mode 11 leaves AvailableModes
// on integrated AMD graphics, on an AMD GPU of unknown kind (an unranked APU, as Phase 6's sensor
// fixtures read it), on a driver without a power rule, and once no GPU is found after the first
// two minutes, each with its reason; the CPU's power mode stays.
#[test]
fn mode_11_leaves_available_modes_when_the_gpu_power_cant_be_shown() {
    let gpu_power = |reading: Reading| {
        Snapshot::new(
            |metric| match metric {
                Metric::GpuPower => reading.clone(),
                _ => value(50),
            },
            Fan::NotChosen,
            Fan::NotChosen,
        )
    };
    let lasting = [
        (
            Unavailable::IntegratedGpuPower,
            "an integrated GPU's power includes the CPU's",
        ),
        (
            Unavailable::UnknownGpuKind,
            "the GPU's kind is unknown, so its power might include the CPU's",
        ),
        (
            Unavailable::NoRule {
                driver: "made-up-gpu".to_owned(),
            },
            "no reading for this driver",
        ),
    ];
    for (reason, text) in lasting {
        for settled in [false, true] {
            let a = availability(&gpu_power(Reading::Unavailable(reason.clone())), settled);
            assert!(!a.modes.contains(&11), "{reason:?}: {:?}", a.modes);
            assert_eq!(a.reasons[&11], format!("Gpu power: {text}"));
            assert!(a.modes.contains(&10), "the CPU's power stays");
        }
    }
    let no_gpu = gpu_power(Reading::Unavailable(Unavailable::NoDevice));
    assert!(
        availability(&no_gpu, false).modes.contains(&11),
        "still looked for"
    );
    let settled = availability(&no_gpu, true);
    assert!(!settled.modes.contains(&11));
    assert_eq!(settled.reasons[&11], "Gpu power: no device found");
}

// Phase 8 plan, Task 8: mode 12 leaves AvailableModes when either power can't be read for good,
// with the first one's reason, top first; the unknown-kind APU takes it out with mode 11.
#[test]
fn mode_12_leaves_available_modes_when_either_power_cant_be_shown() {
    let powers = |cpu: Reading, gpu: Reading| {
        Snapshot::new(
            |metric| match metric {
                Metric::CpuPower => cpu.clone(),
                Metric::GpuPower => gpu.clone(),
                _ => value(50),
            },
            Fan::NotChosen,
            Fan::NotChosen,
        )
    };
    let denied = || {
        Reading::Unavailable(Unavailable::PermissionDenied {
            path: "/sys/fake/energy_uj".into(),
        })
    };
    let unknown = || Reading::Unavailable(Unavailable::UnknownGpuKind);
    let cases = [
        (denied(), value(250), "CPU Power: no permission to read it"),
        (
            value(65),
            unknown(),
            "Gpu power: the GPU's kind is unknown, so its power might include the CPU's",
        ),
        (denied(), unknown(), "CPU Power: no permission to read it"),
    ];
    for (cpu, gpu, reason) in cases {
        for settled in [false, true] {
            let a = availability(&powers(cpu.clone(), gpu.clone()), settled);
            assert!(!a.modes.contains(&12), "{:?}", a.modes);
            assert_eq!(a.reasons[&12], reason);
        }
    }
    let apu = availability(&powers(value(65), unknown()), true);
    assert!(!apu.modes.contains(&11) && !apu.modes.contains(&12));
    assert!(apu.modes.contains(&10), "the CPU's power stays");
    assert!(
        availability(&powers(value(65), value(250)), true)
            .modes
            .contains(&12)
    );
}

// The first second of a reading averaged over the interval draws dashes, then digits.
#[test]
fn not_yet_draws_dashes_then_digits() {
    let first = with(Reading::Unavailable(Unavailable::NotYet), value(67));
    let mut engine = Engine::new(Settings::default(), first);
    let dashes = engine.tick(None, None, false).frame;
    assert_eq!(dashes, Some(home(1, &readings(&[("CPU Usage", None)]))));
    let every: Vec<u32> = (1..=23).collect();
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

/// The test-only switch (Phase 9 plan, Task 3): every mode drawn, the Music modes included, before
/// each one's own commit makes it drawable (Tasks 6–9).
fn every_mode(_: Mode) -> bool {
    true
}

/// Every scenario of a Music mode's file through the engine with the test-only switch, in all four
/// rotations: the spectrum from each scenario's sound, the Music settings, the emoji loops and the
/// Dancer, and the resets, against the oracle's frames.
fn music_scenarios_play_out(mode: u16) {
    let scenarios = mode_scenarios(mode);
    assert!(!scenarios.is_empty(), "mode {mode}: no scenario");
    for scenario in &scenarios {
        for base in 0..4 {
            replay_drawing(scenario, base, every_mode);
        }
    }
}

macro_rules! music_scenarios {
    ($($name:ident: $mode:literal),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                music_scenarios_play_out($mode);
            }
        )+
    };
}

music_scenarios! {
    every_scenario_of_mode_13_plays_out_through_the_engine_with_the_test_switch: 13,
    every_scenario_of_mode_14_plays_out_through_the_engine_with_the_test_switch: 14,
    every_scenario_of_mode_15_plays_out_through_the_engine_with_the_test_switch: 15,
    every_scenario_of_mode_16_plays_out_through_the_engine_with_the_test_switch: 16,
    every_scenario_of_mode_17_plays_out_through_the_engine_with_the_test_switch: 17,
    every_scenario_of_mode_18_plays_out_through_the_engine_with_the_test_switch: 18,
    every_scenario_of_mode_19_plays_out_through_the_engine_with_the_test_switch: 19,
    every_scenario_of_mode_20_plays_out_through_the_engine_with_the_test_switch: 20,
    every_scenario_of_mode_21_plays_out_through_the_engine_with_the_test_switch: 21,
    every_scenario_of_mode_22_plays_out_through_the_engine_with_the_test_switch: 22,
    every_scenario_of_mode_23_plays_out_through_the_engine_with_the_test_switch: 23,
}

/// The lit heights of each column in a frame's bottom `h` rows, from the floor, checked against
/// the layout: a solid bar from the floor, then at most one dot. A dot right above its bar looks
/// like a taller bar, so columns are compared by their lit heights.
fn spectrum_rows(frame: &odc_protocol::Frame, h: u8) -> Vec<Vec<u8>> {
    (1..=14)
        .map(|c| {
            let lit: Vec<u8> = (1..=h)
                .filter(|&height| frame.get(15 - usize::from(height), c))
                .collect();
            let bar = lit
                .iter()
                .enumerate()
                .take_while(|&(i, &height)| usize::from(height) == i + 1)
                .count();
            assert!(bar >= 1, "column {c}: the floor is lit");
            assert!(lit.len() <= bar + 1, "column {c}: one dot at most, {lit:?}");
            lit
        })
        .collect()
}

/// The lit heights `expected.txt`'s rows give each column: its bar, and its dot.
fn expected_rows(rows: &odc_testkit::spectrum::Rows) -> Vec<Vec<u8>> {
    let dots = rows.dots.unwrap_or([None; 14]);
    rows.bars
        .iter()
        .zip(dots)
        .map(|(&bar, dot)| (1..=bar).chain(dot).collect())
        .collect()
}

// PRD layer 3, "audio file → frames" (Phase 9 plan, Task 3, written against the test-only
// switch): every signal of `spectrum/signals.txt` through the engine, at H 12 in mode 13 and H 6
// in mode 14 with the default Music settings; the frames' spectrum rows are `expected.txt`'s,
// tick for tick.
#[test]
fn every_signal_through_the_engine_draws_its_expected_rows() {
    let sections = odc_testkit::spectrum::expected();
    let mut checked = 0;
    for (mode, h) in [(13, 12), (14, 6)] {
        let heading =
            |signal: &str| format!("{signal} · H {h} · peaks on · bars moderate · peaks slow");
        for signal in signals().iter().filter(|s| !s.is_beat_train()) {
            let want = sections
                .iter()
                .find(|e| e.heading == heading(&signal.name))
                .unwrap_or_else(|| panic!("no section {}", heading(&signal.name)));
            let audio = Arc::new(FakeAudio::new());
            let settings = Settings {
                mode,
                ..Settings::default()
            };
            let mut engine = Engine::with_audio(
                settings,
                snapshot(&usage(42)),
                Arc::clone(&audio) as Arc<dyn AudioSource>,
                every_mode,
            );
            for (n, (window, rows)) in signal.windows().iter().zip(&want.ticks).enumerate() {
                audio.set(*window);
                let frame = engine.tick(None, None, false).frame.expect("a frame");
                assert_eq!(
                    spectrum_rows(&frame, h),
                    expected_rows(rows),
                    "mode {mode}, {}, tick {n}",
                    signal.name
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 2 * 81);
}

// The Dancer through the engine (Phase 9 plan, Task 9's "audio file → frames", here against the
// test-only switch): every sequence of `spectrum/dancer.txt`, a reset being Refresh.
#[test]
fn every_dancer_sequence_through_the_engine_draws_its_poses() {
    let poses: Vec<odc_protocol::Frame> = odc_render::Pose::all()
        .map(odc_render::Pose::frame)
        .collect();
    let settings = Settings {
        mode: 23,
        ..Settings::default()
    };
    for dance in odc_testkit::spectrum::dances() {
        let audio = Arc::new(FakeAudio::new());
        let mut engine = Engine::with_audio(
            settings.clone(),
            snapshot(&usage(42)),
            Arc::clone(&audio) as Arc<dyn AudioSource>,
            every_mode,
        );
        for (n, (window, pose)) in windows()[&dance.signal]
            .iter()
            .zip(&dance.poses)
            .enumerate()
        {
            audio.set(*window);
            let reset = dance.resets.contains(&n);
            let apply = reset.then(|| Apply {
                refresh: true,
                asked: Some(n as u64 + 1),
                ..Apply::default()
            });
            let published = reset.then(|| fresh(&usage(42)));
            let frame = engine.tick(apply, published, false).frame.expect("a frame");
            assert_eq!(
                frame,
                poses[usize::from(*pose)],
                "{}, tick {n}",
                dance.heading
            );
        }
    }
}

// music.md point 8 (owner, 2026-11-05): a missed tick draws nothing and moves nothing; the next
// tick goes on from where the bars, the emoji loop and the Dancer were.
#[test]
fn a_missed_tick_in_music_moves_nothing() {
    // One tick of a tone at column 7's centre, then silence.
    let tone = windows()["burst train, column 7, -20 dBFS, 100 ms on, 400 ms off"][0];
    let sound: Vec<[f32; WINDOW]> = std::iter::once(tone)
        .chain(std::iter::repeat_n([0.0; WINDOW], 7))
        .collect();
    for mode in [13, 14, 17, 18, 23] {
        let settings = Settings {
            mode,
            ..Settings::default()
        };
        let engine = |audio: &Arc<FakeAudio>| {
            Engine::with_audio(
                settings.clone(),
                snapshot(&usage(42)),
                Arc::clone(audio) as Arc<dyn AudioSource>,
                every_mode,
            )
        };
        let (on_time, late) = (Arc::new(FakeAudio::new()), Arc::new(FakeAudio::new()));
        let (mut steady, mut missing) = (engine(&on_time), engine(&late));
        // The tone, then silence: the bars fall, the loops step.
        let mut frames = Vec::new();
        for window in &sound {
            on_time.set(*window);
            frames.push(steady.tick(None, None, false).frame.expect("a frame"));
        }
        for (n, window) in sound[..3].iter().enumerate() {
            late.set(*window);
            assert_eq!(missing.tick(None, None, false).frame, Some(frames[n]));
        }
        // Tick 3 is missed: each later tick of the late engine is the steady one's tick before, as
        // the sound is the same silence.
        late.set(sound[3]);
        assert_eq!(missing.tick(None, None, true).frame, None, "mode {mode}");
        for (n, window) in sound[4..8].iter().enumerate() {
            late.set(*window);
            let frame = missing.tick(None, None, false).frame;
            assert_eq!(frame, Some(frames[n + 3]), "mode {mode}, tick {}", n + 4);
        }
    }
}
