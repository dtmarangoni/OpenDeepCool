//! Layer 3, the engine's tick logic (Phase 7 plan, Task 6): every scenario of
//! `docs/spec/frames/scenarios/`, through `Engine`, which turns each
//! frame itself, in all four rotations; the newest settings; Refresh and another GPU waiting for
//! their snapshot; availability with late sources; dashes. From Phase 9 the replay plays each
//! scenario's sound and Music settings (Task 3). The threads and the clock are tested in
//! `runtime.rs`.

use std::collections::HashMap;

use std::sync::{Arc, OnceLock};

use odc_protocol::Rotation;

use odc_render::{COLUMNS, Mode, WINDOW, analyse};

use odc_sensors::{Exact, Metric, Reading, Snapshot, Unavailable};

use odc_service::audio::{AudioSource, FakeAudio};

use odc_service::engine::{Apply, Engine, LATE_SOURCE_TICKS, Override, Published, availability};

use odc_service::settings::{Fall, Settings, TemperatureUnit};

use odc_testkit::diff;

use odc_testkit::expected::{Change, Reset, Scenario, Unit, every_metric, mode_scenarios};

use odc_testkit::spectrum::signals;

/// A snapshot with the readings named and every other metric's filler; an unavailable reading is
/// `NotYet`, which shows dashes.
fn snapshot(readings: &[(String, Option<Exact>)]) -> Snapshot {
    let all = every_metric(readings);
    Snapshot::new(|metric| {
        let (_, value) = all
            .iter()
            .find(|(m, _)| *m == metric)
            .expect("every metric");
        match value {
            Some(v) => Reading::Value(*v),
            None => Reading::Unavailable(Unavailable::NotYet),
        }
    })
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

/// Replays `scenario` through the engine from rotation `base`, comparing every frame: the sound
/// plays from the tick that names it, one window a tick, then silence.
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
    let audio = Arc::new(FakeAudio::new());
    let mut engine = Engine::with_audio(
        start,
        snapshot(&readings),
        Arc::clone(&audio) as Arc<dyn AudioSource>,
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

/// Refresh, with the D-Bus side's snapshot request `asked`.
fn refresh(asked: u64) -> Apply {
    Apply {
        refresh: true,
        asked: Some(asked),
        ..Apply::default()
    }
}

fn with(cpu: Reading, gpu: Reading) -> Snapshot {
    Snapshot::new(|metric| match metric {
        Metric::CpuUsage => cpu.clone(),
        Metric::GpuUsage | Metric::GpuTemperature | Metric::GpuPower => gpu.clone(),
        _ => Reading::Value(Exact::whole(50)),
    })
}

fn value(n: i64) -> Reading {
    Reading::Value(Exact::whole(n))
}

/// Every scenario of a Music mode's file through the engine, in all four rotations: the spectrum from each scenario's sound, the Music settings, the emoji loops and the
/// Dancer, and the resets, against the oracle's frames.
fn music_scenarios_play_out(mode: u16) {
    let scenarios = mode_scenarios(mode);
    assert!(!scenarios.is_empty(), "mode {mode}: no scenario");
    for scenario in &scenarios {
        for base in 0..4 {
            replay(scenario, base);
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
    every_scenario_of_mode_18_plays_out_through_the_engine_with_the_test_switch: 18,
    every_scenario_of_mode_19_plays_out_through_the_engine_with_the_test_switch: 19,
    every_scenario_of_mode_20_plays_out_through_the_engine_with_the_test_switch: 20,
    every_scenario_of_mode_21_plays_out_through_the_engine_with_the_test_switch: 21,
    every_scenario_of_mode_22_plays_out_through_the_engine_with_the_test_switch: 22,
    every_scenario_of_mode_23_plays_out_through_the_engine_with_the_test_switch: 23,
    every_scenario_of_mode_24_plays_out_through_the_engine_with_the_test_switch: 24,
    every_scenario_of_mode_25_plays_out_through_the_engine_with_the_test_switch: 25,
    every_scenario_of_mode_26_plays_out_through_the_engine_with_the_test_switch: 26,
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

mod availability;
mod music;
mod overrides;
mod refresh;
mod scenarios;
