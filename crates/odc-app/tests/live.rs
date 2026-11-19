//! The live tiles' pictures (Phase 11a Task 14; `crates/odc-app/src/live.rs`), layer 1: every mode
//! of every combination, never a sample, against `odc_render`'s own drawing code, which draws the
//! cooler. No display and no D-Bus. Each metric has its own distinct, non-zero value in each case.

use std::collections::HashMap;

use odc_app::live::{
    Beat, Live, SAMPLE_BARS, SAMPLE_PEAKS, TICK_US, follows_unit, music_settings, sample_values,
    values_from,
};
use odc_ipc::FallStop;
use odc_protocol::Frame;
use odc_render::{
    COLUMNS, Dancer, EmojiLoop, Kind, Marquee, Mode, MusicSettings, Pose, Spectrum, Stop, Unit,
    Values, analyse, mode_info, picture, spectrum_height, value_picture, with_spectrum,
};
use odc_sensors::{Exact, Metric};
use odc_testkit::spectrum::signals;

const UNITS: [Unit; 2] = [Unit::Celsius, Unit::Fahrenheit];

fn music(peaks: bool) -> MusicSettings {
    MusicSettings {
        peaks,
        ..MusicSettings::default()
    }
}

/// Each metric its own whole value, `base` + 1 to `base` + 7: two digits for 10, three for 100.
fn distinct(base: i64) -> Values {
    Metric::ALL
        .into_iter()
        .zip(1..)
        .fold(Values::new(), |values, (metric, n)| {
            values.with(metric, Exact::whole(base + n))
        })
}

/// Fractions that round, each metric its own.
fn fractions() -> Values {
    Metric::ALL
        .into_iter()
        .zip(1..)
        .fold(Values::new(), |values, (metric, n)| {
            values.with(
                metric,
                Exact::new(20_000 + 1_377 * n, 1000).expect("a fraction"),
            )
        })
}

/// `distinct(10)` with one metric left without a value, for each metric; and no value at all.
fn with_gaps() -> Vec<Values> {
    let mut sets: Vec<Values> = Metric::ALL
        .into_iter()
        .map(|gap| {
            let mut values = distinct(10);
            values.set(gap, None);
            values
        })
        .collect();
    sets.push(Values::new());
    sets
}

/// Every set of values the matrix runs: two digits, three digits, fractions, each gap, none.
fn value_sets() -> Vec<Values> {
    let mut sets = vec![distinct(10), distinct(100), fractions(), sample_values()];
    sets.extend(with_gaps());
    sets
}

fn still(h: u8, peaks: bool) -> odc_render::Columns {
    Spectrum::at(SAMPLE_BARS, SAMPLE_PEAKS).draw(h, peaks)
}

/// A strip's picture, as the cooler draws it, in the modes that show a value.
fn strip(mode: Mode, values: &Values, unit: Unit) -> odc_render::Picture {
    match mode_info(mode).kind {
        Kind::Music => value_picture(mode, values, unit).expect("a value"),
        _ => picture(mode, values, unit).expect("drawn"),
    }
}

/// `top` over the still sample spectrum, in the modes that have one.
fn over_still(mode: Mode, top: Frame, peaks: bool) -> Frame {
    match spectrum_height(mode) {
        Some(h) => with_spectrum(&top, &still(h, peaks), h),
        None => top,
    }
}

/// What a tile shows on its first tick, at rest, built from `odc_render`'s pieces alone.
fn first_frame(mode: Mode, values: &Values, unit: Unit, peaks: bool) -> Frame {
    let top = match mode.number() {
        13 => Frame::blank(),
        14..=17 => EmojiLoop::of(mode).expect("a loop").tick(),
        23 => return Pose::STILL.frame(),
        _ => {
            let picture = strip(mode, values, unit);
            picture.frame_at(picture.home())
        }
    };
    over_still(mode, top, peaks)
}

fn frames(live: &Live) -> Vec<(Mode, Frame)> {
    Mode::all().map(|mode| (mode, *live.frame(mode))).collect()
}

/// `change` on `live`; the modes it said changed are exactly those whose frame differs, in the
/// order of the modes.
fn changing<T>(live: &mut Live, change: impl FnOnce(&mut Live) -> T) -> (T, Vec<Mode>) {
    let before = frames(live);
    let out = change(live);
    let after = frames(live);
    let differ: Vec<Mode> = before
        .iter()
        .zip(&after)
        .filter(|(b, a)| b.1 != a.1)
        .map(|(b, _)| b.0)
        .collect();
    (out, differ)
}

/// `live.tick()`, checking that its list of changed tiles is exactly the tiles that changed.
fn tick(live: &mut Live) -> Vec<Mode> {
    let (said, differ) = changing(live, |live| live.tick());
    assert_eq!(said, differ, "the modes a tick says changed");
    said
}

#[test]
fn a_fresh_tile_is_the_picture_odc_render_draws_at_home_in_every_combination() {
    let mut checked = 0;
    for values in value_sets() {
        for unit in UNITS {
            for peaks in [true, false] {
                let live = Live::new(unit, music(peaks), values.clone());
                for mode in Mode::all() {
                    assert_eq!(
                        *live.frame(mode),
                        first_frame(mode, &values, unit, peaks),
                        "mode {}, {unit:?}, peaks {peaks}, {values:?}",
                        mode.number()
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, value_sets().len() * 2 * 2 * 23);
}

#[test]
fn a_tile_shows_dashes_for_a_metric_with_no_value_and_no_other_does() {
    let (full, empty) = (distinct(10), Values::new());
    let (with, without) = (
        Live::new(Unit::Celsius, music(true), full),
        Live::new(Unit::Celsius, music(true), empty),
    );
    for mode in Mode::all() {
        let shows_a_value = !mode_info(mode).metrics.is_empty();
        let differs = with.frame(mode) != without.frame(mode);
        assert_eq!(
            differs,
            shows_a_value,
            "mode {}: its dashes differ from its value exactly when it shows one",
            mode.number()
        );
    }
}

#[test]
fn a_scrolling_tile_moves_a_column_each_tick_as_the_cooler_does() {
    for unit in UNITS {
        let values = distinct(100);
        let mut live = Live::new(unit, music(true), values.clone());
        let mut marquees: Vec<(Mode, Marquee)> = Mode::all()
            .filter(|mode| !matches!(mode.number(), 13..=17 | 23))
            .map(|mode| (mode, Marquee::new(strip(mode, &values, unit))))
            .collect();
        let mut moved = 0;
        for n in 0..70 {
            if n > 0 {
                tick(&mut live);
            }
            for (mode, marquee) in &mut marquees {
                let top = marquee.tick();
                assert_eq!(
                    *live.frame(*mode),
                    over_still(*mode, top, true),
                    "mode {}, {unit:?}, tick {n}",
                    mode.number()
                );
            }
            moved += 1;
        }
        assert_eq!(moved, 70);
        // Every value is 100 or more: each of these tiles scrolls, so its picture changes.
        let mut live = Live::new(unit, music(true), values);
        let changed = tick(&mut live);
        for (mode, _) in &marquees {
            assert!(
                changed.contains(mode),
                "mode {} scrolls, {unit:?}",
                mode.number()
            );
        }
    }
}

#[test]
fn a_static_tile_never_changes_and_only_the_emoji_loops_play() {
    let values = distinct(10);
    let mut live = Live::new(Unit::Celsius, music(true), values.clone());
    let mut loops: Vec<(Mode, EmojiLoop)> = (14..=17)
        .map(|n| Mode::new(n).expect("a mode"))
        .map(|mode| {
            let mut emoji = EmojiLoop::of(mode).expect("a loop");
            emoji.tick();
            (mode, emoji)
        })
        .collect();
    let first = frames(&live);
    for n in 1..=200 {
        let changed = tick(&mut live);
        for mode in &changed {
            assert!(
                (14..=17).contains(&mode.number()),
                "mode {} moved at tick {n}",
                mode.number()
            );
        }
        for (mode, emoji) in &mut loops {
            assert_eq!(
                *live.frame(*mode),
                over_still(*mode, emoji.tick(), true),
                "mode {} at tick {n}",
                mode.number()
            );
        }
    }
    for ((mode, before), (_, after)) in first.iter().zip(frames(&live)) {
        if !(14..=17).contains(&mode.number()) {
            assert_eq!(*before, after, "mode {} stayed still", mode.number());
        }
    }
}

#[test]
fn the_emoji_loops_start_together_and_start_again_at_home() {
    let mut live = Live::new(Unit::Celsius, music(true), sample_values());
    let start = frames(&live);
    // Until a loop's picture changes: they hold each picture for several ticks.
    let mut ticks = 0;
    while frames(&live) == start {
        tick(&mut live);
        ticks += 1;
        assert!(ticks < 300, "no loop moved in {ticks} ticks");
    }
    let (_, changed) = changing(&mut live, |live| live.home());
    // Home draws the first picture of every loop again, and every loop that had moved on.
    for mode in (14..=17).map(|n| Mode::new(n).expect("a mode")) {
        assert_eq!(*live.frame(mode), start[usize::from(mode.number()) - 1].1);
    }
    assert_eq!(frames(&live), start, "everything at home");
    assert!(!changed.is_empty());
}

#[test]
fn a_reading_that_changes_keeps_a_scrolling_tile_s_place() {
    let mode = Mode::new(1).expect("Load, CPU");
    let show = |n: i64| Values::new().with(Metric::CpuUsage, Exact::whole(n));
    for unit in UNITS {
        // Scrolling, then another number that scrolls, then one that doesn't, then one that does.
        let steps: [(usize, i64); 5] = [(7, 100), (3, 1234), (2, 5), (30, 7), (11, 999)];
        let mut live = Live::new(unit, music(true), show(120));
        let mut cooler = Marquee::new(strip(mode, &show(120), unit));
        let mut at = 0;
        assert_eq!(
            *live.frame(mode),
            over_still(mode, cooler.tick(), true),
            "tick 0"
        );
        for (ticks, value) in steps {
            live.set_values(show(value));
            cooler.update(strip(mode, &show(value), unit));
            for _ in 0..ticks {
                tick(&mut live);
                at += 1;
                assert_eq!(
                    *live.frame(mode),
                    over_still(mode, cooler.tick(), true),
                    "{unit:?}, tick {at}, after {value}"
                );
            }
        }
    }
}

#[test]
fn a_still_tile_is_centred_again_when_its_length_changes() {
    let mode = Mode::new(1).expect("Load, CPU");
    let show = |n: i64| Values::new().with(Metric::CpuUsage, Exact::whole(n));
    let mut live = Live::new(Unit::Celsius, music(true), show(9));
    for n in [10, 9, 99, 5] {
        live.set_values(show(n));
        tick(&mut live);
        let want = {
            let picture = strip(mode, &show(n), Unit::Celsius);
            picture.frame_at(picture.home())
        };
        assert_eq!(*live.frame(mode), want, "{n}");
    }
}

#[test]
fn a_unit_change_restarts_the_temperature_tiles_at_home_at_once_and_no_other() {
    // °F makes the temperatures 100 or more, so they scroll; the usage tiles scroll at 150.
    let values = distinct(10)
        .with(Metric::CpuTemperature, Exact::whole(57))
        .with(Metric::GpuTemperature, Exact::whole(60))
        .with(Metric::CpuUsage, Exact::whole(150));
    let mut live = Live::new(Unit::Fahrenheit, music(true), values.clone());
    for _ in 0..9 {
        tick(&mut live);
    }
    let mut usage = Marquee::new(strip(Mode::new(1).expect("a mode"), &values, Unit::Celsius));
    for _ in 0..10 {
        usage.tick();
    }
    let (said, differ) = changing(&mut live, |live| live.set_unit(Unit::Celsius));
    assert_eq!(said, differ, "the modes set_unit says changed");
    for mode in Mode::all() {
        if follows_unit(mode) {
            let picture = strip(mode, &values, Unit::Celsius);
            let home = over_still(mode, picture.frame_at(picture.home()), true);
            assert_eq!(*live.frame(mode), home, "mode {} at home", mode.number());
        }
    }
    // A tile that shows no temperature goes on as if nothing happened.
    assert!(
        !said.contains(&Mode::new(1).expect("a mode")),
        "mode 1 doesn't follow the unit"
    );
    tick(&mut live);
    assert_eq!(
        *live.frame(Mode::new(1).expect("a mode")),
        over_still(Mode::new(1).expect("a mode"), usage.tick(), true)
    );
    // The same unit again: nothing.
    assert_eq!(live.set_unit(Unit::Celsius), Vec::new());
}

#[test]
fn without_sound_the_music_tiles_show_the_still_sample_and_levels_change_nothing() {
    let mut live = Live::new(Unit::Celsius, music(true), sample_values());
    assert!(!live.sound());
    let before = frames(&live);
    let (said, differ) = changing(&mut live, |live| live.levels(&[0.9; COLUMNS]));
    assert_eq!((said, differ), (Vec::new(), Vec::new()), "levels, no sound");
    assert_eq!(frames(&live), before);
    // The Dancer stands, whatever the ticks.
    for _ in 0..30 {
        tick(&mut live);
        assert_eq!(
            *live.frame(Mode::new(23).expect("Dancer")),
            Pose::STILL.frame()
        );
    }
}

#[test]
fn the_peak_dots_show_and_hide_at_once_in_the_spectrum_tiles_only() {
    let mut live = Live::new(Unit::Celsius, music(true), sample_values());
    let (said, differ) = changing(&mut live, |live| live.set_music(music(false)));
    assert_eq!(said, differ);
    assert!(!differ.is_empty(), "the sample has peaks above its bars");
    for mode in &differ {
        assert!(
            spectrum_height(*mode).is_some(),
            "mode {} has no spectrum",
            mode.number()
        );
    }
    // Only the spectrum rows differ: every tile is its first frame, with the dots off in those.
    for mode in Mode::all() {
        let peaks = spectrum_height(mode).is_none();
        let want = first_frame(mode, &sample_values(), Unit::Celsius, peaks);
        assert_eq!(*live.frame(mode), want, "mode {}", mode.number());
    }
    assert_eq!(live.set_music(music(false)), Vec::new(), "no change");
}

/// The levels of a test signal of `signals.txt`, tick by tick, as the service analyses them.
fn levels_of(name_part: &str) -> Vec<[f64; COLUMNS]> {
    let signal = signals()
        .into_iter()
        .find(|s| s.name.contains(name_part))
        .unwrap_or_else(|| panic!("no signal with {name_part:?}"));
    signal.windows().iter().map(analyse).collect()
}

#[test]
fn with_sound_the_spectrum_and_the_dancer_follow_the_levels_as_on_the_cooler() {
    let mut checked = 0;
    for settings in [
        music(true),
        MusicSettings {
            peaks: false,
            bar_fall: Stop::Faster,
            peak_fall: Stop::Slower,
        },
    ] {
        for signal in signals().iter().filter(|s| !s.is_beat_train()).take(10) {
            let levels: Vec<[f64; COLUMNS]> = signal.windows().iter().map(analyse).collect();
            let mut live = Live::new(Unit::Celsius, settings, sample_values());
            // A tile set without sound, ticked in step: what the sound's end leaves.
            let mut quiet = Live::new(Unit::Celsius, settings, sample_values());
            let (said, differ) = changing(&mut live, |live| live.set_sound(true));
            assert_eq!(said, differ, "the modes set_sound says changed");
            assert!(live.sound());
            // The cooler's pieces, entered at rest.
            let mut spectrum = Spectrum::enter();
            let mut dancer = Dancer::new();
            let mut loops: Vec<(Mode, EmojiLoop)> = (14..=17)
                .map(|n| Mode::new(n).expect("a mode"))
                .map(|mode| {
                    let mut emoji = EmojiLoop::of(mode).expect("a loop");
                    emoji.tick();
                    (mode, emoji)
                })
                .collect();
            let values = sample_values();
            let mut strips: Vec<(Mode, Marquee)> = (18..=22)
                .map(|n| Mode::new(n).expect("a mode"))
                .map(|mode| {
                    let mut marquee = Marquee::new(strip(mode, &values, Unit::Celsius));
                    marquee.tick();
                    (mode, marquee)
                })
                .collect();
            for (n, levels) in levels.iter().enumerate() {
                tick(&mut live);
                quiet.tick();
                let (said, differ) = changing(&mut live, |live| live.levels(levels));
                assert_eq!(said, differ, "the modes levels() says changed");
                spectrum.update(levels, &settings);
                dancer.tick(levels);
                let columns = |h| spectrum.draw(h, settings.peaks);
                let want = |mode: Mode, top: Frame| {
                    let h = spectrum_height(mode).expect("a spectrum");
                    with_spectrum(&top, &columns(h), h)
                };
                assert_eq!(
                    *live.frame(Mode::new(13).expect("a mode")),
                    want(Mode::new(13).expect("a mode"), Frame::blank()),
                    "{}, mode 13, tick {n}",
                    signal.name
                );
                for (mode, emoji) in &mut loops {
                    assert_eq!(
                        *live.frame(*mode),
                        want(*mode, emoji.tick()),
                        "{}, mode {}, tick {n}",
                        signal.name,
                        mode.number()
                    );
                }
                for (mode, marquee) in &mut strips {
                    assert_eq!(
                        *live.frame(*mode),
                        want(*mode, marquee.tick()),
                        "{}, mode {}, tick {n}",
                        signal.name,
                        mode.number()
                    );
                }
                assert_eq!(
                    *live.frame(Mode::new(23).expect("Dancer")),
                    dancer.pose().frame(),
                    "{}, the Dancer, tick {n}",
                    signal.name
                );
                checked += 1;
            }
            // The sound stops: the still sample again, the Dancer standing.
            let (said, differ) = changing(&mut live, |live| live.set_sound(false));
            assert_eq!(said, differ, "the modes set_sound says changed");
            assert!(!live.sound());
            assert_eq!(frames(&live), frames(&quiet), "back to the still sample");
            assert_eq!(
                *live.frame(Mode::new(23).expect("Dancer")),
                Pose::STILL.frame()
            );
        }
    }
    assert!(checked > 100, "{checked} ticks compared");
    // Starting sound twice, or stopping it twice, is no change.
    let mut live = Live::new(Unit::Celsius, music(true), sample_values());
    assert_eq!(live.set_sound(false), Vec::new());
    live.set_sound(true);
    assert_eq!(live.set_sound(true), Vec::new());
}

#[test]
fn a_tick_of_levels_moves_the_dancer_through_its_poses_and_home_rests_everything() {
    let levels = levels_of("beat train");
    let mut live = Live::new(Unit::Celsius, music(true), sample_values());
    live.set_sound(true);
    let mut dancer = Dancer::new();
    let mut moved = false;
    let mode = Mode::new(23).expect("Dancer");
    for (n, levels) in levels.iter().enumerate() {
        tick(&mut live);
        live.levels(levels);
        let pose = dancer.tick(levels);
        assert_eq!(*live.frame(mode), pose.frame(), "tick {n}");
        moved |= pose != Pose::STILL;
    }
    assert!(moved, "a beat train moves the Dancer");
    // Home: everything at rest again, the sound still live.
    live.home();
    assert!(live.sound());
    let mut fresh = Live::new(Unit::Celsius, music(true), sample_values());
    fresh.set_sound(true);
    assert_eq!(frames(&live), frames(&fresh));
}

// The frame clock's calls are counted into the cooler's ticks: one step for each 100 ms, the
// first call after a start none, and a late call never made up for.
#[test]
fn the_beat_gives_one_step_for_each_100_ms_and_never_catches_up() {
    // At 60 Hz, over ten seconds: a hundred steps, give or take the one the grid's end gives.
    let mut beat = Beat::new();
    let mut steps = 0;
    let mut now = 1_000_000_i64;
    while now < 1_000_000 + 10 * 1_000_000 {
        if beat.due(now) {
            steps += 1;
        }
        now += 16_667;
    }
    assert!(
        (99..=101).contains(&steps),
        "{steps} steps in 10 s at 60 Hz"
    );
    // At 144 Hz and at 30 Hz alike: the grid is the cooler's, not the screen's.
    for hz in [30_i64, 59, 75, 144, 240] {
        let (mut beat, mut steps, mut now) = (Beat::new(), 0, 5_i64);
        while now < 5 + 10 * 1_000_000 {
            if beat.due(now) {
                steps += 1;
            }
            now += 1_000_000 / hz;
        }
        assert!(
            (98..=101).contains(&steps),
            "{steps} steps in 10 s at {hz} Hz"
        );
    }
    // The exact edges: the first call starts the count; a step is due at 100 ms, not before.
    let mut beat = Beat::new();
    assert!(!beat.due(0), "the first call after a start");
    assert!(!beat.due(TICK_US - 1));
    assert!(beat.due(TICK_US));
    assert!(!beat.due(TICK_US + 1));
    assert!(!beat.due(2 * TICK_US - 1));
    assert!(beat.due(2 * TICK_US));
    // A step less than a tick late keeps the grid: late by 50 ms, the next is still due at 200.
    let mut beat = Beat::new();
    beat.due(0);
    assert!(beat.due(TICK_US + 50_000), "50 ms late");
    assert!(!beat.due(2 * TICK_US - 1));
    assert!(beat.due(2 * TICK_US), "back on the grid");
    // A stall of a second: one step, and the next 100 ms after it, not nine in a row.
    let mut beat = Beat::new();
    beat.due(0);
    assert!(beat.due(1_000_000), "one step after the stall");
    assert!(!beat.due(1_000_001));
    assert!(!beat.due(1_000_000 + TICK_US - 1));
    assert!(beat.due(1_000_000 + TICK_US));
    // A tick or more late, exactly: skipped, not caught up.
    let mut beat = Beat::new();
    beat.due(0);
    assert!(beat.due(2 * TICK_US));
    assert!(!beat.due(2 * TICK_US + 1));
    assert!(beat.due(3 * TICK_US));
    // A restart forgets the schedule: the next call starts a new one.
    let mut beat = Beat::new();
    beat.due(0);
    assert!(beat.due(TICK_US));
    beat.restart();
    assert!(!beat.due(5 * TICK_US), "the first call after a restart");
    assert!(beat.due(6 * TICK_US));
}

// `Readings` become the tiles' values by each metric's spec name; one left out has no value, a
// name nobody knows is ignored, and no `Readings` yet is the design's sample.
#[test]
fn readings_become_values_by_their_spec_names() {
    assert_eq!(values_from(None), sample_values());
    let readings: HashMap<String, (i64, u64)> = [
        ("CPU Temperature", (57_250, 1000)),
        ("CPU Usage", (4, 3)),
        ("Gpu power", (-7, 1)),
        ("Memory Load", (i64::MAX, u64::MAX)),
        ("Fan 1", (2000, 1)),
    ]
    .into_iter()
    .map(|(name, fraction)| (name.to_owned(), fraction))
    .collect();
    let values = values_from(Some(&readings));
    for metric in Metric::ALL {
        let want = readings
            .get(metric.spec_name())
            .and_then(|&(n, d)| Exact::new(n, d));
        assert_eq!(values.get(metric), want, "{}", metric.spec_name());
    }
    for gap in [Metric::CpuPower, Metric::GpuTemperature, Metric::GpuUsage] {
        assert_eq!(values.get(gap), None, "{} has no value", gap.spec_name());
    }
    // Empty is not absent: no metric has a value, so every tile draws dashes, not the sample.
    assert_eq!(values_from(Some(&HashMap::new())), Values::new());
    assert_ne!(values_from(Some(&HashMap::new())), sample_values());
    // A denominator of 0 is no value (the state's reading refuses it first).
    let zero: HashMap<String, (i64, u64)> = HashMap::from([("CPU Usage".to_owned(), (5, 0))]);
    assert_eq!(values_from(Some(&zero)).get(Metric::CpuUsage), None);
}

#[test]
fn the_service_s_stops_are_the_stops_of_the_spectrum() {
    let stops = [
        (FallStop::Slower, Stop::Slower),
        (FallStop::Slow, Stop::Slow),
        (FallStop::Moderate, Stop::Moderate),
        (FallStop::Fast, Stop::Fast),
        (FallStop::Faster, Stop::Faster),
    ];
    for (bar, stop_bar) in stops {
        for (peak, stop_peak) in stops {
            for peaks in [true, false] {
                assert_eq!(
                    music_settings(peaks, bar, peak),
                    MusicSettings {
                        peaks,
                        bar_fall: stop_bar,
                        peak_fall: stop_peak,
                    }
                );
            }
        }
    }
}
