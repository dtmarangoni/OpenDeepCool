//! What the dev tool needs from the system: who runs it, the system tools it may start, the files
//! it installs, and the cooler's device files.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use odc_device::{HidrawConnector, HidrawNode};
use odc_testkit::sysfs::{LP360_PRODUCT, LP360_VENDOR, usb_device};

/// The system tools the root steps may start, by full path and without a shell: never `cargo`
/// or any build (solution review P5-03).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    SystemdSysusers,
    Udevadm,
    Gpasswd,
    Getent,
}

impl Tool {
    /// Every tool, so a test can check the list.
    #[cfg(test)]
    pub const ALL: [Tool; 4] = [
        Tool::SystemdSysusers,
        Tool::Udevadm,
        Tool::Gpasswd,
        Tool::Getent,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Tool::SystemdSysusers => "/usr/bin/systemd-sysusers",
            Tool::Udevadm => "/usr/bin/udevadm",
            Tool::Gpasswd => "/usr/bin/gpasswd",
            Tool::Getent => "/usr/bin/getent",
        }
    }

    /// Runs the tool; a failure says the command and its status.
    pub fn run(self, args: &[&str]) -> Result<(), String> {
        let status = Command::new(self.path())
            .args(args)
            .status()
            .map_err(|e| format!("{}: {e}", self.path()))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{} {}: {status}", self.path(), args.join(" ")))
        }
    }

    /// Runs the tool and returns its standard output, or `None` for exit status 2, `getent`'s
    /// "not found".
    pub fn output(self, args: &[&str]) -> Result<Option<String>, String> {
        let out = Command::new(self.path())
            .args(args)
            .output()
            .map_err(|e| format!("{}: {e}", self.path()))?;
        match out.status.code() {
            Some(0) => Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned())),
            Some(2) => Ok(None),
            _ => Err(format!(
                "{} {}: {}",
                self.path(),
                args.join(" "),
                out.status
            )),
        }
    }
}

/// The effective user ID: the second number of `/proc/self/status`'s `Uid:` line (real,
/// effective, saved, file system; proc(5)).
pub fn parse_effective_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// The supplementary group IDs of `/proc/self/status`'s `Groups:` line: the groups this session
/// really has, which a new membership joins only at the next login.
pub fn parse_groups(status: &str) -> Vec<u32> {
    status
        .lines()
        .find_map(|l| l.strip_prefix("Groups:"))
        .map(|g| {
            g.split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

pub fn process_status() -> Result<String, String> {
    fs::read_to_string("/proc/self/status").map_err(|e| format!("/proc/self/status: {e}"))
}

pub fn effective_uid() -> Result<u32, String> {
    parse_effective_uid(&process_status()?)
        .ok_or_else(|| "no Uid: line in /proc/self/status".into())
}

/// The name of a user ID, from `getent passwd`.
pub fn user_name(uid: u32) -> Result<String, String> {
    let line = Tool::Getent
        .output(&["passwd", &uid.to_string()])?
        .ok_or_else(|| format!("no user with ID {uid}"))?;
    line.split(':')
        .next()
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("getent passwd {uid}: {line:?}"))
}

/// The checkout this tool was built from.
pub fn repo_root() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    here.canonicalize().unwrap_or(here)
}

/// A file the development setup installs, from the checkout.
pub struct Install {
    pub source: &'static str,
    pub target: &'static str,
}

/// The product's udev rule and group file, and the dev-only reconnect rule (owner, 2026-10-24).
pub const FILES: [Install; 3] = [
    Install {
        source: "packaging/sysusers/opendeepcool.conf",
        target: "/etc/sysusers.d/opendeepcool.conf",
    },
    Install {
        source: "packaging/udev/70-opendeepcool.rules",
        target: "/etc/udev/rules.d/70-opendeepcool.rules",
    },
    Install {
        source: "tools/dev/reconnect.rules",
        target: "/etc/udev/rules.d/71-opendeepcool-dev-reconnect.rules",
    },
];

/// The LP360's hidraw node, found by USB ID.
pub fn lp360_node() -> Result<Option<HidrawNode>, String> {
    HidrawConnector::system()
        .find()
        .map_err(|e| format!("{}: {}", e.path.display(), e.error))
}

/// The LP360's USB device folder, found by `idVendor` and `idProduct`.
pub fn lp360_usb_device() -> Option<PathBuf> {
    usb_device(Path::new("/sys"), LP360_VENDOR, LP360_PRODUCT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_effective_uid_and_groups_come_from_proc_status() {
        let status = "Name:\tx\nUid:\t1000\t0\t0\t0\nGroups:\t956 998 1000 \n";
        assert_eq!(parse_effective_uid(status), Some(0));
        assert_eq!(parse_groups(status), [956, 998, 1000]);
        assert_eq!(parse_effective_uid("Name:\tx\n"), None);
    }

    #[test]
    fn the_root_steps_may_start_only_these_system_tools() {
        assert_eq!(
            Tool::ALL.map(Tool::path),
            [
                "/usr/bin/systemd-sysusers",
                "/usr/bin/udevadm",
                "/usr/bin/gpasswd",
                "/usr/bin/getent",
            ]
        );
    }

    #[test]
    fn every_installed_file_is_one_the_manifest_accepts() {
        for file in FILES {
            assert!(
                odc_manifest::Entry::file(file.target).is_ok(),
                "{}",
                file.target
            );
            assert!(repo_root().join(file.source).is_file(), "{}", file.source);
        }
    }
}
