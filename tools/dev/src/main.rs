//! `odc-dev` installs, checks and removes OpenDeepCool's development setup on the development PC:
//! the dev copies of the udev rule and the sysusers file, the dev-only reconnect rule, the user's
//! membership, and from Phase 7 the dev copy of the display service with its GPU rank helper, all
//! recorded in a manifest (PRD, System Footprint). It's a dev-only tool, never shipped.
//!
//! `setup` and `teardown` run as the user and never as root; each hands only its root step to
//! `pkexec`, and that step never builds (owner, 2026-10-27; solution review P5-03): `setup`
//! builds first, as the user.

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

/// Builds the dev copy's binaries as the user, says everything the root step will do, then hands
/// the install to `pkexec`.
fn setup() -> Result<(), String> {
    refuse_root(system::effective_uid()?)?;
    let repo = system::repo_root();
    build(&repo)?;
    println!("odc-dev setup will install, through pkexec:");
    for file in system::FILES {
        println!("  {}  (from {})", file.target, file.source);
    }
    println!("  the folders, those missing: {}", system::DIRS.join(", "));
    for file in system::BINARIES.iter().chain(&system::SERVICE_FILES) {
        println!("  {}  (from {})", file.target, file.source);
    }
    println!(
        "  the user and group {group} (systemd-sysusers), with you in the group\n  the manifest \
         {manifest}\nthen reload udev and re-trigger the LP360's node and USB device and the \
         CPU's package energy zones, reload the system bus's policies, polkit's actions and \
         systemd's units, and enable {unit} at boot and (re)start it, which runs the GPU rank helper while there's no \
         ranking. Its saved data goes in {state}.",
        group = odc_manifest::GROUP,
        manifest = odc_manifest::PATH,
        unit = odc_manifest::UNIT,
        state = odc_manifest::STATE_DIR,
    );
    let repo = repo.to_str().ok_or("the checkout's path isn't UTF-8")?;
    as_root(&["install-root", "--repo", repo])
}

/// The dev copy's two binaries, built `--release` from the checkout's lock file, by the cargo
/// that built this tool (solution review P5-03).
const BUILD: [&str; 11] = [
    "build",
    "--release",
    "--locked",
    "-p",
    "odc-service",
    "--bin",
    "opendeepcoold",
    "-p",
    "odc-gpu-rank",
    "--bin",
    "odc-gpu-rank",
];

/// Builds [`BUILD`] in the checkout, as the user: the only build `odc-dev` starts.
fn build(repo: &Path) -> Result<(), String> {
    let cargo = env!("CARGO");
    println!("building: cargo {}", BUILD.join(" "));
    let status = Command::new(cargo)
        .args(BUILD)
        .current_dir(repo)
        .status()
        .map_err(|e| format!("{cargo}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo {}: {status}", BUILD.join(" ")))
    }
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
