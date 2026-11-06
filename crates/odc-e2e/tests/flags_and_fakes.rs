//! `odc-e2e`'s flags, its start-up step and its fake mode, against fake trees, in `cargo test`
//! (Phase 5 plan, Task 9; Phase 6 plan, Task 9). The real cooler and the real GPUs are never
//! touched here: a fake tree's device nodes are regular files.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use odc_e2e::device::restore_switch;
use odc_e2e::{Area, Options, parse_args, run};
use odc_testkit::fake_tree::{FakeTree, LP360_HID_ID};
use odc_testkit::fixtures::{amdgpu_dgpu, nouveau, render_node};
use odc_testkit::hardware::Capability;
use odc_testkit::sysfs::{LP360_PRODUCT, LP360_VENDOR};

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("e2e")
        .join(name)
}

fn tree(name: &str) -> FakeTree {
    FakeTree::create(dir(name).join("tree")).expect("a fake tree")
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

const TICK: Duration = Duration::from_millis(1);

#[test]
fn flags_are_read_and_anything_unknown_is_refused() {
    assert_eq!(parse_args(&args(&[])), Ok(Some(Options::default())));
    assert_eq!(parse_args(&args(&["--help"])), Ok(None));
    assert_eq!(
        parse_args(&args(&["--attended"])),
        Ok(Some(Options {
            attended: true,
            ..Options::default()
        }))
    );
    assert_eq!(
        parse_args(&args(&["--only", "baseline"])).map(|o| o.and_then(|o| o.only)),
        Ok(Some(Area::Baseline))
    );
    assert_eq!(
        parse_args(&args(&["--only", "service", "--require", "dev-service"])),
        Ok(Some(Options {
            require: vec![Capability::DevService],
            only: Some(Area::Service),
            attended: false,
            modes: None,
        }))
    );
    assert_eq!(
        parse_args(&args(&[
            "--require",
            "lp360,lp360-reconnect",
            "--only",
            "device"
        ])),
        Ok(Some(Options {
            require: vec![Capability::Lp360, Capability::Lp360Reconnect],
            only: Some(Area::Device),
            attended: false,
            modes: None,
        }))
    );
    assert_eq!(
        parse_args(&args(&[
            "--require",
            "cpu-temp,board-fans",
            "--only",
            "sensors"
        ])),
        Ok(Some(Options {
            require: vec![Capability::CpuTemp, Capability::BoardFans],
            only: Some(Area::Sensors),
            attended: false,
            modes: None,
        }))
    );
    assert_eq!(
        parse_args(&args(&[
            "--require",
            "gpu-amdgpu,gpu-other",
            "--only",
            "gpu-rank"
        ])),
        Ok(Some(Options {
            require: vec![Capability::GpuAmdgpu, Capability::GpuOther],
            only: Some(Area::GpuRank),
            attended: false,
            modes: None,
        }))
    );
    assert_eq!(
        parse_args(&args(&["--require", "lp360", "--only", "display"])),
        Ok(Some(Options {
            require: vec![Capability::Lp360],
            only: Some(Area::Display),
            attended: false,
            modes: None,
        }))
    );
    for (bad, reason) in [
        (&["--require", "lp361"][..], "isn't a capability"),
        (&["--only", "nowhere"][..], "isn't an area"),
        // Phase 7's name for the display area (owner, 2026-11-05).
        (&["--only", "load"][..], "isn't an area"),
        (&["--fast"][..], "isn't an option"),
        (&["--require"][..], "needs a list"),
        (&["--modes"][..], "needs a list"),
    ] {
        let error = parse_args(&args(bad)).expect_err(bad[0]);
        assert!(error.contains(reason), "{bad:?}: {error}");
    }
}

// Phase 8 plan, Task 3: `--modes` takes numbers and ranges of modes this build draws.
#[test]
fn modes_are_numbers_and_ranges_of_modes_this_build_draws() {
    let modes = |list: &str| parse_args(&args(&["--modes", list])).map(|o| o.and_then(|o| o.modes));
    assert_eq!(modes("1"), Ok(Some(vec![1])));
    assert_eq!(modes("4-6,1,5"), Ok(Some(vec![1, 4, 5, 6])));
    let drawn = |n: u16| odc_render::Mode::new(n).is_some_and(odc_render::draws);
    // Modes 7–12 come one by one in Phase 8; each is accepted once drawn.
    assert_eq!(modes("7").is_ok(), drawn(7));
    assert_eq!(
        modes("7-9,12").is_ok(),
        [7, 8, 9, 12].into_iter().all(drawn)
    );
    for (bad, reason) in [
        ("0", "isn't in the mode table"),
        ("24", "isn't in the mode table"),
        ("9-7", "a range goes from the lower mode up"),
        ("x", "isn't a mode or a range of modes"),
        ("1,", "isn't a mode or a range of modes"),
        ("-3", "isn't a mode or a range of modes"),
    ] {
        let error = modes(bad).expect_err(bad);
        assert!(error.contains(reason), "{bad:?}: {error}");
    }
    // The first mode a later commit draws, while there is one (Phase 9: Music, mode by mode).
    if let Some(later) = odc_render::Mode::all().find(|&m| !odc_render::draws(m)) {
        let error = modes(&later.number().to_string()).expect_err("not drawn yet");
        assert!(error.contains("isn't drawn by this build yet"), "{error}");
    }
    // Without the flag, every mode this build draws.
    assert_eq!(Options::default().modes(), odc_e2e::drawn_modes());
    assert!(odc_e2e::drawn_modes().starts_with(&[1, 2, 3, 4, 5, 6]));
}

#[test]
fn with_no_cooler_every_check_runs_against_fakes_and_passes() {
    let t = tree("absent");
    let summary = run(&Options::default(), &t.roots(), &dir("absent"), TICK);
    let table = summary.table();
    assert!(summary.passed(), "{table}");
    assert!(table.contains("lp360            fake"), "{table}");
    assert!(table.contains("lp360-reconnect  fake"), "{table}");
    for check in ["test_images", "reconnect", "reset"] {
        assert!(table.contains(&format!("{check:<16} passed")), "{table}");
    }
    // The dev service's checks, in process (Phase 7 plan, Task 9); the baseline only when named.
    assert!(table.contains("dev-service      fake"), "{table}");
    assert!(!table.contains("baseline"), "{table}");
    for check in [
        "service_start",
        "service_timing",
        "service_mode_1",
        "service_mode_6",
        "service_rotated",
        "service_scrolling",
        "service_dashes",
        "service_S1",
        "service_S18",
        "service_S26",
    ] {
        assert!(table.contains(&format!("{check:<16} passed")), "{table}");
    }
    // Each mode drawn so far, through the fake cooler (Phase 7 plan, Task 3; Phase 8 plan, Task 3).
    for mode in odc_e2e::drawn_modes() {
        let check = format!("display_mode_{mode}");
        assert!(
            table.contains(&format!("{check:<16} passed  03 08, then")),
            "{table}"
        );
    }
    for gpu in ["amdgpu", "i915", "xe", "nvidia", "other"] {
        let capability = format!("gpu-{gpu}");
        let check = format!("gpu_rank_{gpu}");
        assert!(table.contains(&format!("{capability:<16} fake")), "{table}");
        assert!(table.contains(&format!("{check:<16} passed")), "{table}");
    }
    assert!(!table.contains("gpu_rank_real"), "no real GPU: {table}");
    for capability in [
        "cpu-temp",
        "cpu-power",
        "proc-stat",
        "meminfo",
        "board-fans",
    ] {
        assert!(table.contains(&format!("{capability:<16} fake")), "{table}");
    }
    for tool in ["sensors-tool", "gpu-load-tool"] {
        assert!(table.contains(&format!("{tool:<16} fake")), "{table}");
    }
    assert!(table.contains("no real check needs it"), "{table}");
    for check in [
        "cpu_temp",
        "cpu_usage",
        "cpu_power",
        "memory_load",
        "fans",
        "gpu_amdgpu",
        "gpu_i915",
        "gpu_xe",
        "gpu_nvidia",
        "gpu_other",
    ] {
        assert!(
            table.contains(&format!("{check:<16} passed  fake")),
            "{table}"
        );
    }
}

#[test]
fn a_gpu_whose_kind_query_fails_fails_its_check_with_the_error() {
    let t = tree("gpu-query-fails");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    render_node(&t, "0000:0a:00.0", 128).expect("a stand-in the probe can open");
    nouveau(&t, "0000:0e:00.0").expect("a GPU on nouveau");
    let options = Options {
        require: Vec::new(),
        only: Some(Area::GpuRank),
        attended: false,
        modes: None,
    };
    let summary = run(&options, &t.roots(), &dir("gpu-query-fails"), TICK);
    let table = summary.table();
    assert!(!summary.passed(), "{table}");
    assert!(table.contains("gpu-amdgpu       real"), "{table}");
    assert!(table.contains("gpu-other        real"), "{table}");
    assert!(
        table.contains("gpu_rank_real    passed  2 ranked"),
        "{table}"
    );
    assert!(
        table.contains("gpu_rank_amdgpu  FAILED  0000:0a:00.0: the kind query failed: "),
        "{table}"
    );
    assert!(table.contains("isn't the device 226:128"), "{table}");
    assert!(table.contains("gpu_rank_other   passed"), "{table}");
    assert!(table.contains("gpu_rank_i915    passed"), "a fake: {table}");
}

#[test]
fn a_required_capability_that_is_absent_fails_the_run() {
    let t = tree("required");
    let options = Options {
        require: vec![Capability::Lp360],
        only: None,
        attended: false,
        modes: None,
    };
    let summary = run(&options, &t.roots(), &dir("required"), TICK);
    assert!(!summary.passed());
    assert!(
        summary.table().contains("FAILED  required real"),
        "{}",
        summary.table()
    );
}

#[test]
fn an_lp360_without_access_fails_with_the_setup_fix_and_no_fakes() {
    let t = tree("unusable");
    let node = t.hidraw(3, LP360_HID_ID).expect("a node");
    fs::remove_file(&node).expect("the stand-in");
    fs::create_dir(&node).expect("a folder in its place: no account can write it");
    let summary = run(&Options::default(), &t.roots(), &dir("unusable"), TICK);
    let table = summary.table();
    assert!(!summary.passed(), "{table}");
    assert!(table.contains("test_images      FAILED"), "{table}");
    assert!(table.contains("odc-dev -- setup"), "{table}");
}

#[test]
fn the_start_up_step_switches_a_cooler_left_off_back_on() {
    let t = tree("switch-off");
    let usb = t
        .usb_device("1-6", LP360_VENDOR, LP360_PRODUCT, "0\n")
        .expect("a USB device");
    let note = restore_switch(&t.roots()).expect("the switch is readable");
    assert!(note.is_some_and(|n| n.contains("switched it on")));
    assert_eq!(
        fs::read_to_string(usb.join("authorized")).expect("the switch"),
        "1"
    );
}

#[test]
fn the_start_up_step_leaves_a_cooler_that_is_on_alone() {
    let t = tree("switch-on");
    let usb = t
        .usb_device("1-6", LP360_VENDOR, LP360_PRODUCT, "1\n")
        .expect("a USB device");
    assert_eq!(restore_switch(&t.roots()), Ok(None));
    assert_eq!(
        fs::read_to_string(usb.join("authorized")).expect("the switch"),
        "1\n",
        "nothing written"
    );
}

/// The test-only switch (Phase 9 plan, Task 3): every mode drawn, the Music modes included.
fn every_mode(_: odc_render::Mode) -> bool {
    true
}

// Phase 9 plan, Task 5: each Music mode without values draws its examples, in silence, as the
// files expect, before its own commit makes it drawn; the value ones draw through `picture`, so
// they come with Task 8.
#[test]
fn music_examples_draw_their_expected_frames_in_silence() {
    for n in [13, 14, 15, 16, 17, 23] {
        let mode = odc_render::Mode::new(n).expect("a mode");
        let frames = odc_e2e::display::examples_match(mode).unwrap_or_else(|e| panic!("{e}"));
        assert!(frames >= 6, "mode {n}: {frames} frames");
    }
}

// The plans the service checks play for a Music mode, through the engine with the test-only
// switch: a case in silence upright and turned, mode 13's test signal through the override, and
// the first variant of each scenario type, with their sound and Music settings.
#[test]
fn music_plans_play_out_through_the_engine() {
    use odc_e2e::plays::{Plan, play_engine_drawing};
    use odc_testkit::expected::{load_mode, mode_scenarios};
    let play =
        |plan: &Plan| play_engine_drawing(plan, every_mode).unwrap_or_else(|e| panic!("{e}"));
    play(&Plan::of_signal("column 7 centre, -20 dBFS").expect("a plan"));
    for n in 13..=23 {
        let file = load_mode(n);
        for rotation in [0, 1] {
            let case = file
                .cases
                .iter()
                .find(|c| c.rotation.setting() == rotation && c.pose.is_none_or(|p| p == 0))
                .expect("a case");
            play(&Plan::of_case(n, case).expect("a plan"));
        }
        let mut seen = Vec::new();
        for scenario in mode_scenarios(n) {
            let id = scenario.heading.split(' ').next().unwrap_or("").to_owned();
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            play(&Plan::of_scenario(&scenario));
        }
    }
}
