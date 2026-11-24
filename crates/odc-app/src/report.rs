//! `opendeepcool --report` (Phase 11 plan, Task 11; `docs/spec/report.md`): a bug report for the
//! user to read, change and post. It runs a few programs and reads two files, scrubs what a
//! machine would identify out of everything it printed, and sends nothing.
//!
//! The report asks its [`Runner`] for every program and file, so the tests give it a fake.

use std::fmt::Write as _;
use std::io::ErrorKind;
use std::process::{Command, Stdio};

/// Where the report is posted.
const ISSUES: &str = "https://github.com/dtmarangoni/OpenDeepCool/issues";

/// The system units the report asks about and takes the journal of.
const SYSTEM_UNITS: [&str; 2] = [odc_boot::SERVICE, "opendeepcool-gpu-rank.service"];

/// The sound helper's user unit.
const USER_UNIT: &str = odc_boot::HELPER;

/// What `systemctl show` is asked for, in the order the report prints it.
const PROPERTIES: [&str; 7] = [
    "LoadState",
    "UnitFileState",
    "ActiveState",
    "SubState",
    "Result",
    "NRestarts",
    "ExecMainStatus",
];

/// OpenDeepCool's programs, by file name: the only ones whose crashes the report lists.
const PROGRAMS: [&str; 5] = [
    "opendeepcoold",
    "opendeepcool",
    "opendeepcool-audio",
    "odc-gpu-rank",
    "opendeepcool-boot",
];

/// How many lines of each journal the report holds, and of its warnings and errors.
const SYSTEM_LINES: &str = "100";
const USER_LINES: &str = "40";
const SYSTEM_WARNINGS: &str = "50";
const USER_WARNINGS: &str = "20";

/// What a finished program printed.
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub ok: bool,
}

/// What the report needs from the machine.
pub trait Runner {
    /// Runs `program` with `args`, found on `PATH`, with no shell. `Err` says why it didn't run.
    fn run(&mut self, program: &str, args: &[&str]) -> Result<Output, String>;
    /// A file's text, or why it can't be read.
    fn read(&mut self, path: &str) -> Result<String, String>;
    /// Whether a path exists.
    fn exists(&mut self, path: &str) -> bool;
    /// An environment variable, which the report reads only to scrub what it holds.
    fn var(&mut self, name: &str) -> Option<String>;
}

/// The real machine.
pub struct System;

impl Runner for System {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<Output, String> {
        let output = Command::new(program)
            .args(args)
            // The messages the report reads are English, whatever the user's language.
            .env("LC_ALL", "C.UTF-8")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| match e.kind() {
                ErrorKind::NotFound => format!("{program} isn't installed"),
                _ => format!("{program}: {e}"),
            })?;
        Ok(Output {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            ok: output.status.success(),
        })
    }

    fn read(&mut self, path: &str) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
    }

    fn exists(&mut self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    fn var(&mut self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// What the machine would be known by, to be taken out of the report.
#[derive(Default)]
pub struct Identity {
    home: Option<String>,
    machine_id: Option<String>,
    boot_id: Option<String>,
    host: Option<String>,
    user: Option<String>,
}

impl Identity {
    /// Reads the home folder, the host name, the two IDs and the login name.
    pub fn read(runner: &mut impl Runner) -> Self {
        let mut text = |path: &str| {
            runner
                .read(path)
                .ok()
                .map(|t| t.trim().to_owned())
                .filter(|t| !t.is_empty())
        };
        let machine_id = text("/etc/machine-id");
        let boot_id = text("/proc/sys/kernel/random/boot_id");
        let host = text("/proc/sys/kernel/hostname");
        let home = runner.var("HOME").filter(|h| h.len() > 1);
        let user = runner
            .var("USER")
            .or_else(|| runner.var("LOGNAME"))
            .or_else(|| {
                home.as_deref()
                    .and_then(|h| h.rsplit('/').next())
                    .map(str::to_owned)
            })
            // The superuser's name is no one's.
            .filter(|u| u.len() >= 2 && u != "root");
        Identity {
            home,
            machine_id,
            boot_id,
            host,
            user,
        }
    }

    /// `text` with each of them replaced: the home folder, the IDs and the host name wherever
    /// they are, the host name and the login name only as whole words. One pass, so a
    /// replacement is never scrubbed again.
    pub fn scrub(&self, text: &str) -> String {
        let patterns: [(&Option<String>, &str, bool); 5] = [
            (&self.home, "~", false),
            (&self.machine_id, "<machine-id>", false),
            (&self.boot_id, "<boot-id>", false),
            (&self.host, "<host>", true),
            (&self.user, "<user>", true),
        ];
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        while at < text.len() {
            let rest = &text[at..];
            let hit = patterns.iter().find_map(|(name, replacement, whole)| {
                let name = name.as_deref()?;
                if !rest.starts_with(name) {
                    return None;
                }
                let before = text[..at].chars().next_back();
                let after = rest[name.len()..].chars().next();
                let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
                if *whole && (word(before) || word(after)) {
                    return None;
                }
                Some((name.len(), *replacement))
            });
            match hit {
                Some((length, replacement)) => {
                    out.push_str(replacement);
                    at += length;
                }
                None => {
                    let c = rest.chars().next().unwrap_or_default();
                    out.push(c);
                    at += c.len_utf8().max(1);
                }
            }
        }
        out
    }
}

/// The report's text.
pub fn build(runner: &mut impl Runner) -> String {
    let who = Identity::read(runner);
    let mut report = String::new();
    let _ = write!(
        report,
        "# OpenDeepCool bug report\n\nRead this before you post it. Nothing was sent. Remove \
         anything you don't want to share.\nPost it at {ISSUES}\n\n"
    );
    section(&mut report, "Version", &version(runner), &who);
    section(&mut report, "System", &system(runner), &who);
    section(&mut report, "Units", &units(runner), &who);
    section(
        &mut report,
        "The service's journal",
        &system_journal(runner),
        &who,
    );
    section(
        &mut report,
        "The sound helper's journal",
        &user_journal(runner),
        &who,
    );
    section(
        &mut report,
        "Warnings and errors since the boot",
        &warnings(runner),
        &who,
    );
    section(&mut report, "Crashes", &crashes(runner), &who);
    report
}

/// One section: its title, and its body scrubbed and fenced as code.
fn section(report: &mut String, title: &str, body: &str, who: &Identity) {
    let body = who.scrub(body);
    // A fence longer than any run of backticks in the text, so the text can't close it.
    let longest = body
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat(longest.max(2) + 1);
    let _ = write!(
        report,
        "## {title}\n\n{fence}\n{}\n{fence}\n\n",
        body.trim_end()
    );
}

/// `not available: <reason>`.
fn missing(reason: &str) -> String {
    format!("not available: {reason}")
}

fn version(runner: &mut impl Runner) -> String {
    let package = match runner.run("pacman", &["-Q", "opendeepcool"]) {
        Ok(out) if out.ok => out.stdout.trim().to_owned(),
        Ok(out) => missing(&format!("pacman: {}", out.stderr.trim())),
        Err(reason) => missing(&reason),
    };
    format!(
        "opendeepcool {} (the program)\npackage: {package}",
        env!("CARGO_PKG_VERSION")
    )
}

fn system(runner: &mut impl Runner) -> String {
    let os = match runner.read("/etc/os-release") {
        Ok(text) => ["NAME", "ID", "ID_LIKE"]
            .iter()
            .filter_map(|key| {
                text.lines().find_map(|line| {
                    let value = line.strip_prefix(key)?.strip_prefix('=')?;
                    Some(format!("{key}={}", value.trim_matches('"')))
                })
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Err(reason) => missing(&reason),
    };
    let kernel = match runner.read("/proc/sys/kernel/osrelease") {
        Ok(text) => text.trim().to_owned(),
        Err(reason) => missing(&reason),
    };
    let systemd = if runner.exists("/run/systemd/system") {
        "running"
    } else {
        "not running"
    };
    format!("{os}\nkernel: {kernel}\nsystemd: {systemd}")
}

fn units(runner: &mut impl Runner) -> String {
    let mut lines = Vec::new();
    let asks = SYSTEM_UNITS
        .iter()
        .map(|unit| (false, *unit))
        .chain([(true, USER_UNIT)]);
    for (user, unit) in asks {
        let mut args = Vec::new();
        if user {
            args.push("--user");
        }
        args.extend(["show", unit, "--no-pager"]);
        let properties: Vec<String> = PROPERTIES
            .iter()
            .map(|p| format!("--property={p}"))
            .collect();
        args.extend(properties.iter().map(String::as_str));
        lines.push(match runner.run("systemctl", &args) {
            Ok(out) if out.ok => {
                let shown: Vec<String> = PROPERTIES
                    .iter()
                    .filter_map(|key| {
                        out.stdout
                            .lines()
                            .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
                            .map(|value| format!("{key}={value}"))
                    })
                    .collect();
                format!("{unit}: {}", shown.join(" "))
            }
            Ok(out) => format!(
                "{unit}: {}",
                missing(&format!("systemctl: {}", out.stderr.trim()))
            ),
            Err(reason) => format!("{unit}: {}", missing(&reason)),
        });
    }
    lines.join("\n")
}

/// The log lines of a journal's output: not the blank ones, nor journalctl's own `-- …` notes.
fn journal_lines(out: &Output) -> Vec<&str> {
    out.stdout
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("-- "))
        .collect()
}

fn system_journal(runner: &mut impl Runner) -> String {
    let mut args = vec![
        "-b",
        "-n",
        SYSTEM_LINES,
        "-o",
        "short-iso",
        "--no-hostname",
        "--no-pager",
    ];
    for unit in SYSTEM_UNITS {
        args.extend(["-u", unit]);
    }
    match runner.run("journalctl", &args) {
        Ok(out) => {
            let lines = journal_lines(&out);
            if lines.is_empty() {
                // A regular user reads only their own journal (ArchWiki, systemd/Journal).
                missing(
                    "no line could be read. A regular user reads only their own journal: add \
                     the account to the systemd-journal group (adm and wheel can read it too), \
                     or run `sudo journalctl -u opendeepcool.service -b -n 100 --no-hostname` \
                     and paste its output here",
                )
            } else {
                lines.join("\n")
            }
        }
        Err(reason) => missing(&reason),
    }
}

fn user_journal(runner: &mut impl Runner) -> String {
    let args = [
        "--user",
        "-b",
        "-n",
        USER_LINES,
        "-o",
        "short-iso",
        "--no-hostname",
        "--no-pager",
        "-u",
        USER_UNIT,
    ];
    match runner.run("journalctl", &args) {
        Ok(out) => {
            let lines = journal_lines(&out);
            if lines.is_empty() {
                "no lines since the boot".to_owned()
            } else {
                lines.join("\n")
            }
        }
        Err(reason) => missing(&reason),
    }
}

/// The warnings and errors of the boot, from both journals: in a long session the lines that
/// matter have left the last 100.
fn warnings(runner: &mut impl Runner) -> String {
    let mut system = vec![
        "-b",
        "-p",
        "warning",
        "-n",
        SYSTEM_WARNINGS,
        "-o",
        "short-iso",
        "--no-hostname",
        "--no-pager",
    ];
    for unit in SYSTEM_UNITS {
        system.extend(["-u", unit]);
    }
    let user = [
        "--user",
        "-b",
        "-p",
        "warning",
        "-n",
        USER_WARNINGS,
        "-o",
        "short-iso",
        "--no-hostname",
        "--no-pager",
        "-u",
        USER_UNIT,
    ];
    let mut parts = Vec::new();
    for (name, args) in [
        ("The service's units", &system[..]),
        ("The sound helper", &user[..]),
    ] {
        let body = match runner.run("journalctl", args) {
            Ok(out) => {
                let lines = journal_lines(&out);
                if lines.is_empty() {
                    "none (or the journal can't be read: see above)".to_owned()
                } else {
                    lines.join("\n")
                }
            }
            Err(reason) => missing(&reason),
        };
        parts.push(format!("{name}:\n{body}"));
    }
    parts.join("\n\n")
}

/// The `coredumpctl list` lines of OpenDeepCool's programs, with the header; nothing of any other
/// program.
fn crashes(runner: &mut impl Runner) -> String {
    let out = match runner.run("coredumpctl", &["list", "--no-pager"]) {
        Ok(out) => out,
        Err(reason) => return missing(&reason),
    };
    let ours: Vec<&str> = out
        .stdout
        .lines()
        .filter(|line| {
            // The line ends with the program's path and its size.
            let mut words = line.split_whitespace().rev();
            words.next();
            words
                .next()
                .and_then(|path| path.rsplit('/').next())
                .is_some_and(|name| PROGRAMS.contains(&name))
        })
        .collect();
    if ours.is_empty() {
        return "no core dump of an OpenDeepCool program is listed".to_owned();
    }
    let header = out
        .stdout
        .lines()
        .find(|l| l.trim_start().starts_with("TIME"))
        .unwrap_or_default();
    format!(
        "{header}\n{}\nDon't attach a core dump: it is a copy of the program's memory.",
        ours.join("\n")
    )
}

#[cfg(test)]
mod tests {
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
            &format!(
                "a {MACHINE} b\nboot {BOOT}\n/home/walnut/.config/x\nwalnut-pc\nuser walnut ok\n"
            ),
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
        assert!(report.contains(
            "package: not available: pacman: error: package 'opendeepcool' was not found"
        ));
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
}
