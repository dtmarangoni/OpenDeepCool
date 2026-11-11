//! `opendeepcool-boot`, the settings app's Start at boot switch (`docs/spec/settings.md`, Start at
//! boot; Phase 11 plan, Task 3). It runs as root through `pkexec`, under the polkit action
//! [`ACTION`], and turns OpenDeepCool on at boot, or off, with systemd's own `systemctl`.
//!
//! `pkexec` lets whoever starts it choose the arguments, so the program takes exactly one, `on` or
//! `off`, and nothing else: no path, unit name or option ever comes from its caller, and the
//! programs it runs get an emptied environment. A refusal comes before anything runs. Only the
//! standard library is used, and no `unsafe` (the workspace forbids it). This library holds the
//! rules, so the tests run them with fake programs; `main` fixes the real ones.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

/// Where the package installs the program (`packaging/FILES.md`). The polkit action's
/// `org.freedesktop.policykit.exec.path` names it, so `pkexec` runs nothing else under the action,
/// and the settings app starts it by this path.
pub const PROGRAM: &str = "/usr/lib/opendeepcool/opendeepcool-boot";
/// The polkit action `pkexec` checks: the administrator's password at every change.
pub const ACTION: &str = "io.github.dtmarangoni.OpenDeepCool1.start-at-boot";
/// The display service, a system unit.
pub const SERVICE: &str = "opendeepcool.service";
/// The sound helper, a user unit, turned on for every user's session (`systemctl --global`).
pub const HELPER: &str = "opendeepcool-audio.service";

/// The exit code of a refusal. `pkexec` keeps 126 and 127 for itself (pkexec(1)).
pub const REFUSED: u8 = 2;
/// The exit code when a check or a step fails; it says which.
pub const FAILED: u8 = 1;

/// What the caller asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    On,
    Off,
}

impl Request {
    /// The argument that asks for it.
    pub fn arg(self) -> &'static str {
        match self {
            Request::On => "on",
            Request::Off => "off",
        }
    }
}

/// Why the program refuses to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// It isn't running as root.
    NotRoot { euid: u32 },
    /// It wasn't started by `pkexec`, which sets `PKEXEC_UID` to its caller's user ID.
    NoPkexecUid,
    /// `PKEXEC_UID` isn't a user ID.
    BadPkexecUid,
    /// It takes exactly one argument.
    Arguments { count: usize },
    /// The one argument isn't `on` or `off`.
    NotOnOrOff { given: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NotRoot { euid } => write!(
                f,
                "it runs as user {euid}, not root: refused (it's started by pkexec)"
            ),
            Refusal::NoPkexecUid => {
                write!(f, "PKEXEC_UID isn't set: refused (it's started by pkexec)")
            }
            Refusal::BadPkexecUid => write!(f, "PKEXEC_UID isn't a user ID: refused"),
            Refusal::Arguments { count } => {
                write!(f, "it takes one argument, on or off, not {count}: refused")
            }
            Refusal::NotOnOrOff { given } => write!(f, "{given:?} isn't on or off: refused"),
        }
    }
}

/// The usage line every refusal ends with.
pub const USAGE: &str = "usage: pkexec /usr/lib/opendeepcool/opendeepcool-boot on|off";

/// The checks, in order, before anything runs: root, started by `pkexec`, then one argument,
/// `on` or `off`, exactly.
///
/// # Errors
///
/// The first check that fails.
pub fn check(euid: u32, pkexec_uid: Option<&OsStr>, args: &[OsString]) -> Result<Request, Refusal> {
    if euid != 0 {
        return Err(Refusal::NotRoot { euid });
    }
    let uid = pkexec_uid.ok_or(Refusal::NoPkexecUid)?;
    // Digits only: `u32`'s parser would also take a leading `+`.
    let user_id = uid.to_str().is_some_and(|uid| {
        !uid.is_empty() && uid.bytes().all(|b| b.is_ascii_digit()) && uid.parse::<u32>().is_ok()
    });
    if !user_id {
        return Err(Refusal::BadPkexecUid);
    }
    let [arg] = args else {
        return Err(Refusal::Arguments { count: args.len() });
    };
    match arg.to_str() {
        Some("on") => Ok(Request::On),
        Some("off") => Ok(Request::Off),
        _ => Err(Refusal::NotOnOrOff {
            given: arg.to_string_lossy().into_owned(),
        }),
    }
}

/// The effective user ID in `/proc/self/status`: the second number of its `Uid:` line (real,
/// effective, saved, filesystem; proc_pid_status(5)).
pub fn effective_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|ids| ids.split_whitespace().nth(1))
        .and_then(|id| id.parse().ok())
}

/// The programs it runs, always by their full paths.
pub struct Programs {
    pub systemctl: PathBuf,
    pub detect_virt: PathBuf,
    pub notify: PathBuf,
}

impl Programs {
    /// systemd's own, where Arch's systemd package installs them.
    pub fn system() -> Programs {
        Programs {
            systemctl: PathBuf::from("/usr/bin/systemctl"),
            detect_virt: PathBuf::from("/usr/bin/systemd-detect-virt"),
            notify: PathBuf::from("/usr/bin/systemd-notify"),
        }
    }
}

/// The `systemctl` commands for `request`, in order. With systemd `live` the service also starts
/// or stops; otherwise only the links change, as Arch's `systemd-hook` skips its work in a chroot
/// or when the root isn't booted.
pub fn steps(request: Request, live: bool) -> Vec<&'static [&'static str]> {
    match (request, live) {
        (Request::On, true) => vec![
            &["enable", SERVICE],
            &["--global", "enable", HELPER],
            &["start", SERVICE],
        ],
        (Request::On, false) => vec![&["enable", SERVICE], &["--global", "enable", HELPER]],
        (Request::Off, true) => vec![
            &["--global", "disable", HELPER],
            &["disable", "--now", SERVICE],
        ],
        (Request::Off, false) => vec![&["--global", "disable", HELPER], &["disable", SERVICE]],
    }
}

/// Runs `program` with `args`, an emptied environment, no input and `/` as its folder; its output
/// goes to this program's.
fn status(program: &PathBuf, args: &[&str]) -> Result<ExitStatus, String> {
    Command::new(program)
        .args(args)
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::null())
        .status()
        .map_err(|e| format!("{}: {e}", program.display()))
}

/// Whether systemd runs this root: not a chroot (`systemd-detect-virt --chroot` exits 0 in one),
/// and booted with systemd (`systemd-notify --booted` exits 0), the checks of Arch's
/// `systemd-hook`.
///
/// # Errors
///
/// When a check can't run.
pub fn live(programs: &Programs) -> Result<bool, String> {
    if status(&programs.detect_virt, &["--chroot"])?.success() {
        return Ok(false);
    }
    Ok(status(&programs.notify, &["--booted"])?.success())
}

/// Turns OpenDeepCool on or off: the checks of [`live`], then each step of [`steps`], stopping at
/// the first that fails. Returns what it did.
///
/// # Errors
///
/// Which check or step failed; the steps before it stay done.
pub fn run(request: Request, programs: &Programs) -> Result<String, String> {
    let live = live(programs)?;
    let steps = steps(request, live);
    for (n, args) in steps.iter().enumerate() {
        let status = status(&programs.systemctl, args)?;
        if !status.success() {
            return Err(format!(
                "step {} of {}, `systemctl {}`, failed: {status}",
                n + 1,
                steps.len(),
                args.join(" ")
            ));
        }
    }
    let what = match request {
        Request::On => "turned on at boot",
        Request::Off => "turned off at boot",
    };
    Ok(if live {
        match request {
            Request::On => format!("{what}, and the service started"),
            Request::Off => format!("{what}, and the service stopped"),
        }
    } else {
        format!("{what}; in a chroot or without a running systemd, nothing was started or stopped")
    })
}
