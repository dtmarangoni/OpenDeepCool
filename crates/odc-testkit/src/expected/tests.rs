use super::*;

/// Every file loads into typed cases, as many as it has `##` headings (284 when Phase 5
/// ran, 328 with Phase 9's Dancer, 336 with the six emoji modes of Phase 11a); cycles have at
/// least 21 pictures, `?` appears only in the spectrum's modes 13–16 and 18–22, and mode 23
/// has each of the Dancer's eleven poses in each rotation.
#[test]
fn every_expected_frame_file_loads() {
    let mut modes: Vec<u16> = fs::read_dir(FRAMES_DIR)
        .expect("docs/spec/frames/ is readable")
        .map(|entry| entry.expect("a directory entry").file_name())
        .filter_map(|name| {
            let name = name.into_string().ok()?;
            name.strip_prefix("mode-")?
                .strip_suffix(".txt")?
                .parse()
                .ok()
        })
        .collect();
    modes.sort_unstable();
    assert!(!modes.is_empty());
    let (mut cases, mut headings) = (0, 0);
    let mut dancer = Vec::new();
    for mode in modes {
        let text = fs::read_to_string(Path::new(FRAMES_DIR).join(format!("mode-{mode:03}.txt")))
            .expect("a mode file");
        headings += text.lines().filter(|l| l.starts_with("## ")).count();
        let frames = load_mode(mode);
        assert!(!frames.title.is_empty());
        for case in &frames.cases {
            if let Kind::Scrolling(pictures) = &case.kind {
                assert!(pictures.len() >= 21, "mode {mode}, {}", case.heading);
            }
            if !matches!(mode, 13..=16 | 18..=22) {
                let spectrum = case.kind.pictures().iter().any(|p| p.has_any());
                assert!(
                    !spectrum,
                    "mode {mode}, {}: `?` outside Music",
                    case.heading
                );
            }
        }
        for case in &frames.cases {
            assert_eq!(
                case.pose.is_some(),
                mode == 23,
                "mode {mode}, {}",
                case.heading
            );
            if let Some(pose) = case.pose {
                dancer.push((case.rotation.setting(), pose));
            }
        }
        cases += frames.cases.len();
    }
    assert_eq!(cases, headings);
    assert_eq!(cases, 336);
    dancer.sort_unstable();
    dancer.dedup();
    assert_eq!(dancer.len(), 4 * 11, "each pose in each rotation");
}

/// The dash cases and the scenarios load too: every heading becomes a case or a scenario.
#[test]
fn the_dash_cases_and_the_scenarios_load() {
    let dashes = load_unavailable();
    assert_eq!(dashes.len(), 44);
    assert!(
        dashes
            .iter()
            .all(|(mode, _)| [1, 4, 6, 7, 9, 10, 11, 12].contains(mode))
    );
    // A dash case of a mode with a temperature names its unit.
    for (mode, case) in &dashes {
        assert_eq!(
            case.unit.is_some(),
            [7, 9].contains(mode),
            "{}",
            case.heading
        );
    }
    let signals = fs::read_to_string(Path::new(FRAMES_DIR).join("spectrum/signals.txt"))
        .expect("the signals");
    let signal = |name: &str| signals.lines().any(|l| l.strip_prefix("## ") == Some(name));
    // Every mode but the retired 17: 25 files of scenarios.
    for mode in (1..=26).filter(|&m| m != 17) {
        let name = format!("mode-{mode:03}.txt");
        let text = fs::read_to_string(Path::new(FRAMES_DIR).join("scenarios").join(&name))
            .expect("a scenario file");
        let scenarios = mode_scenarios(mode);
        let headings = text.lines().filter(|l| l.starts_with("## ")).count();
        assert_eq!(scenarios.len(), headings, "{name}");
        for scenario in &scenarios {
            // A Music mode's file also holds the switches into it from modes 1–12.
            let into = scenario
                .ticks
                .iter()
                .any(|t| t.changes.contains(&Change::Reset(Reset::Mode(mode))));
            assert!(
                scenario.mode == mode || (mode >= 13 && into),
                "{name}: {}",
                scenario.heading
            );
            check_ticks(scenario);
            for change in scenario.ticks.iter().flat_map(|t| &t.changes) {
                if let Change::Audio(played) = change {
                    assert!(signal(played), "{name}: no signal {played:?}");
                }
            }
        }
        // A temperature mode's scenarios run in each unit, and their headings say which.
        if [7, 8, 9, 18, 19].contains(&mode) {
            for (unit, label) in [(Unit::Celsius, " · °C"), (Unit::Fahrenheit, " · °F")] {
                let mine = scenarios
                    .iter()
                    .filter(|s| s.unit == unit && s.mode == mode);
                assert!(mine.clone().count() > 0, "{name}: none in {unit:?}");
                assert!(mine.into_iter().all(|s| s.heading.contains(label)));
            }
        }
    }
}

/// A missed tick, and only a missed tick, has no frame.
fn check_ticks(scenario: &Scenario) {
    for tick in &scenario.ticks {
        let missed = tick.changes.contains(&Change::Missed);
        assert_eq!(
            tick.frame.is_none(),
            missed,
            "{}, tick {}",
            scenario.heading,
            tick.n
        );
    }
}

// Phase 8 plan, Task 1: tick 0's unit is the scenario's, and a unit change in a mode with a
// temperature is a reset.
#[test]
fn a_scenario_reads_its_unit_and_its_unit_resets() {
    let text = scenario_file(
        "0: reset start-up; unit fahrenheit; CPU Usage = 100 → f1\n\
             1: reset unit celsius → f1\n\
             2: unit fahrenheit, only saved → f1\n",
    );
    let scenario = &parse_scenarios("test.txt", &text)[0];
    assert_eq!(scenario.unit, Unit::Fahrenheit);
    assert_eq!(
        scenario.ticks[0].changes.len(),
        2,
        "the unit isn't a change"
    );
    assert_eq!(
        scenario.ticks[1].changes,
        [Change::Reset(Reset::Unit(Unit::Celsius))]
    );
    assert_eq!(
        scenario.ticks[2].changes,
        [Change::UnitSaved(Unit::Fahrenheit)]
    );
    let none = &parse_scenarios("test.txt", &scenario_file(START))[0];
    assert_eq!(none.unit, Unit::Celsius, "no unit named runs in °C");
}

#[test]
#[should_panic(
    expected = "test.txt:6: \"unit celsius\" without `reset` or `only saved` comes at tick 0 only"
)]
fn a_bare_unit_after_tick_0_is_refused() {
    parse_scenarios(
        "test.txt",
        &scenario_file(&format!("{START}1: unit celsius → f1\n")),
    );
}

#[test]
#[should_panic(expected = "test.txt:5: tick 0 names the unit twice")]
fn a_unit_named_twice_is_refused() {
    parse_scenarios(
        "test.txt",
        &scenario_file("0: reset start-up; unit celsius; unit fahrenheit → f1\n"),
    );
}

#[test]
fn every_metric_gets_the_named_reading_or_its_own_filler() {
    let all = every_metric(&[
        ("CPU Usage".to_owned(), Some(Exact::whole(42))),
        ("Memory Load".to_owned(), None),
    ]);
    assert_eq!(all.len(), 7);
    assert!(all.contains(&(Metric::CpuUsage, Some(Exact::whole(42)))));
    assert!(all.contains(&(Metric::MemoryLoad, None)));
    assert!(all.contains(&(Metric::GpuUsage, Some(Exact::whole(73)))));
    let mut fillers: Vec<i64> = FILLERS.iter().map(|&(_, f)| f).collect();
    fillers.dedup();
    assert_eq!(fillers.len(), 7, "every filler its own");
}

#[test]
#[should_panic(expected = "\"Cpu usage\" isn't a metric")]
fn an_unknown_metric_name_is_refused() {
    every_metric(&[("Cpu usage".to_owned(), None)]);
}

#[test]
fn readings_are_exact_decimals_or_unavailable() {
    assert_eq!(reading("42"), Some(Some(Exact::whole(42))));
    assert_eq!(reading("99.49"), Exact::new(9949, 100).map(Some));
    assert_eq!(reading("-0.5"), Exact::new(-5, 10).map(Some));
    assert_eq!(reading("unavailable"), Some(None));
    for bad in ["", "-", ".5", "5.", "1e3", "+1", "4 2", "NaN", "inf"] {
        assert_eq!(reading(bad), None, "{bad:?}");
    }
}

// display.md S9 (owner, 2026-10-29): `9%` centred, `10%` re-centred, and back.
#[test]
fn the_s9_scenario_of_mode_1_reads_as_written() {
    let scenarios = mode_scenarios(1);
    let s9 = scenarios
        .iter()
        .find(|s| s.heading == "## S9 · mode 1")
        .expect("S9 in mode 1");
    assert_eq!((s9.id.as_str(), s9.mode, s9.ticks.len()), ("S9", 1, 6));
    let cpu = |value: i64| Change::Reading {
        metric: "CPU Usage".to_owned(),
        value: Some(Exact::whole(value)),
    };
    assert_eq!(s9.ticks[0].changes[0], Change::Reset(Reset::StartUp));
    assert_eq!(s9.ticks[0].changes[1], cpu(9));
    assert_eq!(s9.ticks[2].changes, [cpu(10)]);
    let nine = s9.ticks[0].frame.as_ref().expect("a frame");
    let ten = s9.ticks[2].frame.as_ref().expect("a frame");
    // `9%` takes columns 4–12, `10%` columns 2–14: the 1's foot starts at column 2.
    assert_eq!((nine.get(6, 4), nine.get(10, 2)), (Pixel::On, Pixel::Off));
    assert_eq!(ten.get(10, 2), Pixel::On);
    assert_eq!(s9.ticks[4].frame, s9.ticks[0].frame);
}

// Phase 9 plan, Task 1: the scenarios play sound and change the Music settings.
#[test]
fn a_scenario_reads_its_sound_and_the_music_settings() {
    let scenarios = parse_scenarios(
        "test.txt",
        &scenario_file(
            "0: reset start-up; audio burst train, column 7 → f1\n\
                 1: music peaks off; music bar fall faster; music peak fall slower → f1\n",
        ),
    );
    assert_eq!(
        scenarios[0].ticks[0].changes[1],
        Change::Audio("burst train, column 7".to_owned())
    );
    assert_eq!(
        scenarios[0].ticks[1].changes,
        [
            Change::MusicPeaks(false),
            Change::MusicBarFall("faster".to_owned()),
            Change::MusicPeakFall("slower".to_owned()),
        ]
    );
}

#[test]
#[should_panic(expected = "isn't a change")]
fn a_stop_that_isnt_one_of_the_five_is_refused() {
    parse_scenarios(
        "test.txt",
        &scenario_file("0: reset start-up; music bar fall fastest → f1\n"),
    );
}

fn scenario_file(ticks: &str) -> String {
    format!(
        "# Test scenarios\n\n## S1 · mode 1\n\n{ticks}\n# Frames\n\n### f1\n\n{}",
        BLANK_ROWS.repeat(14)
    )
}

const START: &str = "0: reset start-up; CPU Usage = 100 → f1\n";

#[test]
fn a_scenario_file_reads_its_ticks_and_frames() {
    let text = scenario_file(&format!(
        "{START}1: missed → no frame\n2: reset rotation +2 → f1\n"
    ));
    let scenarios = parse_scenarios("test.txt", &text);
    let ticks = &scenarios[0].ticks;
    assert_eq!(ticks.len(), 3);
    assert_eq!(ticks[1].frame, None);
    assert_eq!(ticks[2].changes, [Change::Reset(Reset::Rotation(2))]);
    assert_eq!(ticks[2].frame, ticks[0].frame);
}

#[test]
#[should_panic(expected = "test.txt:6: \"reset rotation +4\" isn't a change")]
fn an_unknown_change_is_refused() {
    parse_scenarios(
        "test.txt",
        &scenario_file(&format!("{START}1: reset rotation +4 → f1\n")),
    );
}

#[test]
#[should_panic(expected = "test.txt:6: f2 isn't defined under `# Frames`")]
fn a_missing_frame_is_refused() {
    parse_scenarios("test.txt", &scenario_file(&format!("{START}1: - → f2\n")));
}

#[test]
#[should_panic(expected = "test.txt:6: a missed tick has no frame")]
fn a_missed_tick_with_a_frame_is_refused() {
    parse_scenarios(
        "test.txt",
        &scenario_file(&format!("{START}1: missed → f1\n")),
    );
}

#[test]
#[should_panic(expected = "test.txt:6: tick 2 comes after tick 0")]
fn a_skipped_tick_number_is_refused() {
    parse_scenarios("test.txt", &scenario_file(&format!("{START}2: - → f1\n")));
}

#[test]
#[should_panic(expected = "test.txt:3: a scenario starts with tick 0 and `reset start-up`")]
fn a_scenario_without_a_start_is_refused() {
    parse_scenarios("test.txt", &scenario_file("0: CPU Usage = 100 → f1\n"));
}

#[test]
#[should_panic(expected = "test.txt: f2 is defined but no tick names it")]
fn an_unused_frame_is_refused() {
    let text = format!(
        "{}\n### f2\n\n{}",
        scenario_file(START),
        BLANK_ROWS.repeat(14)
    );
    parse_scenarios("test.txt", &text);
}

#[test]
fn mode_1_reads_as_written() {
    let frames = load_mode(1);
    assert_eq!(frames.title, "Load · CPU");
    let first = &frames.cases[0];
    assert_eq!(first.heading, "## rotation 0, CPU Usage = 3");
    assert_eq!(first.rotation, Rotation::default());
    assert_eq!(first.unit, None);
    assert_eq!(
        first.readings,
        [("CPU Usage".to_owned(), Some(Exact::whole(3)))]
    );
    let Kind::Static(picture) = &first.kind else {
        panic!("3% is static");
    };
    // display.md: the digit 3's top row, centred, and the % sign.
    assert_eq!(picture.get(6, 4), Pixel::On);
    assert_eq!(picture.get(6, 7), Pixel::Off);
    assert_eq!(picture.get(1, 1), Pixel::Off);
    assert!(matches!(&frames.cases[2].kind, Kind::Scrolling(p) if p.len() == 21));
}

#[test]
fn units_and_loops_read_as_written() {
    let temperature = load_mode(7);
    assert_eq!(temperature.cases[0].unit, Some(Unit::Celsius));
    assert!(
        temperature
            .cases
            .iter()
            .any(|c| c.unit == Some(Unit::Fahrenheit))
    );
    let emoji = load_mode(14);
    let Kind::Loop(steps) = &emoji.cases[0].kind else {
        panic!("an emoji loop");
    };
    let durations: Vec<u64> = steps.iter().map(|(d, _)| d.as_millis() as u64).collect();
    assert_eq!(durations, [600, 100, 100, 200]);
    assert!(steps[0].1.has_any());
}

const BLANK_ROWS: &str = "..............\n";

fn file(head: &str, rows: &str) -> String {
    format!("# Mode 1: Test\n\n{head}\n\n{rows}")
}

#[test]
#[should_panic(expected = "test.txt:1: expected \"# Mode 1: \"")]
fn a_wrong_title_is_refused() {
    parse("test.txt", 1, "# Mode 2: Test\n");
}

#[test]
#[should_panic(expected = "test.txt:3: the heading doesn't start with `rotation 0` to `3`")]
fn rotation_4_is_refused() {
    parse(
        "test.txt",
        1,
        &file("## rotation 4", &BLANK_ROWS.repeat(14)),
    );
}

#[test]
#[should_panic(expected = "test.txt:3: \"CPU Usage: 3\" is neither a unit nor `Name = value`")]
fn a_malformed_reading_is_refused() {
    parse(
        "test.txt",
        1,
        &file("## rotation 0, CPU Usage: 3", &BLANK_ROWS.repeat(14)),
    );
}

#[test]
#[should_panic(expected = "test.txt:7: 'x' isn't '#', '.' or '?'")]
fn a_foreign_pixel_is_refused() {
    let rows = format!(
        "{}......x.......\n{}",
        BLANK_ROWS.repeat(2),
        BLANK_ROWS.repeat(11)
    );
    parse("test.txt", 1, &file("## rotation 0", &rows));
}

#[test]
#[should_panic(expected = "isn't 14 pixels wide")]
fn a_short_row_is_refused() {
    let rows = format!(".............\n{}", BLANK_ROWS.repeat(13));
    parse("test.txt", 1, &file("## rotation 0", &rows));
}

#[test]
#[should_panic(expected = "test.txt:17: the file ends after 13 of 14 picture rows")]
fn a_short_block_is_refused() {
    parse(
        "test.txt",
        1,
        &file("## rotation 0", &BLANK_ROWS.repeat(13)),
    );
}

#[test]
#[should_panic(expected = "test.txt:3: a static case has one picture, not 2")]
fn two_pictures_without_a_note_are_refused() {
    let rows = "..............  ..............\n".repeat(14);
    parse("test.txt", 1, &file("## rotation 0", &rows));
}

#[test]
#[should_panic(expected = "test.txt:3: a cycle of 21 frames (at least 21) has 1 pictures")]
fn a_cycle_with_missing_frames_is_refused() {
    let rows = format!(
        "21 frames in time order, one per 100 ms tick\n\n{}",
        BLANK_ROWS.repeat(14)
    );
    parse("test.txt", 1, &file("## rotation 0", &rows));
}

#[test]
#[should_panic(expected = "test.txt:3: a loop of 2 durations has 1 pictures")]
fn a_loop_with_a_missing_picture_is_refused() {
    let rows = format!("{LOOP_NOTE}400 ms, 100 ms\n\n{}", BLANK_ROWS.repeat(14));
    parse("test.txt", 1, &file("## rotation 0", &rows));
}

#[test]
#[should_panic(expected = "test.txt:5: a duration isn't `<n> ms`")]
fn a_malformed_duration_is_refused() {
    let rows = format!("{LOOP_NOTE}400 ms, 0.1 s\n\n{}", BLANK_ROWS.repeat(14));
    parse("test.txt", 1, &file("## rotation 0", &rows));
}

#[test]
#[should_panic(expected = "test.txt:1: the file has no case")]
fn a_file_without_cases_is_refused() {
    parse("test.txt", 1, "# Mode 1: Test\n\n");
}

#[test]
fn spectrum_pixels_match_either_value() {
    let mut rows = [".............."; SIZE];
    rows[0] = "?.............";
    let picture = Picture::from_rows(&rows);
    let mut frame = Frame::blank();
    assert!(picture.matches(&frame));
    frame.set(1, 1, true);
    assert!(picture.matches(&frame));
    frame.set(1, 2, true);
    assert!(!picture.matches(&frame));
    assert_eq!(picture.to_frame(), None);
}
