//! The hardware probe every phase's end-to-end checks share (owner, 2026-10-24: the dev program
//! runs on other computers too). A device present and usable is tested for real, an absent one
//! through fakes, and one present but unusable fails with its fix, never falling back to fakes
//! (PRD, Testing Strategy, layer 4).

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use odc_device::HidrawConnector;

use crate::sysfs::{LP360_PRODUCT, LP360_VENDOR, usb_device};

/// What a check needs from the PC. Phases 6–11 add theirs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    /// The LP360's hidraw node, writable by this account.
    Lp360,
    /// The LP360's USB `authorized` switch, writable by this account (the dev reconnect rule).
    Lp360Reconnect,
}

impl Capability {
    pub const ALL: [Capability; 2] = [Capability::Lp360, Capability::Lp360Reconnect];

    /// The name `--require` takes.
    pub fn name(self) -> &'static str {
        match self {
            Capability::Lp360 => "lp360",
            Capability::Lp360Reconnect => "lp360-reconnect",
        }
    }

    pub fn from_name(name: &str) -> Option<Capability> {
        Capability::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// What the probe found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe {
    /// Present, and this account can use it.
    Real,
    /// Not present: the checks run against fakes.
    Absent { reason: String },
    /// Present but not usable: the checks fail with the fix.
    Unusable { reason: String, fix: String },
}

/// The roots the probe looks under: `/sys` and `/dev`, or a fake tree's.
#[derive(Clone, Debug)]
pub struct Roots {
    pub sysfs: PathBuf,
    pub dev: PathBuf,
}

impl Roots {
    pub fn system() -> Self {
        Roots {
            sysfs: PathBuf::from("/sys"),
            dev: PathBuf::from("/dev"),
        }
    }
}

const SETUP_FIX: &str = "run `cargo run -p odc-dev -- setup`, then log in again or use \
                         `newgrp opendeepcool`";

/// Finds a capability the way the product finds devices: by USB ID.
pub fn probe(capability: Capability, roots: &Roots) -> Probe {
    match capability {
        Capability::Lp360 => {
            let node = match HidrawConnector::new(&roots.sysfs, &roots.dev).find() {
                Ok(Some(node)) => node,
                Ok(None) => return absent("no LP360 (USB 3633:000c) is attached"),
                Err(e) => return unusable(&e.path, &e.error.to_string()),
            };
            writable(&node.dev)
        }
        Capability::Lp360Reconnect => match usb_device(&roots.sysfs, LP360_VENDOR, LP360_PRODUCT) {
            Some(usb) => writable(&usb.join("authorized")),
            None => absent("no LP360 USB device (3633:000c) is attached"),
        },
    }
}

/// Opening for writing proves access and writes nothing.
fn writable(path: &Path) -> Probe {
    match OpenOptions::new().write(true).open(path) {
        Ok(_) => Probe::Real,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            unusable(path, "this account may not write to it")
        }
        Err(e) => unusable(path, &e.to_string()),
    }
}

fn absent(reason: &str) -> Probe {
    Probe::Absent {
        reason: reason.to_owned(),
    }
}

fn unusable(path: &Path, why: &str) -> Probe {
    Probe::Unusable {
        reason: format!("{}: {why}", path.display()),
        fix: SETUP_FIX.to_owned(),
    }
}

/// How a capability ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Real,
    Fake { reason: String },
    Failed { reason: String },
}

/// Collects every capability's mode and every check's result, and prints them as one table at
/// the end of a run.
#[derive(Debug, Default)]
pub struct Summary {
    modes: Vec<(Capability, Mode)>,
    checks: Vec<(String, Result<String, String>)>,
}

impl Summary {
    /// Records how a capability ran; the first record wins.
    pub fn mode(&mut self, capability: Capability, mode: Mode) {
        if !self.modes.iter().any(|(c, _)| *c == capability) {
            self.modes.push((capability, mode));
        }
    }

    pub fn check(&mut self, name: &str, result: Result<String, String>) {
        self.checks.push((name.to_owned(), result));
    }

    pub fn passed(&self) -> bool {
        self.checks.iter().all(|(_, r)| r.is_ok())
            && self
                .modes
                .iter()
                .all(|(_, m)| !matches!(m, Mode::Failed { .. }))
    }

    /// The table, one line per check, then one per capability.
    pub fn table(&self) -> String {
        let mut out = String::from("check            result\n");
        for (name, result) in &self.checks {
            let line = match result {
                Ok(note) => format!("passed  {note}"),
                Err(reason) => format!("FAILED  {reason}"),
            };
            out.push_str(&format!("{name:<16} {line}\n"));
        }
        out.push_str("capability       mode\n");
        let mut modes = self.modes.clone();
        modes.sort_by_key(|(c, _)| *c);
        for (capability, mode) in modes {
            let line = match mode {
                Mode::Real => "real".to_owned(),
                Mode::Fake { reason } => format!("fake    {reason}"),
                Mode::Failed { reason } => format!("FAILED  {reason}"),
            };
            out.push_str(&format!("{:<16} {line}\n", capability.name()));
        }
        out
    }
}
