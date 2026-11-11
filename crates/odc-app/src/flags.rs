//! The app's flags (Phase 10 plan, Task 4; Phase 11 plan, Task 3): only the five the tests give,
//! never environment variables (owner, 2026-10-24). Anything else is refused.

use std::path::PathBuf;

/// What the flags ask for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    /// The D-Bus address of the service's bus, in place of the system bus.
    pub service_address: Option<String>,
    /// The folder in place of `~/.local/state/omarchy`.
    pub omarchy_state: Option<PathBuf>,
    /// A file each preview frame the service sends is added to, for the end-to-end run's
    /// comparison with the dev recording (Phase 10 plan, Task 9; decisions log D84).
    pub frame_log: Option<PathBuf>,
    /// The D-Bus address of the user's own manager's bus, in place of the session bus: the
    /// tests' fake systemd (Start at boot).
    pub user_bus_address: Option<String>,
    /// The program run in place of `pkexec` for a Start at boot flip: the tests' fake, with the
    /// same arguments. It runs as the user, as the app does.
    pub pkexec: Option<PathBuf>,
}

pub const USAGE: &str = "usage: opendeepcool [--service-address ADDRESS] [--omarchy-state DIR] \
                         [--frame-log FILE] [--user-bus-address ADDRESS] [--pkexec PROGRAM]";

/// The flags' names.
const NAMES: [&str; 5] = [
    "--service-address",
    "--omarchy-state",
    "--frame-log",
    "--user-bus-address",
    "--pkexec",
];

impl Flags {
    /// Whether a test started the app.
    pub fn testing(&self) -> bool {
        self.service_address.is_some()
            || self.omarchy_state.is_some()
            || self.frame_log.is_some()
            || self.user_bus_address.is_some()
            || self.pkexec.is_some()
    }
}

/// The flags in `args`, the program's name left out: each as `--flag value` or
/// `--flag=value`, at most once.
///
/// # Errors
///
/// A line naming what was refused.
pub fn parse(args: &[String]) -> Result<Flags, String> {
    let mut flags = Flags::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) => (name, Some(value.to_owned())),
            None => (arg.as_str(), None),
        };
        if !NAMES.contains(&name) {
            return Err(format!("unknown argument {arg:?}"));
        }
        let value = match inline {
            Some(value) => value,
            None => rest
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))?,
        };
        if value.is_empty() {
            return Err(format!("{name} needs a value"));
        }
        let taken = match name {
            "--service-address" => flags.service_address.replace(value).is_some(),
            "--omarchy-state" => flags.omarchy_state.replace(PathBuf::from(value)).is_some(),
            "--frame-log" => flags.frame_log.replace(PathBuf::from(value)).is_some(),
            "--user-bus-address" => flags.user_bus_address.replace(value).is_some(),
            _ => flags.pkexec.replace(PathBuf::from(value)).is_some(),
        };
        if taken {
            return Err(format!("{name} given twice"));
        }
    }
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn the_flags_are_read_in_either_form() {
        assert_eq!(parse(&[]), Ok(Flags::default()));
        let want = Flags {
            service_address: Some("unix:path=/tmp/bus".to_owned()),
            omarchy_state: Some(PathBuf::from("/tmp/omarchy")),
            ..Flags::default()
        };
        assert_eq!(
            parse(&args(&[
                "--service-address",
                "unix:path=/tmp/bus",
                "--omarchy-state",
                "/tmp/omarchy"
            ])),
            Ok(want.clone())
        );
        assert_eq!(
            parse(&args(&[
                "--omarchy-state=/tmp/omarchy",
                "--service-address=unix:path=/tmp/bus"
            ])),
            Ok(want.clone())
        );
        assert!(want.testing());
        assert!(!Flags::default().testing());
        let frames = parse(&args(&["--frame-log", "/tmp/frames.log"])).expect("read");
        assert_eq!(frames.frame_log, Some(PathBuf::from("/tmp/frames.log")));
        assert!(frames.testing(), "a test's app never hands over");
        let boot = parse(&args(&[
            "--user-bus-address=unix:path=/tmp/user",
            "--pkexec",
            "/tmp/fake/pkexec",
        ]))
        .expect("read");
        assert_eq!(
            boot.user_bus_address.as_deref(),
            Some("unix:path=/tmp/user")
        );
        assert_eq!(boot.pkexec, Some(PathBuf::from("/tmp/fake/pkexec")));
        assert!(boot.testing());
    }

    #[test]
    fn anything_else_is_refused() {
        for bad in [
            &["--dev"][..],
            &["somefile"],
            &["--service-address"],
            &["--service-address="],
            &["--omarchy-state", "/a", "--omarchy-state", "/b"],
            &["--frame-log"],
            &["--frame-log", "/a", "--frame-log", "/b"],
            &["--pkexec"],
            &["--pkexec="],
            &["--pkexec", "/a", "--pkexec", "/b"],
            &["--user-bus-address", "a", "--user-bus-address=b"],
            &["--systemctl", "/a"],
            &["--gapplication-service"],
            &["-h"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
