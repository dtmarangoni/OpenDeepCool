//! The app's flags (Phase 10 plan, Task 4): only the three the tests give, never environment
//! variables (owner, 2026-10-24). Anything else is refused.

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
}

pub const USAGE: &str =
    "usage: opendeepcool [--service-address ADDRESS] [--omarchy-state DIR] [--frame-log FILE]";

impl Flags {
    /// Whether a test started the app.
    pub fn testing(&self) -> bool {
        self.service_address.is_some() || self.omarchy_state.is_some() || self.frame_log.is_some()
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
        if !matches!(
            name,
            "--service-address" | "--omarchy-state" | "--frame-log"
        ) {
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
            _ => flags.frame_log.replace(PathBuf::from(value)).is_some(),
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
    fn the_two_flags_are_read_in_either_form() {
        assert_eq!(parse(&[]), Ok(Flags::default()));
        let want = Flags {
            service_address: Some("unix:path=/tmp/bus".to_owned()),
            omarchy_state: Some(PathBuf::from("/tmp/omarchy")),
            frame_log: None,
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
            &["--gapplication-service"],
            &["-h"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
