//! `odc-sensors-detect` against fake roots (Phase 6 plan, Task 10). This program is also the fake
//! `sensors-detect` and the fake `systemctl` (AGENTS.md rule 13): each case links both names to it
//! in the case's folder, and it plays the one its name says, as that folder's `scenario` file
//! tells it. Nothing here touches the real `/etc`, lm-sensors or systemd.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};
use std::panic;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};

use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{BOARD_CHIP, amdgpu_dgpu, board_fans};

const TESTS: [(&str, fn()); 13] = [
    (
        "a_fresh_pc_runs_it_once_and_records_the_configuration",
        a_fresh_pc_runs_it_once_and_records_the_configuration,
    ),
    (
        "fan_readings_that_exist_already_leave_nothing_to_find",
        fan_readings_that_exist_already_leave_nothing_to_find,
    ),
    (
        "a_graphics_cards_fan_alone_doesnt_count",
        a_graphics_cards_fan_alone_doesnt_count,
    ),
    (
        "an_existing_configuration_is_never_touched",
        an_existing_configuration_is_never_touched,
    ),
    (
        "existing_module_options_alone_are_never_touched",
        existing_module_options_alone_are_never_touched,
    ),
    (
        "a_run_that_writes_module_options_records_both_files",
        a_run_that_writes_module_options_records_both_files,
    ),
    (
        "a_replaced_board_runs_it_again",
        a_replaced_board_runs_it_again,
    ),
    (
        "a_failing_script_is_recorded_all_the_same",
        a_failing_script_is_recorded_all_the_same,
    ),
    (
        "a_state_directory_that_isnt_this_accounts_alone_is_refused",
        a_state_directory_that_isnt_this_accounts_alone_is_refused,
    ),
    (
        "a_record_that_is_a_link_is_refused_and_never_followed",
        a_record_that_is_a_link_is_refused_and_never_followed,
    ),
    (
        "an_enabled_service_and_a_broken_record_are_handled",
        an_enabled_service_and_a_broken_record_are_handled,
    ),
    (
        "help_unknown_flags_and_missing_values",
        help_unknown_flags_and_missing_values,
    ),
    (
        "both_helper_units_have_their_time_limits_and_the_detection_doesnt_hold_up_the_service",
        both_helper_units_have_their_time_limits_and_the_detection_doesnt_hold_up_the_service,
    ),
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let me = PathBuf::from(&args[0]);
    match me.file_name().and_then(|name| name.to_str()) {
        Some("sensors-detect") => process::exit(fake_sensors_detect(&me, &args[1..])),
        Some("systemctl") => process::exit(fake_systemctl(&me, &args[1..])),
        _ => {}
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

/// Appends `line` to `path`.
fn append(path: &Path, line: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("a log file");
    writeln!(file, "{line}").expect("a log line");
}

/// The fake `sensors-detect`: it logs each start to `ran`, then follows `scenario`: `etc <dir>`
/// names the fake `/etc`, `write conf.d` and `write modprobe.d` write lm-sensors' two files there,
/// and `exit <n>` sets its exit code.
fn fake_sensors_detect(me: &Path, args: &[String]) -> i32 {
    let dir = me.parent().expect("the case's folder");
    append(&dir.join("ran"), &args.join(" "));
    if args != ["--auto"] {
        return 99;
    }
    let scenario = fs::read_to_string(dir.join("scenario")).unwrap_or_default();
    let mut etc = PathBuf::new();
    let mut code = 0;
    for line in scenario.lines() {
        match line.split_once(' ') {
            Some(("etc", path)) => etc = PathBuf::from(path),
            Some(("write", "conf.d")) => fs::write(
                etc.join("conf.d/lm_sensors"),
                format!("HWMON_MODULES=\"{BOARD_CHIP}\"\n"),
            )
            .expect("the configuration"),
            Some(("write", "modprobe.d")) => fs::write(
                etc.join("modprobe.d/lm_sensors.conf"),
                format!("options {BOARD_CHIP} force=1\n"),
            )
            .expect("the module options"),
            Some(("exit", n)) => code = n.parse().unwrap_or(98),
            _ => {}
        }
    }
    code
}

/// The fake `systemctl`: it logs each call to `systemctl-calls`, and answers `is-enabled
/// lm_sensors.service` with `enabled` when the case's folder has a file `enabled`.
fn fake_systemctl(me: &Path, args: &[String]) -> i32 {
    let dir = me.parent().expect("the case's folder");
    append(&dir.join("systemctl-calls"), &args.join(" "));
    if args != ["is-enabled", "lm_sensors.service"] {
        return 99;
    }
    if dir.join("enabled").exists() {
        println!("enabled");
        0
    } else {
        println!("disabled");
        1
    }
}

/// One case: its fake sysfs tree with a made-up board, a fake `/etc`, the state directory (not
/// made: the program makes it), and the two fakes linked into its folder.
struct Case {
    dir: PathBuf,
    tree: FakeTree,
    etc: PathBuf,
    state: PathBuf,
}

impl Case {
    fn new(name: &str) -> Case {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("detect")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        let tree = FakeTree::create(dir.join("tree")).expect("a fake tree");
        let etc = dir.join("etc");
        for sub in ["conf.d", "modprobe.d"] {
            fs::create_dir_all(etc.join(sub)).expect("a fake /etc");
        }
        let me = std::env::current_exe().expect("this program");
        for fake in ["sensors-detect", "systemctl"] {
            symlink(&me, dir.join(fake)).expect("a fake");
        }
        let case = Case {
            state: dir.join("state"),
            dir,
            tree,
            etc,
        };
        case.board("FAKE-1");
        case.scenario("write conf.d\nexit 0\n");
        case
    }

    /// The board's DMI identity, with a made-up vendor and version.
    fn board(&self, name: &str) {
        for (file, value) in [
            ("board_vendor", "Made-up Boards"),
            ("board_name", name),
            ("board_version", "1.0"),
        ] {
            self.tree
                .file(format!("sys/class/dmi/id/{file}"), format!("{value}\n"))
                .expect("a DMI file");
        }
    }

    fn scenario(&self, lines: &str) {
        fs::write(
            self.dir.join("scenario"),
            format!("etc {}\n{lines}", self.etc.display()),
        )
        .expect("the scenario");
    }

    fn run_with_state(&self, state: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_odc-sensors-detect"))
            .arg("--etc")
            .arg(&self.etc)
            .arg("--sys")
            .arg(self.tree.roots().sysfs)
            .arg("--state-dir")
            .arg(state)
            .arg("--sensors-detect")
            .arg(self.dir.join("sensors-detect"))
            .arg("--systemctl")
            .arg(self.dir.join("systemctl"))
            .output()
            .expect("the program runs")
    }

    fn run(&self) -> Output {
        self.run_with_state(&self.state)
    }

    /// How many times the fake `sensors-detect` started.
    fn runs(&self) -> usize {
        fs::read_to_string(self.dir.join("ran")).map_or(0, |log| log.lines().count())
    }

    fn record(&self) -> Option<String> {
        fs::read_to_string(self.state.join("sensors-detect.json")).ok()
    }

    fn remove_configuration(&self) {
        for file in ["conf.d/lm_sensors", "modprobe.d/lm_sensors.conf"] {
            let _ = fs::remove_file(self.etc.join(file));
        }
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Asserts a run succeeded, and returns what it printed.
fn succeeded(output: &Output) -> String {
    let all = format!("{}{}", text(&output.stdout), text(&output.stderr));
    assert!(output.status.success(), "{all}");
    all
}

/// Asserts a run was refused with `reason`.
fn refused(output: &Output, reason: &str) {
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains(reason), "{stderr}");
}

/// The exact record a run writes: the fixed fields, never a path. The date is the run's.
fn expected_record(record: &str, name: &str, conf_d: bool, modprobe_d: bool, enabled: bool) {
    let date = record
        .lines()
        .find_map(|line| line.strip_prefix("  \"date\": \""))
        .and_then(|rest| rest.strip_suffix("\","))
        .expect("a date line");
    assert!(
        date.len() == 10
            && date.bytes().enumerate().all(|(i, b)| match i {
                4 | 7 => b == b'-',
                _ => b.is_ascii_digit(),
            }),
        "{date:?}"
    );
    assert_eq!(
        record,
        format!(
            "{{\n  \"version\": 1,\n  \"board_vendor\": \"Made-up Boards\",\n  \"board_name\": \
             \"{name}\",\n  \"board_version\": \"1.0\",\n  \"date\": \"{date}\",\n  \
             \"created_conf_d\": {conf_d},\n  \"created_modprobe_d\": {modprobe_d},\n  \
             \"lm_sensors_enabled_before\": {enabled}\n}}\n"
        )
    );
}

fn a_fresh_pc_runs_it_once_and_records_the_configuration() {
    let case = Case::new("fresh");
    succeeded(&case.run());
    assert_eq!(case.runs(), 1);
    let record = case.record().expect("a record");
    expected_record(&record, "FAKE-1", true, false, false);
    let meta = fs::metadata(&case.state).expect("the state directory");
    assert_eq!(meta.mode() & 0o7777, 0o700);
    // With its configuration, then without it on the same board: never again.
    let note = succeeded(&case.run());
    assert!(note.contains("set up already"), "{note}");
    case.remove_configuration();
    let note = succeeded(&case.run());
    assert!(note.contains("once per motherboard"), "{note}");
    assert_eq!(case.runs(), 1);
    assert_eq!(case.record(), Some(record));
}

fn fan_readings_that_exist_already_leave_nothing_to_find() {
    let case = Case::new("fans-exist");
    board_fans(&case.tree, BOARD_CHIP, &[(1, 860, None), (2, 3_400, None)]).expect("board fans");
    let note = succeeded(&case.run());
    assert!(note.contains("has fan inputs already"), "{note}");
    assert_eq!(case.runs(), 0);
    assert!(!case.state.exists(), "nothing made or recorded");
}

fn a_graphics_cards_fan_alone_doesnt_count() {
    let case = Case::new("card-fan");
    amdgpu_dgpu(&case.tree, "0000:0a:00.0", 41_000, 37, 182_000_000).expect("a card with a fan");
    succeeded(&case.run());
    assert_eq!(case.runs(), 1);
    assert!(case.record().is_some());
}

fn an_existing_configuration_is_never_touched() {
    let case = Case::new("conf-d-exists");
    let mine = b"# the owner's own\nHWMON_MODULES=\"mine\"\n";
    let file = case.etc.join("conf.d/lm_sensors");
    fs::write(&file, mine).expect("an existing configuration");
    succeeded(&case.run());
    assert_eq!(case.runs(), 0);
    assert!(!case.state.exists(), "nothing recorded");
    assert_eq!(fs::read(&file).expect("the file"), mine);
}

fn existing_module_options_alone_are_never_touched() {
    let case = Case::new("modprobe-d-exists");
    let mine = b"options mine force=1\n";
    let file = case.etc.join("modprobe.d/lm_sensors.conf");
    fs::write(&file, mine).expect("existing module options");
    succeeded(&case.run());
    assert_eq!(case.runs(), 0);
    assert!(!case.etc.join("conf.d/lm_sensors").exists());
    assert_eq!(fs::read(&file).expect("the file"), mine);
}

fn a_run_that_writes_module_options_records_both_files() {
    let case = Case::new("both-files");
    case.scenario("write conf.d\nwrite modprobe.d\nexit 0\n");
    succeeded(&case.run());
    expected_record(
        &case.record().expect("a record"),
        "FAKE-1",
        true,
        true,
        false,
    );
}

fn a_replaced_board_runs_it_again() {
    let case = Case::new("new-board");
    succeeded(&case.run());
    case.remove_configuration();
    case.board("FAKE-2");
    succeeded(&case.run());
    assert_eq!(case.runs(), 2);
    expected_record(
        &case.record().expect("a record"),
        "FAKE-2",
        true,
        false,
        false,
    );
}

fn a_failing_script_is_recorded_all_the_same() {
    let case = Case::new("script-fails");
    case.scenario("write conf.d\nexit 1\n");
    let note = succeeded(&case.run());
    assert!(
        note.contains("<4>odc-sensors-detect: sensors-detect ended with"),
        "{note}"
    );
    expected_record(
        &case.record().expect("a record"),
        "FAKE-1",
        true,
        false,
        false,
    );
    let nothing = Case::new("script-fails-early");
    nothing.scenario("exit 2\n");
    succeeded(&nothing.run());
    expected_record(
        &nothing.record().expect("a record"),
        "FAKE-1",
        false,
        false,
        false,
    );
}

/// The effective user ID, from `/proc/self/status`.
fn effective_uid() -> u32 {
    fs::read_to_string("/proc/self/status")
        .expect("the process status")
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|ids| ids.split_whitespace().nth(1))
        .and_then(|id| id.parse().ok())
        .expect("an effective uid")
}

fn a_state_directory_that_isnt_this_accounts_alone_is_refused() {
    let case = Case::new("state-open");
    fs::DirBuilder::new()
        .mode(0o755)
        .create(&case.state)
        .expect("an open state directory");
    fs::set_permissions(&case.state, fs::Permissions::from_mode(0o755)).expect("mode 755");
    refused(&case.run(), "has mode 755, not 700: refused");
    let linked = case.dir.join("state-link");
    symlink(&case.state, &linked).expect("a link to a directory");
    refused(&case.run_with_state(&linked), "isn't a directory: refused");
    // Another owner: as root (CI), a directory given away; otherwise one root owns.
    let other = if effective_uid() == 0 {
        let other = case.dir.join("state-other");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&other)
            .expect("a state directory");
        std::os::unix::fs::chown(&other, Some(65_534), Some(65_534)).expect("given away");
        other
    } else {
        PathBuf::from("/")
    };
    refused(&case.run_with_state(&other), "is owned by uid");
    assert_eq!(case.runs(), 0, "nothing ran");
    assert!(!case.dir.join("systemctl-calls").exists(), "nothing asked");
    assert_eq!(
        fs::read_dir(&case.state).expect("the directory").count(),
        0,
        "nothing written"
    );
}

fn a_record_that_is_a_link_is_refused_and_never_followed() {
    let case = Case::new("record-link");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&case.state)
        .expect("a state directory");
    let elsewhere = case.dir.join("elsewhere.json");
    fs::write(&elsewhere, "untouched").expect("a file elsewhere");
    symlink(&elsewhere, case.state.join("sensors-detect.json")).expect("a linked record");
    refused(&case.run(), "isn't a plain file: refused");
    assert_eq!(case.runs(), 0);
    assert_eq!(
        fs::read_to_string(&elsewhere).expect("the target"),
        "untouched"
    );
    assert!(
        fs::symlink_metadata(case.state.join("sensors-detect.json"))
            .expect("the link")
            .file_type()
            .is_symlink()
    );
}

fn an_enabled_service_and_a_broken_record_are_handled() {
    let case = Case::new("enabled");
    fs::write(case.dir.join("enabled"), "").expect("the service enabled");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&case.state)
        .expect("a state directory");
    fs::write(case.state.join("sensors-detect.json"), "{ not a record").expect("a broken record");
    let note = succeeded(&case.run());
    assert!(note.contains("replaced"), "{note}");
    assert!(note.contains("was enabled before"), "{note}");
    assert_eq!(case.runs(), 1);
    assert_eq!(
        fs::read_to_string(case.dir.join("systemctl-calls")).expect("the calls"),
        "is-enabled lm_sensors.service\n"
    );
    expected_record(
        &case.record().expect("a record"),
        "FAKE-1",
        true,
        false,
        true,
    );
}

fn help_unknown_flags_and_missing_values() {
    let program = env!("CARGO_BIN_EXE_odc-sensors-detect");
    let help = Command::new(program).arg("--help").output().expect("runs");
    assert!(help.status.success());
    assert!(text(&help.stdout).starts_with("usage: odc-sensors-detect"));
    for (args, reason) in [
        (&["--fast"][..], "isn't an option"),
        (&["--etc"][..], "--etc needs a path"),
        (&["--state-dir"][..], "--state-dir needs a path"),
    ] {
        let output = Command::new(program).args(args).output().expect("runs");
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(text(&output.stderr).contains(reason), "{args:?}");
    }
}

/// Each `key=value` line of a unit in `packaging/systemd/`, with its section.
fn unit(name: &str) -> Vec<(String, String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/systemd")
        .join(name);
    let mut section = String::new();
    let mut lines = Vec::new();
    for line in fs::read_to_string(&path).expect("the unit").lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.to_owned();
            continue;
        }
        let (key, value) = line.split_once('=').expect("a key=value line");
        lines.push((section.clone(), key.to_owned(), value.to_owned()));
    }
    lines
}

fn values(lines: &[(String, String, String)], section: &str, key: &str) -> Vec<String> {
    lines
        .iter()
        .filter(|(s, k, _)| s == section && k == key)
        .map(|(_, _, v)| v.clone())
        .collect()
}

/// P6-02, P6-03, P6-14: both one-shots end within their limit, the rank helper makes its state
/// directory, and the detection runs as root, in its own root-only directory, only without an
/// lm-sensors setup, and never holds up the display service.
fn both_helper_units_have_their_time_limits_and_the_detection_doesnt_hold_up_the_service() {
    let rank = unit("opendeepcool-gpu-rank.service");
    assert_eq!(values(&rank, "Service", "TimeoutStartSec"), ["30"]);
    assert_eq!(values(&rank, "Service", "StateDirectory"), ["opendeepcool"]);
    let detect = unit("opendeepcool-sensors-detect.service");
    assert_eq!(values(&detect, "Service", "Type"), ["oneshot"]);
    assert_eq!(
        values(&detect, "Service", "ExecStart"),
        ["/usr/bin/odc-sensors-detect"]
    );
    assert!(values(&detect, "Service", "User").is_empty(), "root");
    assert_eq!(values(&detect, "Service", "TimeoutStartSec"), ["120"]);
    assert_eq!(
        values(&detect, "Service", "StateDirectory"),
        ["opendeepcool-detect"]
    );
    assert_eq!(values(&detect, "Service", "StateDirectoryMode"), ["0700"]);
    assert_eq!(
        values(&detect, "Unit", "ConditionPathExists"),
        [
            "!/etc/conf.d/lm_sensors",
            "!/etc/modprobe.d/lm_sensors.conf"
        ]
    );
    assert!(values(&detect, "Unit", "Before").is_empty());
    assert!(values(&detect, "Unit", "After").is_empty());
    assert_eq!(
        values(&detect, "Install", "WantedBy"),
        ["opendeepcool.service"]
    );
    for stream in ["StandardOutput", "StandardError"] {
        assert_eq!(values(&detect, "Service", stream), ["journal"], "{stream}");
    }
}
