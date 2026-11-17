//! The real program's `--report` (Phase 11 plan, Task 11; `docs/spec/report.md`): with nothing on
//! `PATH`, it still prints every section, says what it couldn't read, and
//! exits 0; a flag beside it is refused with exit code 2.

use std::process::Command;

fn app() -> Command {
    // A folder with nothing in it for `PATH` (an unset `PATH` would make the system's default
    // search path apply), so no program is found; no display, no session bus, no home folder.
    let empty = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("report-empty-path");
    std::fs::create_dir_all(&empty).expect("an empty folder");
    let mut command = Command::new(env!("CARGO_BIN_EXE_opendeepcool"));
    command.env_clear().env("PATH", empty);
    command
}

#[test]
fn the_report_prints_every_section_with_no_program_on_path() {
    let output = app().arg("--report").output().expect("the program runs");
    assert!(output.status.success(), "{:?}", output.status);
    let text = String::from_utf8(output.stdout).expect("text");
    assert!(text.starts_with("# OpenDeepCool bug report\n"));
    for title in [
        "## Version",
        "## System",
        "## Units",
        "## The service's journal",
        "## The sound helper's journal",
        "## Warnings and errors since the boot",
        "## Crashes",
    ] {
        assert!(text.contains(title), "{title}");
    }
    // Nothing could run: each program says so, and the program's own version is still there.
    assert!(text.contains("pacman isn't installed"), "{text}");
    assert!(text.contains("systemctl isn't installed"));
    assert!(text.contains("journalctl isn't installed"));
    assert!(text.contains("coredumpctl isn't installed"));
    assert!(text.contains(&format!(
        "opendeepcool {} (the program)",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
}

#[test]
fn another_flag_beside_it_exits_2_and_prints_no_report() {
    let output = app()
        .args(["--report", "--pkexec", "/tmp/x"])
        .output()
        .expect("the program runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("--report is given alone"), "{error}");
}
