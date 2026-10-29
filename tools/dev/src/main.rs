//! `odc-dev` installs, checks and removes OpenDeepCool's development setup on the development PC:
//! the dev copies of the udev rule and the group file, the dev-only reconnect rule, and the
//! user's membership, all recorded in a manifest (PRD, System Footprint). It's a dev-only tool,
//! never shipped.
//!
//! `setup` and `teardown` run as the user and never as root; each hands only its root step to
//! `pkexec`, and that step never builds (owner, 2026-10-27; solution review P5-03).

mod check;
mod install;
mod system;

use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

use odc_manifest::Manifest;

const PKEXEC: &str = "/usr/bin/pkexec";

const USAGE: &str = "usage: odc-dev <command>

  setup                        install the development setup (one pkexec step)
  check                        report whether it's in place; changes nothing
  teardown                     remove everything its manifest lists (one pkexec step)
  teardown --check-only <file> check a manifest; removes nothing

Run setup and teardown as your own user, never as root.
The real-cooler tests and research tools must not run at the same time.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["setup"] => setup(),
        ["check"] => check::check(),
        ["teardown"] => teardown(),
        ["teardown", "--check-only", manifest] => check_only(Path::new(manifest)),
        ["install-root", "--repo", repo] => install::install_root(Path::new(repo)),
        ["remove-root"] => install::remove_root(),
        ["-h" | "--help"] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("odc-dev: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Refuses root: the user side of setup builds, and a build must never run as root (P5-03).
fn refuse_root(euid: u32) -> Result<(), String> {
    if euid == 0 {
        Err(
            "run odc-dev as your own user, not as root: only its root step runs through pkexec"
                .into(),
        )
    } else {
        Ok(())
    }
}

/// Builds what the dev copy needs (nothing yet in Phase 5; Phase 7 adds the service), then hands
/// the install to `pkexec`.
fn setup() -> Result<(), String> {
    refuse_root(system::effective_uid()?)?;
    let repo = system::repo_root();
    println!("odc-dev setup will install, through pkexec:");
    for file in system::FILES {
        println!("  {}  (from {})", file.target, file.source);
    }
    println!(
        "  the group {} (systemd-sysusers), with you in it\n  the manifest {}\nthen reload udev \
         and re-trigger the LP360's node and USB device and the CPU's package energy zones.",
        odc_manifest::GROUP,
        odc_manifest::PATH
    );
    let repo = repo.to_str().ok_or("the checkout's path isn't UTF-8")?;
    as_root(&["install-root", "--repo", repo])
}

fn teardown() -> Result<(), String> {
    refuse_root(system::effective_uid()?)?;
    as_root(&["remove-root"])
}

/// Runs this binary's root step through `pkexec`.
fn as_root(args: &[&str]) -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| format!("this program's path: {e}"))?;
    let status = Command::new(PKEXEC)
        .arg(&me)
        .args(args)
        .status()
        .map_err(|e| format!("{PKEXEC}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("the root step failed: {status}"))
    }
}

fn check_only(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let manifest = Manifest::parse(&text)
        .map_err(|r| format!("{}: {r}; refused, nothing would be removed", path.display()))?;
    println!(
        "{}: accepted, {} entries",
        path.display(),
        manifest.entries().len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_and_teardown_refuse_root() {
        assert!(refuse_root(0).is_err());
        assert!(refuse_root(1000).is_ok());
    }
}
