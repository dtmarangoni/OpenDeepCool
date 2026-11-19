//! `opendeepcool-boot` (Phase 11 plan, Task 3): every refusal, the `systemctl` commands for `on`
//! and `off` with systemd running and without, a failed step, the polkit action and the package's
//! files. This program is also the fake `systemctl`, `systemd-detect-virt` and `systemd-notify`
//! (AGENTS.md rule 13): each case links those names to it in its own `bin` folder, and each fake
//! logs its arguments, its environment's size and its folder to `calls`, then answers from the
//! case's files. Nothing here touches systemd.

use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::symlink;
use std::panic;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use odc_boot::{
    ACTION_OFF, ACTION_ON, FAILED, HELPER, PROGRAM, Programs, REFUSED, Refusal, Request, SERVICE,
    check, effective_uid, run, steps,
};

type Test = (&'static str, fn());

const TESTS: [Test; 10] = [
    (
        "every_refusal_comes_before_anything_runs",
        every_refusal_comes_before_anything_runs,
    ),
    (
        "the_program_refuses_a_caller_that_pkexec_didnt_start_as_root",
        the_program_refuses_a_caller_that_pkexec_didnt_start_as_root,
    ),
    (
        "on_enables_both_units_then_starts_the_service",
        on_enables_both_units_then_starts_the_service,
    ),
    (
        "off_disables_both_units_and_stops_the_service",
        off_disables_both_units_and_stops_the_service,
    ),
    (
        "in_a_chroot_or_an_unbooted_root_only_the_links_change",
        in_a_chroot_or_an_unbooted_root_only_the_links_change,
    ),
    (
        "a_failed_step_stops_the_rest_and_names_itself",
        a_failed_step_stops_the_rest_and_names_itself,
    ),
    (
        "a_check_that_cant_run_stops_before_any_step",
        a_check_that_cant_run_stops_before_any_step,
    ),
    (
        "the_effective_user_id_is_read_from_proc",
        the_effective_user_id_is_read_from_proc,
    ),
    (
        "the_polkit_actions_ask_the_administrator_at_every_change_and_say_which",
        the_polkit_actions_ask_the_administrator_at_every_change_and_say_which,
    ),
    (
        "the_program_and_its_units_are_the_packages",
        the_program_and_its_units_are_the_packages,
    ),
];

const FAKES: [&str; 3] = ["systemctl", "systemd-detect-virt", "systemd-notify"];

fn main() {
    let args: Vec<String> = env::args().collect();
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

/// The fakes: each logs `<name> <args> env=<n> cwd=<folder>` to `calls` in the case's folder, the
/// `bin` folder's parent. `systemd-detect-virt --chroot` exits 0 when the case has a `chroot`
/// file, `systemd-notify --booted` when it has a `booted` file, and `systemctl` fails the command
/// written in `fail`.
fn fake(me: &Path, name: &str, args: &[String]) -> i32 {
    let dir = me
        .parent()
        .and_then(Path::parent)
        .expect("the case's folder");
    let cwd = env::current_dir().expect("a folder");
    let mut calls = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("calls"))
        .expect("the calls log");
    writeln!(
        calls,
        "{name} {} env={} cwd={}",
        args.join(" "),
        env::vars_os().count(),
        cwd.display()
    )
    .expect("logged");
    let has = |file: &str| dir.join(file).exists();
    match (name, args.join(" ").as_str()) {
        ("systemd-detect-virt", "--chroot") => i32::from(!has("chroot")),
        ("systemd-notify", "--booted") => i32::from(!has("booted")),
        ("systemctl", command) => {
            let fail = fs::read_to_string(dir.join("fail")).unwrap_or_default();
            i32::from(fail.trim() == command)
        }
        _ => 99,
    }
}

/// A case's folder with a `bin` of fakes, and the programs pointing at them.
fn case(name: &str) -> (PathBuf, Programs) {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("boot")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).expect("a bin folder");
    let me = env::current_exe().expect("this program");
    for fake in FAKES {
        symlink(&me, bin.join(fake)).expect("a fake");
    }
    let programs = Programs {
        systemctl: bin.join("systemctl"),
        detect_virt: bin.join("systemd-detect-virt"),
        notify: bin.join("systemd-notify"),
    };
    (dir, programs)
}

/// The calls the fakes logged, in order.
fn calls(dir: &Path) -> Vec<String> {
    fs::read_to_string(dir.join("calls"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A logged call with an emptied environment, run from `/`.
fn call(name: &str, args: &str) -> String {
    format!("{name} {args} env=0 cwd=/")
}

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

// Rule 5: everything the entry point must refuse. `pkexec` lets its caller choose the arguments,
// so only `on` or `off`, alone, gets through, and only as root under `pkexec`.
fn every_refusal_comes_before_anything_runs() {
    let uid = Some(std::ffi::OsStr::new("1000"));
    assert_eq!(check(0, uid, &args(&["on"])), Ok(Request::On));
    assert_eq!(check(0, uid, &args(&["off"])), Ok(Request::Off));
    assert_eq!(
        check(1000, uid, &args(&["on"])),
        Err(Refusal::NotRoot { euid: 1000 })
    );
    assert_eq!(check(0, None, &args(&["on"])), Err(Refusal::NoPkexecUid));
    for forged in [
        "",
        "abc",
        "+1000",
        "-1",
        " 1000",
        "1000 ",
        "4294967296",
        "1e3",
    ] {
        assert_eq!(
            check(0, Some(std::ffi::OsStr::new(forged)), &args(&["on"])),
            Err(Refusal::BadPkexecUid),
            "PKEXEC_UID {forged:?}"
        );
    }
    let not_utf8 = OsString::from_vec(vec![0x31, 0xff]);
    assert_eq!(
        check(0, Some(&not_utf8), &args(&["on"])),
        Err(Refusal::BadPkexecUid)
    );
    assert_eq!(check(0, uid, &[]), Err(Refusal::Arguments { count: 0 }));
    assert_eq!(
        check(0, uid, &args(&["on", "off"])),
        Err(Refusal::Arguments { count: 2 })
    );
    assert_eq!(
        check(0, uid, &args(&["on", "--now"])),
        Err(Refusal::Arguments { count: 2 })
    );
    for other in [
        "ON",
        "On",
        "on ",
        " off",
        "--help",
        "-h",
        "enable",
        "/usr/bin/sh",
        "opendeepcool.service",
        "",
    ] {
        assert_eq!(
            check(0, uid, &args(&[other])),
            Err(Refusal::NotOnOrOff {
                given: other.to_owned()
            }),
            "{other:?}"
        );
    }
    let bytes = OsString::from_vec(vec![b'o', b'n', 0xff]);
    assert!(matches!(
        check(0, uid, &[bytes]),
        Err(Refusal::NotOnOrOff { .. })
    ));
    // The root check comes first, so a user learns nothing else from it.
    assert_eq!(check(1000, None, &[]), Err(Refusal::NotRoot { euid: 1000 }));
    for refusal in [
        Refusal::NotRoot { euid: 1000 },
        Refusal::NoPkexecUid,
        Refusal::BadPkexecUid,
        Refusal::Arguments { count: 2 },
        Refusal::NotOnOrOff { given: "x".into() },
    ] {
        assert!(refusal.to_string().contains("refused"), "{refusal}");
    }
}

// The built program itself, started without `pkexec`: as a user it isn't root, and as root (CI's
// container) it has no `PKEXEC_UID`. Either way it exits 2 before running anything.
fn the_program_refuses_a_caller_that_pkexec_didnt_start_as_root() {
    for list in [&["on"][..], &["off"], &[], &["on", "off"], &["--help"]] {
        let out = Command::new(env!("CARGO_BIN_EXE_opendeepcool-boot"))
            .args(list)
            .env_clear()
            .output()
            .expect("the program ran");
        assert_eq!(out.status.code(), Some(i32::from(REFUSED)), "{list:?}");
        let said = String::from_utf8_lossy(&out.stderr);
        assert!(
            said.contains("not root: refused") || said.contains("PKEXEC_UID isn't set: refused"),
            "{list:?}: {said}"
        );
        assert!(said.contains("usage: pkexec /usr/lib/opendeepcool/opendeepcool-boot on|off"));
        assert!(out.stdout.is_empty(), "{list:?}");
    }
}

fn on_enables_both_units_then_starts_the_service() {
    let (dir, programs) = case("on");
    fs::write(dir.join("booted"), "").expect("booted");
    let done = run(Request::On, &programs).expect("turned on");
    assert_eq!(done, "turned on at boot, and the service started");
    assert_eq!(
        calls(&dir),
        [
            call("systemd-detect-virt", "--chroot"),
            call("systemd-notify", "--booted"),
            call("systemctl", "enable opendeepcool.service"),
            call("systemctl", "--global enable opendeepcool-audio.service"),
            call("systemctl", "start opendeepcool.service"),
        ]
    );
}

fn off_disables_both_units_and_stops_the_service() {
    let (dir, programs) = case("off");
    fs::write(dir.join("booted"), "").expect("booted");
    let done = run(Request::Off, &programs).expect("turned off");
    assert_eq!(done, "turned off at boot, and the service stopped");
    assert_eq!(
        calls(&dir)[2..],
        [
            call("systemctl", "--global disable opendeepcool-audio.service"),
            call("systemctl", "disable --now opendeepcool.service"),
        ]
    );
}

// As Arch's `systemd-hook`: in a chroot, or a root systemd didn't boot, nothing starts or stops,
// so the package test can use the program in its throwaway root (Task 5).
fn in_a_chroot_or_an_unbooted_root_only_the_links_change() {
    let (dir, programs) = case("chroot");
    fs::write(dir.join("chroot"), "").expect("a chroot");
    fs::write(dir.join("booted"), "").expect("booted all the same");
    let done = run(Request::On, &programs).expect("links only");
    assert!(done.contains("nothing was started or stopped"), "{done}");
    assert_eq!(
        calls(&dir),
        [
            call("systemd-detect-virt", "--chroot"),
            call("systemctl", "enable opendeepcool.service"),
            call("systemctl", "--global enable opendeepcool-audio.service"),
        ]
    );
    let (dir, programs) = case("unbooted");
    run(Request::Off, &programs).expect("links only");
    assert_eq!(
        calls(&dir),
        [
            call("systemd-detect-virt", "--chroot"),
            call("systemd-notify", "--booted"),
            call("systemctl", "--global disable opendeepcool-audio.service"),
            call("systemctl", "disable opendeepcool.service"),
        ]
    );
    // Every command names exactly the two units, whatever the case.
    for request in [Request::On, Request::Off] {
        for live in [true, false] {
            for step in steps(request, live) {
                let units: Vec<&&str> = step.iter().filter(|a| a.ends_with(".service")).collect();
                assert!(
                    units == [&SERVICE] || units == [&HELPER],
                    "{request:?} {live}: {step:?}"
                );
            }
        }
    }
}

fn a_failed_step_stops_the_rest_and_names_itself() {
    let (dir, programs) = case("failed");
    fs::write(dir.join("booted"), "").expect("booted");
    fs::write(
        dir.join("fail"),
        "--global enable opendeepcool-audio.service",
    )
    .expect("a failure");
    let e = run(Request::On, &programs).expect_err("the second step fails");
    assert!(
        e.starts_with(
            "step 2 of 3, `systemctl --global enable opendeepcool-audio.service`, failed: exit status: 1"
        ),
        "{e}"
    );
    assert!(
        !calls(&dir).iter().any(|c| c.contains("start opendeepcool")),
        "nothing after it ran"
    );
}

fn a_check_that_cant_run_stops_before_any_step() {
    let (dir, mut programs) = case("no-check");
    programs.detect_virt = dir.join("bin/missing");
    let e = run(Request::On, &programs).expect_err("the check can't run");
    assert!(e.contains("bin/missing"), "{e}");
    assert!(calls(&dir).is_empty(), "{:?}", calls(&dir));
}

fn the_effective_user_id_is_read_from_proc() {
    let status = "Name:\topendeepcool-bo\nUid:\t1000\t0\t0\t0\nGid:\t1000\t1000\t1000\t1000\n";
    assert_eq!(effective_uid(status), Some(0));
    assert_eq!(effective_uid("Name:\tx\n"), None);
    assert_eq!(effective_uid("Uid:\t1000\n"), None);
    let mine = fs::read_to_string("/proc/self/status").expect("this process's status");
    assert!(effective_uid(&mine).is_some());
}

fn packaging(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging")
        .join(path)
}

/// The text from `start` up to the end of its `tag`.
fn block<'a>(text: &'a str, start: &str, tag: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{start} in the file"));
    let rest = &text[from..];
    let end = rest
        .find(&format!("</{tag}>"))
        .unwrap_or_else(|| panic!("the end of {start}"));
    &rest[..end]
}

// polkit(8) and pkexec(1): `pkexec` picks the action whose `exec.path` is the program's full path
// and, where it has an `exec.argv1`, whose value is the program's first argument: one action for
// `on` and one for `off`, so the prompt says what is asked (owner, 2026-11-19). Only the user at
// the PC, with the administrator's password every time (`auth_admin`, never `auth_admin_keep`),
// and no GUI variables for the program (`exec.allow_gui`).
fn the_polkit_actions_ask_the_administrator_at_every_change_and_say_which() {
    let text = fs::read_to_string(packaging(
        "polkit/io.github.dtmarangoni.OpenDeepCool1.policy",
    ))
    .expect("the policy");
    for (id, argument, message, description) in [
        (
            ACTION_ON,
            "on",
            "Automatically start the cooler service at boot.",
            "Start the cooler service automatically at boot",
        ),
        (
            ACTION_OFF,
            "off",
            "Disable automatic cooler service start at boot.",
            "Stop starting the cooler service automatically at boot",
        ),
    ] {
        let action = block(&text, &format!("<action id=\"{id}\">"), "action");
        for line in [
            "<allow_any>no</allow_any>",
            "<allow_inactive>no</allow_inactive>",
            "<allow_active>auth_admin</allow_active>",
            &format!("<message>{message}</message>"),
            &format!("<description>{description}</description>"),
            &format!("<annotate key=\"org.freedesktop.policykit.exec.path\">{PROGRAM}</annotate>"),
            &format!(
                "<annotate key=\"org.freedesktop.policykit.exec.argv1\">{argument}</annotate>"
            ),
        ] {
            assert!(action.contains(line), "{id}: {line}");
        }
        assert!(!action.contains("allow_gui"), "{id}");
    }
    // Two actions for the program, no third: an argument the program refuses has no action of its
    // own, so pkexec falls back to its generic one, and the program refuses it.
    assert_eq!(
        text.matches("org.freedesktop.policykit.exec.path").count(),
        2
    );
    assert_eq!(
        text.matches("org.freedesktop.policykit.exec.argv1").count(),
        2
    );
}

// The units the program turns on are the package's, and `FILES.md` installs the program where the
// action names it.
fn the_program_and_its_units_are_the_packages() {
    assert!(packaging(&format!("systemd/{SERVICE}")).is_file());
    assert!(packaging(&format!("systemd-user/{HELPER}")).is_file());
    let files = fs::read_to_string(packaging("FILES.md")).expect("the file list");
    assert!(
        files.contains(&format!(
            "| `{PROGRAM}` | program | root:root | 0755 | `target/release/opendeepcool-boot` |"
        )),
        "FILES.md installs {PROGRAM}"
    );
    assert_eq!(FAILED, 1);
    assert!(
        ![126, 127].contains(&REFUSED) && ![126, 127].contains(&FAILED),
        "pkexec's own"
    );
}
