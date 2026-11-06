//! The Music checks' tools (Phase 9 plan, Task 5): reading `pactl`'s and `pw-dump`'s output, the
//! start-up repair of a killed run's leftovers, the harness that sets everything back however a
//! check ends, the delay's arithmetic, and the capture in process. This test program is also the
//! fake `pactl`, `systemctl`, `pw-dump`, `pw-record`, `pw-play` and helper (AGENTS.md rule 13):
//! each case links those names to it in its own `bin` folder, and each fake logs its arguments to
//! `calls` and answers from the folder's files. Nothing here touches PipeWire or systemd.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::{Duration, Instant};

use odc_e2e::music::{
    HELPER_UNIT, Harness, Tools, capture_at_full_volume, delays, fake_capture, has_node,
    leftover_sinks, module_index, quantile, repair,
};
use odc_protocol::Frame;
use odc_render::Mode;
use odc_testkit::hardware::{Capability, Probe, Roots, Summary, probe};

type Test = (&'static str, fn());

const TESTS: [Test; 7] = [
    (
        "the_tools_outputs_are_read_as_pipewire_prints_them",
        the_tools_outputs_are_read_as_pipewire_prints_them,
    ),
    (
        "the_start_repairs_what_a_killed_run_left",
        the_start_repairs_what_a_killed_run_left,
    ),
    (
        "the_harness_sets_everything_back_even_when_a_check_panics",
        the_harness_sets_everything_back_even_when_a_check_panics,
    ),
    (
        "each_delay_is_the_first_frame_that_shows_its_burst",
        each_delay_is_the_first_frame_that_shows_its_burst,
    ),
    (
        "in_process_the_tone_draws_its_expected_rows",
        in_process_the_tone_draws_its_expected_rows,
    ),
    (
        "pipewire_is_real_only_with_its_tools_and_server",
        pipewire_is_real_only_with_its_tools_and_server,
    ),
    (
        "a_check_that_cant_run_is_never_reported_as_passed",
        a_check_that_cant_run_is_never_reported_as_passed,
    ),
];

const FAKES: [&str; 6] = [
    "pactl",
    "systemctl",
    "pw-dump",
    "pw-record",
    "pw-play",
    "opendeepcool-audio",
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let me = PathBuf::from(&args[0]);
    if let Some(name) = me.file_name().and_then(|n| n.to_str())
        && FAKES.contains(&name)
    {
        process::exit(fake(&me, name, &args[1..]));
    }
    let filter = args.iter().skip(1).find(|arg| !arg.starts_with('-'));
    let mut failed = Vec::new();
    let mut passed = 0;
    for (name, test) in TESTS {
        if filter.is_some_and(|f| !name.contains(f.as_str())) {
            continue;
        }
        if panic::catch_unwind(test).is_ok() {
            passed += 1;
            println!("test {name} ... ok");
        } else {
            println!("test {name} ... FAILED");
            failed.push(name);
        }
    }
    let result = if failed.is_empty() { "ok" } else { "FAILED" };
    println!(
        "\ntest result: {result}. {passed} passed; {} failed",
        failed.len()
    );
    if !failed.is_empty() {
        process::exit(101);
    }
}

/// The fakes: each logs `<name> <args>` to `calls` in the case's folder, the `bin` folder's
/// parent, then answers from its files.
fn fake(me: &Path, name: &str, args: &[String]) -> i32 {
    let dir = me
        .parent()
        .and_then(Path::parent)
        .expect("the case's folder");
    let mut calls = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("calls"))
        .expect("the calls log");
    writeln!(calls, "{name} {}", args.join(" ")).expect("logged");
    drop(calls);
    let file = |name: &str| fs::read_to_string(dir.join(name)).unwrap_or_default();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match (name, args.as_slice()) {
        ("pactl", ["load-module", ..]) => print!("{}", file("load-module")),
        ("pactl", ["list", "short", "modules"]) => print!("{}", file("modules")),
        ("pactl", ["info"]) => println!("Server Name: PulseAudio (on PipeWire 1.6.9)"),
        ("systemctl", ["--user", "show", ..]) => println!("{}", file("load-state").trim()),
        ("systemctl", ["--user", "--quiet", "is-active", _]) => {
            return if file("active").trim() == "yes" { 0 } else { 3 };
        }
        ("systemctl", ["--user", "stop", _]) => {
            fs::write(dir.join("active"), "no").expect("the state");
        }
        ("systemctl", ["--user", "start", _]) => {
            fs::write(dir.join("active"), "yes").expect("the state");
        }
        ("pw-dump", []) => print!("{}", file("dump")),
        ("opendeepcool-audio", _) => {
            fs::write(dir.join("helper-pid"), process::id().to_string()).expect("the PID");
            // Until it's killed.
            thread::sleep(Duration::from_secs(60));
        }
        _ => {}
    }
    0
}

/// A case's folder with a `bin` of fakes; its roots' `bin` is that folder.
fn case(name: &str) -> (PathBuf, Roots) {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("music-tools")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).expect("a bin folder");
    let me = std::env::current_exe().expect("this program");
    for fake in FAKES {
        symlink(&me, bin.join(fake)).expect("a fake");
    }
    let roots = Roots {
        bin,
        ..Roots::system()
    };
    (dir, roots)
}

fn calls(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join("calls"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

// As read on the development PC, 2026-11-05: `pactl load-module` prints the index;
// `list short modules` tab-separates index, name and arguments, and a module's arguments may run
// over several lines; the null sink is one node named after it in `pw-dump`.
fn the_tools_outputs_are_read_as_pipewire_prints_them() {
    assert_eq!(module_index("536870916\n"), Some(536_870_916));
    assert_eq!(
        module_index("Failure: Module initialization failed\n"),
        None
    );
    let list = "1\tlibpipewire-module-rt\t{\n            nice.level    = -11\n\
                536870916\tmodule-null-sink\tsink_name=odc-e2e\t\n\
                536870917\tmodule-null-sink\tsink_name=odc-e2e-other\t\n\
                536870918\tmodule-loopback\tsink_name=odc-e2e\t\n";
    assert_eq!(leftover_sinks(list), [536_870_916]);
    let dump = r#"[{ "id": 85, "info": { "props": { "node.name": "odc-e2e", "media.class": "Audio/Sink" } } }]"#;
    assert!(has_node(dump, "odc-e2e"));
    assert!(!has_node(dump, "opendeepcool-audio"));
    let outputs = |percent: &str| {
        format!(
            "Source Output #4380\n\tDriver: PipeWire\n\tVolume: front-left: 65536 / {percent} / \
             0.00 dB,   front-right: 65536 / {percent} / 0.00 dB\n\tProperties:\n\t\t\
             node.name = \"opendeepcool-audio\"\n"
        )
    };
    assert_eq!(capture_at_full_volume(&outputs("100%")), Some(true));
    assert_eq!(capture_at_full_volume(&outputs("91%")), Some(false));
    assert_eq!(capture_at_full_volume("Source Output #1\n"), None);
    // Not known yet: the stream has only just joined.
    let joining = "Source Output #4380\n\tVolume: (invalid)\n\tProperties:\n\t\t\
                   node.name = \"opendeepcool-audio\"\n";
    assert_eq!(capture_at_full_volume(joining), None);
    let sorted: Vec<f64> = (1..=200).map(f64::from).collect();
    assert_eq!(quantile(&sorted, 0.5), 100.0);
    assert_eq!(quantile(&sorted, 0.95), 190.0);
    assert_eq!(quantile(&sorted, 0.99), 198.0);
}

fn the_start_repairs_what_a_killed_run_left() {
    let (dir, roots) = case("repair");
    fs::write(
        dir.join("modules"),
        "7\tmodule-null-sink\tsink_name=odc-e2e\t\n8\tmodule-null-sink\tsink_name=other\t\n",
    )
    .expect("the modules");
    fs::write(dir.join("load-state"), "loaded").expect("the unit");
    fs::write(dir.join("active"), "no").expect("stopped");
    let tools = Tools::new(&roots);
    let notes = repair(&tools).expect("repaired");
    assert_eq!(notes.len(), 2, "{notes:?}");
    let made = calls(&dir);
    assert!(
        made.contains(&"pactl unload-module 7".to_owned()),
        "{made:?}"
    );
    assert!(!made.iter().any(|c| c.contains("unload-module 8")));
    assert!(made.contains(&format!("systemctl --user start {HELPER_UNIT}")));
    // Nothing left over: nothing to do.
    fs::write(
        dir.join("modules"),
        "8\tmodule-null-sink\tsink_name=other\t\n",
    )
    .expect("modules");
    fs::remove_file(dir.join("calls")).expect("cleared");
    assert!(repair(&tools).expect("checked").is_empty());
    assert!(
        !calls(&dir)
            .iter()
            .any(|c| c.contains("unload") || c.contains(" start "))
    );
    // A PC without the helper's unit has nothing to start.
    fs::write(dir.join("load-state"), "not-found").expect("no unit");
    fs::write(dir.join("active"), "no").expect("stopped");
    assert!(repair(&tools).expect("checked").is_empty());
}

fn gone(pid: &str) -> bool {
    !Path::new(&format!("/proc/{pid}")).exists()
}

fn the_harness_sets_everything_back_even_when_a_check_panics() {
    let (dir, roots) = case("harness");
    fs::write(dir.join("load-module"), "41\n").expect("an index");
    fs::write(dir.join("active"), "yes").expect("running");
    let tools = Tools::new(&roots);
    let helper = roots.bin.join("opendeepcool-audio");
    let log = dir.join("helper.log");
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let _harness = Harness::start(&tools, &helper, &log).expect("started");
        let until = Instant::now() + Duration::from_secs(3);
        while !dir.join("helper-pid").exists() && Instant::now() < until {
            thread::sleep(Duration::from_millis(10));
        }
        panic!("a check failed");
    }));
    assert!(result.is_err(), "the check panicked");
    let made = calls(&dir);
    let at = |call: &str| {
        made.iter()
            .position(|c| c.starts_with(call))
            .unwrap_or_else(|| panic!("{call} in {made:?}"))
    };
    assert!(
        at("pactl load-module module-null-sink sink_name=odc-e2e") < at("systemctl --user stop")
    );
    assert!(at("systemctl --user stop") < at("opendeepcool-audio --target odc-e2e"));
    assert!(at("pactl unload-module 41") < at("systemctl --user start"));
    let pid = fs::read_to_string(dir.join("helper-pid")).expect("the helper ran");
    assert!(gone(pid.trim()), "the dev helper was stopped");
    assert_eq!(
        fs::read_to_string(dir.join("active")).expect("state"),
        "yes"
    );
    // An owner's helper that wasn't running stays stopped.
    fs::remove_file(dir.join("calls")).expect("cleared");
    fs::write(dir.join("active"), "no").expect("stopped");
    drop(Harness::start(&tools, &helper, &log).expect("started"));
    assert!(!calls(&dir).iter().any(|c| c.contains(" start ")));
}

/// A frame whose column 7 is `height` rows high.
fn column_7(height: usize) -> Frame {
    let mut frame = Frame::blank();
    for c in 1..=14 {
        frame.set(14, c, true);
    }
    for h in 1..=height {
        frame.set(15 - h, 7, true);
    }
    frame
}

fn each_delay_is_the_first_frame_that_shows_its_burst() {
    let ms = |n: u64| n * 1_000_000;
    let frames = vec![
        (ms(990), column_7(9)),
        (ms(1_090), column_7(1)),
        (ms(1_190), column_7(5)),
        (ms(1_290), column_7(8)),
        (ms(1_490), column_7(1)),
        (ms(1_590), column_7(6)),
    ];
    let found = delays(&frames, &[ms(1_100), ms(1_500)]).expect("both shown");
    assert_eq!(found, [90.0, 90.0]);
    let missing = delays(&frames, &[ms(1_100), ms(1_500), ms(1_600)]);
    assert!(missing.is_err(), "the third burst shows in no frame");
}

// Without PipeWire or the dev service, the tone runs through the engine in process; mode 13 is
// drawable only from Task 6, so the test-only switch draws it here.
fn in_process_the_tone_draws_its_expected_rows() {
    fn every_mode(_: Mode) -> bool {
        true
    }
    let note = fake_capture(every_mode).expect("as expected");
    assert!(note.contains("6 ticks as expected"), "{note}");
}

fn pipewire_is_real_only_with_its_tools_and_server() {
    let (_, roots) = case("probe");
    assert_eq!(probe(Capability::PipeWire, &roots), Probe::Real);
    fs::remove_file(roots.bin.join("pw-record")).expect("removed");
    assert!(matches!(
        probe(Capability::PipeWire, &roots),
        Probe::Absent { .. }
    ));
}

fn a_check_that_cant_run_is_never_reported_as_passed() {
    let mut summary = Summary::default();
    summary.skipped(
        "music_delay",
        "it needs the dev service and PipeWire on this PC",
    );
    assert!(summary.passed(), "not a failure");
    let table = summary.table();
    assert!(
        table.contains("music_delay      not run it needs"),
        "{table}"
    );
    assert!(!table.contains("passed"), "{table}");
}
