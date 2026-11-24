use std::collections::HashMap;

use super::*;

/// A machine in a map: what each program prints, each file holds, and each variable is.
#[derive(Default)]
struct Fake {
    programs: HashMap<String, Result<(String, String, bool), String>>,
    files: HashMap<&'static str, &'static str>,
    vars: HashMap<&'static str, &'static str>,
    systemd: bool,
    ran: Vec<String>,
}

impl Fake {
    /// A program that prints `stdout`, matched by its name and the first of its arguments
    /// that isn't a flag or `--user`'s: the key is `name` or `name --user`.
    fn prints(&mut self, key: &str, stdout: &str) {
        self.programs
            .insert(key.to_owned(), Ok((stdout.to_owned(), String::new(), true)));
    }
}

impl Runner for Fake {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<Output, String> {
        self.ran.push(format!("{program} {}", args.join(" ")));
        // The user's journal is a program of its own for the fake; the rest by name.
        let key = if program == "journalctl" && args.contains(&"--user") {
            "journalctl --user".to_owned()
        } else {
            program.to_owned()
        };
        match self.programs.get(&key) {
            Some(Ok((stdout, stderr, ok))) => Ok(Output {
                stdout: stdout.clone(),
                stderr: stderr.clone(),
                ok: *ok,
            }),
            Some(Err(reason)) => Err(reason.clone()),
            None => Err(format!("{program} isn't installed")),
        }
    }

    fn read(&mut self, path: &str) -> Result<String, String> {
        self.files
            .get(path)
            .map(|t| (*t).to_owned())
            .ok_or_else(|| format!("{path}: No such file or directory"))
    }

    fn exists(&mut self, _: &str) -> bool {
        self.systemd
    }

    fn var(&mut self, name: &str) -> Option<String> {
        self.vars.get(name).map(|v| (*v).to_owned())
    }
}

const MACHINE: &str = "0123456789abcdef0123456789abcdef";
const BOOT: &str = "fedcba9876543210fedcba9876543210";

/// A machine named `walnut-pc`, of the user `walnut`, with everything the report reads.
fn machine() -> Fake {
    let mut fake = Fake {
        systemd: true,
        ..Fake::default()
    };
    fake.files.insert(
        "/etc/os-release",
        "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\nBUILD_ID=rolling\n",
    );
    fake.files
        .insert("/proc/sys/kernel/osrelease", "7.3.4-arch1\n");
    fake.files
        .insert("/proc/sys/kernel/hostname", "walnut-pc\n");
    fake.files
        .insert("/etc/machine-id", "0123456789abcdef0123456789abcdef\n");
    fake.files.insert(
        "/proc/sys/kernel/random/boot_id",
        "fedcba9876543210fedcba9876543210\n",
    );
    fake.vars.insert("HOME", "/home/walnut");
    fake.vars.insert("USER", "walnut");
    fake.prints("pacman", "opendeepcool 0.1.0-2\n");
    fake.prints(
            "systemctl",
            "ActiveState=active\nSubState=running\nLoadState=loaded\nResult=success\nNRestarts=0\nUnitFileState=enabled\nExecMainStatus=0\n",
        );
    fake.prints(
            "journalctl",
            "2026-11-17T12:41:32-03:00 opendeepcoold[174400]: the cooler is connected\n2026-11-17T12:41:33-03:00 opendeepcoold[174400]: settings read from /home/walnut/x walnut-pc\n",
        );
    fake.prints(
        "journalctl --user",
        "2026-11-17T12:41:32-03:00 opendeepcool-audio[75682]: not capturing\n",
    );
    fake.prints(
            "coredumpctl",
            "TIME                           PID  UID  GID SIG     COREFILE EXE                    SIZE\nMon 2026-11-16 17:38:03 -03 219229 1000 1000 SIGABRT present  /usr/lib/opendeepcool/opendeepcoold  31.1K\nFri 2026-11-13 13:38:21 -03   1379 1000 1000 SIGSEGV present  /usr/bin/Hyprland  4.1M\n",
        );
    fake
}

fn titles(report: &str) -> Vec<&str> {
    report
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .collect()
}

#[test]
fn the_report_has_every_section_in_order() {
    let report = build(&mut machine());
    assert_eq!(
        titles(&report),
        [
            "Version",
            "System",
            "Units",
            "The service's journal",
            "The sound helper's journal",
            "Warnings and errors since the boot",
            "Crashes"
        ]
    );
    assert!(report.starts_with("# OpenDeepCool bug report"));
    assert!(report.contains("Nothing was sent"));
    assert!(report.contains("https://github.com/dtmarangoni/OpenDeepCool/issues"));
    assert!(report.contains(&format!(
        "opendeepcool {} (the program)\npackage: opendeepcool 0.1.0-2",
        env!("CARGO_PKG_VERSION")
    )));
    // The distribution's name and ID, kept; its build ID and pretty name, not asked for.
    assert!(report.contains("NAME=Arch Linux\nID=arch\nkernel: 7.3.4-arch1\nsystemd: running"));
    assert!(!report.contains("BUILD_ID") && !report.contains("PRETTY_NAME"));
    // Each unit, its properties in the report's order whatever order systemctl gave.
    assert!(report.contains(
        "opendeepcool.service: LoadState=loaded UnitFileState=enabled ActiveState=active \
             SubState=running Result=success NRestarts=0 ExecMainStatus=0"
    ));
    assert_eq!(
        report
            .matches("opendeepcool-audio.service: LoadState")
            .count(),
        1
    );
    assert!(report.contains("the cooler is connected"));
    assert!(report.contains("not capturing"));
}

#[test]
fn the_programs_run_with_fixed_arguments_and_the_user_ones_with_user() {
    let mut fake = machine();
    build(&mut fake);
    assert!(fake.ran.contains(&"pacman -Q opendeepcool".to_owned()));
    assert!(fake.ran.iter().any(|r| r
        == "journalctl -b -n 100 -o short-iso --no-hostname --no-pager -u opendeepcool.service \
                -u opendeepcool-gpu-rank.service"));
    assert!(fake.ran.iter().any(|r| r
            == "journalctl --user -b -n 40 -o short-iso --no-hostname --no-pager -u opendeepcool-audio.service"));
    assert!(
        fake.ran
            .iter()
            .any(|r| r.starts_with("systemctl --user show opendeepcool-audio.service"))
    );
    assert_eq!(
        fake.ran
            .iter()
            .filter(|r| r.starts_with("systemctl show "))
            .count(),
        2
    );
    assert!(fake.ran.contains(&"coredumpctl list --no-pager".to_owned()));
}

#[test]
fn what_identifies_the_machine_never_reaches_the_report() {
    let mut fake = machine();
    // Each in the text of a different program.
    fake.prints(
        "journalctl",
        &format!("a {MACHINE} b\nboot {BOOT}\n/home/walnut/.config/x\nwalnut-pc\nuser walnut ok\n"),
    );
    fake.prints("pacman", "opendeepcool 0.1.0-2 walnut-pc\n");
    let report = build(&mut fake);
    for private in [MACHINE, BOOT, "/home/walnut", "walnut-pc", "walnut"] {
        assert!(!report.contains(private), "{private}");
    }
    assert!(report.contains("a <machine-id> b"));
    assert!(report.contains("boot <boot-id>"));
    assert!(report.contains("~/.config/x"));
    assert!(report.contains("user <user> ok"));
    assert!(report.contains("opendeepcool 0.1.0-2 <host>"));
}

#[test]
fn the_scrub_replaces_in_one_pass_and_a_name_only_as_a_whole_word() {
    let who = Identity {
        home: Some("/home/al".to_owned()),
        machine_id: None,
        boot_id: None,
        host: Some("al-desktop".to_owned()),
        user: Some("al".to_owned()),
    };
    // The home folder first, then the host, then the user as a whole word; a replacement is
    // never read again, and `al` inside `alsa`, `final` and `al_x` stays.
    assert_eq!(
        who.scrub("/home/al/x al-desktop al alsa final al_x (al) \"al\""),
        "~/x <host> <user> alsa final al_x (<user>) \"<user>\""
    );
    // A short host name is a whole word too: `box` stays in `toolbox`.
    let short = Identity {
        host: Some("box".to_owned()),
        ..Identity::default()
    };
    assert_eq!(short.scrub("toolbox box boxes"), "toolbox <host> boxes");
    assert_eq!(
        Identity::default().scrub("nothing to hide"),
        "nothing to hide"
    );
    assert_eq!(who.scrub("é al é"), "é <user> é");
}

#[test]
fn the_superuser_and_a_one_letter_name_are_no_one() {
    for name in ["root", "a"] {
        let mut fake = Fake::default();
        fake.vars.insert("USER", name);
        fake.vars.insert("HOME", "/");
        let who = Identity::read(&mut fake);
        assert_eq!(
            who.scrub(&format!("{name} root a /")),
            format!("{name} root a /")
        );
    }
}

#[test]
fn a_missing_or_failing_program_leaves_one_line_and_the_other_sections() {
    let mut fake = machine();
    fake.programs.remove("systemctl");
    fake.programs.insert(
        "pacman".to_owned(),
        Ok((
            String::new(),
            "error: package 'opendeepcool' was not found\n".to_owned(),
            false,
        )),
    );
    fake.files.remove("/proc/sys/kernel/osrelease");
    let report = build(&mut fake);
    assert!(report.contains("opendeepcool.service: not available: systemctl isn't installed"));
    assert!(
        report.contains(
            "package: not available: pacman: error: package 'opendeepcool' was not found"
        )
    );
    assert!(report.contains("kernel: not available: /proc/sys/kernel/osrelease"));
    assert!(
        report.contains("the cooler is connected"),
        "the journal is still there"
    );
    assert_eq!(titles(&report).len(), 7);
}

#[test]
fn with_nothing_at_all_every_section_still_prints() {
    let report = build(&mut Fake::default());
    assert_eq!(titles(&report).len(), 7);
    assert_eq!(
        report.matches("not available").count(),
        // pacman, os-release, the kernel, three units, two journals, two lists of warnings
        // and coredumpctl.
        1 + 1 + 1 + 3 + 2 + 2 + 1
    );
    assert!(report.contains("systemd: not running"));
}

#[test]
fn an_unreadable_system_journal_says_what_to_do() {
    let mut fake = machine();
    fake.prints("journalctl", "-- No entries --\n");
    let report = build(&mut fake);
    assert!(report.contains("systemd-journal"));
    assert!(report.contains("sudo journalctl -u opendeepcool.service -b -n 100 --no-hostname"));
    assert!(
        report.contains("not capturing"),
        "the user's own journal is unaffected"
    );
}

#[test]
fn only_opendeepcools_crashes_are_listed() {
    let report = build(&mut machine());
    assert!(report.contains("opendeepcoold  31.1K"));
    assert!(report.contains("TIME "));
    assert!(!report.contains("Hyprland"));
    assert!(report.contains("Don't attach a core dump"));
    // Another program with a name that only starts like ours isn't ours.
    let mut fake = machine();
    fake.prints(
            "coredumpctl",
            "TIME PID UID GID SIG COREFILE EXE SIZE\nMon 2026-11-16 17:38:03 -03 1 1000 1000 SIGABRT present /usr/bin/opendeepcool-other 1K\nMon 2026-11-16 17:38:04 -03 2 1000 1000 SIGABRT present /tmp/x/odc-gpu-rank 2K\n",
        );
    let report = build(&mut fake);
    assert!(!report.contains("opendeepcool-other"));
    assert!(report.contains("odc-gpu-rank 2K"));
}

#[test]
fn the_warnings_of_the_boot_are_asked_for_in_both_journals() {
    let mut fake = machine();
    let report = build(&mut fake);
    assert!(report.contains("The service's units:\n"));
    assert!(report.contains("The sound helper:\n"));
    assert!(fake.ran.iter().any(|r| r.starts_with(
            "journalctl -b -p warning -n 50 -o short-iso --no-hostname --no-pager -u opendeepcool.service"
        )));
    assert!(fake.ran.iter().any(|r| r
            == "journalctl --user -b -p warning -n 20 -o short-iso --no-hostname --no-pager -u opendeepcool-audio.service"));
    // Nothing to show is said, not left empty.
    let mut quiet = machine();
    quiet.prints("journalctl", "-- No entries --\n");
    quiet.prints("journalctl --user", "");
    let report = build(&mut quiet);
    assert!(report.contains("none (or the journal can't be read: see above)"));
}

#[test]
fn no_crash_says_so() {
    let mut fake = machine();
    fake.prints("coredumpctl", "");
    assert!(build(&mut fake).contains("no core dump of an OpenDeepCool program is listed"));
}

#[test]
fn a_fence_is_longer_than_any_run_of_backticks_in_its_text() {
    let mut fake = machine();
    fake.prints("journalctl --user", "a ```` b\n");
    let report = build(&mut fake);
    assert!(report.contains("`````\na ```` b\n`````"));
}
