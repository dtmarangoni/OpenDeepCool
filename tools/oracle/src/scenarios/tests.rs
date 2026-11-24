use super::*;

/// Every scenario of the 25 files.
fn all() -> Vec<Scenario> {
    MODES.into_iter().flat_map(scenarios).collect()
}

/// Every scenario's heading, mode and run, file by file, in file order.
fn runs() -> Vec<(String, u16, Vec<Tick>)> {
    all()
        .iter()
        .map(|s| (s.heading.clone(), s.mode, run(s)))
        .collect()
}

/// The first columns, from 1 to 14, of the copies of a strip drawn at `x` with cycle `cycle`.
fn on_screen(x: i64, cycle: i64) -> impl Iterator<Item = i64> {
    (1..=14).filter(move |c| (c - x).rem_euclid(cycle) == 0)
}

// Phase 7 plan, Task 1: nothing ever moves by more than one column per frame except at a
// reset, or when a stopped picture is re-centred at its new home (S9; owner, 2026-10-29).
// Every copy whose first column is on the screen had it there, or one column to its right,
// in the frame before.
#[test]
fn nothing_moves_more_than_one_column_per_frame_except_at_a_reset() {
    for (heading, _, ticks) in runs() {
        let mut previous: Option<Placed> = None;
        for tick in ticks.iter().filter(|t| t.frame.is_some()) {
            let Some(placed) = tick.placed else {
                previous = None;
                continue;
            };
            let re_centred = !tick.moving && placed.x == placed.home;
            if let Some(before) = previous
                && !tick.reset
                && !re_centred
            {
                let was = |c: i64| (c - before.x).rem_euclid(before.cycle) == 0;
                for c in on_screen(placed.x, placed.cycle) {
                    assert!(
                        was(c) || was(c + 1),
                        "{heading}, tick {}: a copy at column {c}",
                        tick.n
                    );
                }
            }
            previous = Some(placed);
        }
    }
}

// S1, S12: a strip that must scroll moves one column per frame from home; a stopped strip
// is at home.
#[test]
fn a_stopped_strip_is_always_at_home() {
    for (heading, _, ticks) in runs() {
        for tick in &ticks {
            if let Some(placed) = tick.placed
                && !tick.moving
            {
                assert_eq!(placed.x, placed.home, "{heading}, tick {}", tick.n);
            }
        }
    }
}

// Phase 8 plan, Task 1: tick 0 names all seven metrics, so a switch into another kind draws
// named readings.
#[test]
fn every_scenario_names_a_mode_starts_with_a_reset_and_names_every_metric() {
    for (heading, mode, ticks) in runs() {
        assert!(MODES.contains(&mode), "{heading}");
        assert!(ticks[0].reset, "{heading}");
        assert_eq!(ticks[0].n, 0, "{heading}");
        assert!(
            ticks.windows(2).all(|w| w[1].n == w[0].n + 1),
            "{heading}: ticks in order"
        );
        for metric in METRICS {
            assert!(
                ticks[0]
                    .steps
                    .iter()
                    .any(|s| matches!(s, Step::Set(m, _) if *m == metric)),
                "{heading}: tick 0 doesn't name {metric}"
            );
        }
    }
}

// AC4: all 132 ordered pairs among modes 1–12, and from Phase 9 every ordered pair of modes (506
// among the 23 of Phase 9, 600 among today's 25), a pair with a mode that shows a temperature in
// both units.
#[test]
fn every_ordered_pair_of_modes_has_a_switch_in_each_unit_it_needs() {
    let mut switches: Vec<(u16, u16, Option<bool>)> = Vec::new();
    for scenario in all().iter().filter(|s| s.heading.starts_with("## S14 ")) {
        for step in scenario.script.values().flatten() {
            if let Step::Reset(Cause::Mode(target)) = step {
                switches.push((scenario.mode, *target, scenario.unit));
            }
        }
    }
    for from in MODES {
        for into in MODES.into_iter().filter(|&t| t != from) {
            let units: &[Option<bool>] = if shows_temperature(from) || shows_temperature(into) {
                &[Some(false), Some(true)]
            } else {
                &[None]
            };
            for unit in units {
                assert!(
                    switches.contains(&(from, into, *unit)),
                    "no switch from mode {from} into {into} in {unit:?}"
                );
            }
        }
    }
}

// display.md, Mode switches: a unit change resets only a mode that shows a temperature, and
// is only saved in any other.
#[test]
fn a_unit_change_resets_only_where_a_temperature_is_shown() {
    for scenario in all() {
        let mut mode = scenario.mode;
        for (tick, steps) in &scenario.script {
            for step in steps {
                match step {
                    Step::Reset(Cause::Mode(m)) => mode = *m,
                    Step::Reset(Cause::Unit(_)) => assert!(
                        shows_temperature(mode),
                        "{}, tick {tick}: a reset in mode {mode}",
                        scenario.heading
                    ),
                    Step::UnitSaved(_) => assert!(
                        !shows_temperature(mode),
                        "{}, tick {tick}: only saved in mode {mode}",
                        scenario.heading
                    ),
                    Step::Unit(_) => assert_eq!(*tick, 0, "{}", scenario.heading),
                    _ => {}
                }
            }
        }
    }
}

// S23: the mode refused as unavailable shows a metric the scenario's mode doesn't, so the
// service can make it unavailable while the scenario's mode still draws its own.
#[test]
fn s23_refuses_a_mode_whose_metric_the_scenario_s_mode_doesn_t_show() {
    for mode in 1..=12 {
        let refused = refused_unavailable(mode);
        assert_ne!(refused, mode);
        assert!(
            shown(refused).iter().any(|m| !shown(mode).contains(m)),
            "mode {mode} refuses mode {refused}"
        );
    }
    // Load keeps Phase 7's choices; modes 9 and 12 reach outside their kind.
    let chosen: Vec<u16> = (1..=12).map(refused_unavailable).collect();
    assert_eq!(chosen, [2, 3, 4, 5, 6, 1, 8, 9, 1, 11, 12, 1]);
}

// Phase 8 plan, Task 1: a unit change in a Load or Power mode leaves every frame as it was.
#[test]
fn a_unit_change_only_saved_changes_no_frame() {
    let mut checked = 0;
    for scenario in all() {
        let saved = |s: &Step| matches!(s, Step::UnitSaved(_));
        if !scenario.script.values().flatten().any(saved) {
            continue;
        }
        let mut without = scenario.clone();
        for steps in without.script.values_mut() {
            steps.retain(|s| !saved(s));
        }
        let frames = |s: &Scenario| run(s).into_iter().map(|t| t.frame).collect::<Vec<_>>();
        assert!(
            frames(&scenario) == frames(&without),
            "{}: a frame differs",
            scenario.heading
        );
        checked += 1;
    }
    assert!(checked > 0);
}

// S7 and S20 in °F: the °C readings chosen draw the °F numbers they're chosen for.
#[test]
fn the_fahrenheit_readings_draw_the_numbers_they_stand_for() {
    let drawn = |c: &str| {
        crate::rules::number(Ratio::decimal(c), Kind::Temperature, true).expect("a number")
    };
    assert_eq!(
        boundaries("CPU Temperature", true)
            .iter()
            .map(|c| drawn(c))
            .collect::<Vec<_>>(),
        [32, 99, 100, 999, 1000, 9997, 9999, 9999, 9999]
    );
    for metric in ["CPU Temperature", "Gpu temperature"] {
        assert_eq!(drawn(near_limit(metric, true, false)), 99);
        assert_eq!(drawn(near_limit(metric, true, true)), 100);
        assert_eq!(drawn(&drawing(metric, true, 999)), 999);
        assert_eq!(drawn(&drawing(metric, true, 99)), 99);
    }
    assert_eq!(drawn(&level("CPU Temperature", true, Level::Max)), 9999);
    assert_eq!(drawn(&level("Gpu temperature", true, Level::Max)), 9997);
    assert_eq!(drawn("5537.5"), 9999, "10 000, capped");
}

/// The mode each tick draws, from the scenario's switches.
fn modes(scenario: &Scenario, ticks: &[Tick]) -> Vec<u16> {
    let mut mode = scenario.mode;
    ticks
        .iter()
        .map(|tick| {
            for step in &tick.steps {
                if let Step::Reset(Cause::Mode(m)) = step {
                    mode = *m;
                }
            }
            mode
        })
        .collect()
}

// music.md, Emoji animations: a reset shows the loop's first picture on the next frame; the loop
// runs while sound plays and for nine ticks after it, otherwise it stands on its first
// picture; a unit change or a Music setting change moves nothing.
#[test]
fn an_emoji_loop_restarts_at_every_reset_and_runs_only_while_sound_plays() {
    let (mut checked, mut moving, mut resting) = (0, 0, 0);
    for scenario in all() {
        let ticks = run(&scenario);
        let (mut at, mut quiet) = (0, REST_TICKS);
        for (tick, mode) in ticks.iter().zip(modes(&scenario, &ticks)) {
            if !is_emoji(mode) {
                continue;
            }
            if tick.reset {
                at = 0;
                quiet = REST_TICKS;
                checked += 1;
            }
            if tick.sound {
                quiet = 0;
            } else if quiet < REST_TICKS {
                quiet += 1;
            }
            let runs = quiet < REST_TICKS;
            if !runs {
                at = 0;
                resting += 1;
            } else if at > 0 {
                moving += 1;
            }
            let loop_ = emoji_ticks(mode);
            let picture = emoji(mode)[loop_[at]];
            let frame = tick.frame.expect("no missed tick in an emoji mode");
            for (r, line) in picture.iter().enumerate() {
                assert!(
                    frame[r] == row(line),
                    "{}, tick {}, row {}",
                    scenario.heading,
                    tick.n,
                    r + 1
                );
            }
            // With a spectrum, row 8 is off; alone, the picture has every row.
            assert_eq!(picture.len(), if is_emoji_alone(mode) { SIZE } else { 7 });
            if !is_emoji_alone(mode) {
                assert!(frame[7].iter().all(|&p| p == Pixel::Off));
            }
            if runs {
                at = (at + 1) % loop_.len();
            }
        }
    }
    // The scenarios cover both: loops that run under the burst train, and loops at rest.
    assert!(checked > 0 && moving > 0 && resting > 0);
}

// music.md point 7: a Music setting change resets nothing. Without it, every frame of a mode
// without a spectrum and every top area stays the same; the spectrum changes somewhere.
#[test]
fn a_music_setting_changes_only_the_spectrum() {
    let setting = |s: &Step| {
        matches!(
            s,
            Step::MusicPeaks(_) | Step::MusicBarFall(_) | Step::MusicPeakFall(_)
        )
    };
    let mut checked = 0;
    for scenario in all() {
        if !scenario.script.values().flatten().any(setting) {
            continue;
        }
        let mut without = scenario.clone();
        for steps in without.script.values_mut() {
            steps.retain(|s| !setting(s));
        }
        let (with_, plain) = (run(&scenario), run(&without));
        let mut differs = false;
        for ((a, b), mode) in with_.iter().zip(&plain).zip(modes(&scenario, &with_)) {
            let (a, b) = (a.frame.expect("a frame"), b.frame.expect("a frame"));
            let top = match mode {
                13 => 2,
                14..=16 | 18..=22 => 8,
                _ => SIZE,
            };
            assert!(a[..top] == b[..top], "{}", scenario.heading);
            differs |= a != b;
        }
        assert!(
            differs,
            "{}: the settings changed nothing",
            scenario.heading
        );
        checked += 1;
    }
    assert!(checked >= 12);
}

// music.md, Spectrum: layout: each column a solid bar from the lit floor, and at most one dot
// above it with a gap, within the area; nothing between the area and what's above it.
#[test]
fn every_music_frame_meets_the_spectrum_layout() {
    for scenario in all() {
        let ticks = run(&scenario);
        for (tick, mode) in ticks.iter().zip(modes(&scenario, &ticks)) {
            // A missed tick draws nothing (S22).
            let Some(frame) = tick.frame.filter(|_| has_spectrum(mode)) else {
                continue;
            };
            let first = if mode == 13 { 2 } else { 8 };
            assert!(frame[first - 1].iter().all(|&p| p == Pixel::Off));
            for c in 0..SIZE {
                let column: Vec<Pixel> = frame.iter().map(|row| row[c]).collect();
                let lit: Vec<usize> = (first..SIZE).filter(|&r| column[r] == Pixel::On).collect();
                // The bar: lit rows from the floor up, without a gap; the dot, above a gap.
                let bar = lit
                    .iter()
                    .rev()
                    .zip((0..SIZE).rev())
                    .take_while(|(r, want)| **r == *want)
                    .count();
                assert!(bar >= 1, "{}, tick {}: no floor", scenario.heading, tick.n);
                assert!(
                    lit.len() <= bar + 1,
                    "{}, tick {}, column {c}",
                    scenario.heading,
                    tick.n
                );
            }
        }
    }
}

// music.md point 8: entering a spectrum mode from a mode without one starts the levels at 0,
// and a switch between Music modes keeps them. Two ticks after column 7's first burst, its bar
// still stands above the floor after 14 → 13, and is the floor after 1 → 13.
#[test]
fn the_spectrum_starts_at_0_when_entered_and_carries_on_between_music_modes() {
    let column_7 = |from: u16| {
        let scenario = Script::with("S14", from, None, &[], &[])
            .at(0, [Step::Audio(BURST)])
            .at(2, [Step::Reset(Cause::Mode(13))])
            .end(End::Total(3));
        let frame = run(&scenario)[2].frame.expect("a frame");
        (2..SIZE).filter(|&r| frame[r][6] == Pixel::On).count()
    };
    assert!(column_7(14) > 2);
    assert_eq!(column_7(1), 1);
}
