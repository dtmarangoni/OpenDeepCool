//! `odc-dev` installs, checks and removes OpenDeepCool's development setup on the development PC:
//! the dev copies of the udev rule and the sysusers file, the dev-only reconnect rule, the user's
//! membership, from Phase 7 the dev copy of the display service with its GPU rank helper, and
//! from Phase 9 the sound helper's user unit, all recorded in a manifest (PRD, System Footprint).
//! It's a dev-only tool, never shipped.
//!
//! `setup` and `teardown` run as the user and never as root; each hands only its root step to
//! `pkexec`, and that step never builds (owner, 2026-10-27; solution review P5-03): `setup`
//! builds first, as the user.
//!
//! While the Arch package is installed, the dev copy must not be: its files in `/etc` carry the
//! package's names, so they would override the package's, and the package's removal would delete
//! them. `setup` refuses then, and `setup --test-access` gives the tests what they need from the
//! package instead (Phase 11 plan, Task 7; owner, 2026-11-11).

mod check;
mod install;
mod system;

use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

use odc_manifest::{Manifest, USER_UNIT};

use crate::system::Tool;

const PKEXEC: &str = "/usr/bin/pkexec";

/// The Arch package whose installation changes what the dev tool may do.
const PACKAGE: &str = "opendeepcool";

const USAGE: &str = "usage: odc-dev <command>

  setup                        install the development setup (one pkexec step); refused
                               while the package opendeepcool is installed
  setup --test-access          for the installed package: your account in its group and the
                               reconnect rule, nothing else (one pkexec step)
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
        ["setup", "--test-access"] => setup_test_access(),
        ["check"] => check::check(),
        ["teardown"] => teardown(),
        ["teardown", "--check-only", manifest] => check_only(Path::new(manifest)),
        ["install-root", "--repo", repo] => install::install_root(Path::new(repo)),
        ["test-access-root", "--repo", repo] => install::test_access_root(Path::new(repo)),
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

/// Refuses the dev copy while the package is installed, and changes nothing (owner, 2026-11-11).
fn refuse_while_packaged(installed: bool) -> Result<(), String> {
    if installed {
        Err(format!(
            "the package {PACKAGE} is installed, so the development copy can't be: its files in \
             /etc carry the package's names, so they would override the package's files, and \
             removing the package would delete them. Nothing was changed. Remove the package \
             first with `sudo pacman -R {PACKAGE}`; `odc-dev setup --test-access` is the setup \
             for the installed package"
        ))
    } else {
        Ok(())
    }
}

/// Gives the tests what they need from the installed package, and nothing else: the user in its
/// group, and the dev-only reconnect rule. Says what the root step will do, then hands it to
/// `pkexec`. It builds nothing: there's no dev copy.
fn setup_test_access() -> Result<(), String> {
    refuse_root(system::effective_uid()?)?;
    if !system::package_installed()? {
        return Err(format!(
            "the package {PACKAGE} isn't installed: `--test-access` is for the installed \
             package, and its group comes from the package. Nothing was changed"
        ));
    }
    let repo = system::repo_root();
    println!("odc-dev setup --test-access will, through pkexec:");
    println!(
        "  install {}  (from {})",
        system::RECONNECT_RULE.target,
        system::RECONNECT_RULE.source
    );
    println!(
        "  add you to the package's group {}, which takes effect at your next login",
        odc_manifest::GROUP
    );
    println!(
        "  record both in the manifest {}, which `odc-dev teardown` removes; the group is the \
         package's, so it stays",
        odc_manifest::PATH
    );
    println!(
        "then reload udev and re-trigger the LP360's node and USB device and the CPU's package \
         energy zones."
    );
    let repo = repo.to_str().ok_or("the checkout's path isn't UTF-8")?;
    as_root(&["test-access-root", "--repo", repo])
}

/// Builds the dev copy's binaries as the user, says everything the root step will do, then hands
/// the install to `pkexec`.
fn setup() -> Result<(), String> {
    refuse_root(system::effective_uid()?)?;
    refuse_while_packaged(system::package_installed()?)?;
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
         systemd's units, enable {unit} at boot and (re)start it, which runs the GPU rank helper \
         while there's no ranking, and enable {user_unit} for every user. Its saved data goes in \
         {state}. Then, as you, (re)start {user_unit} in this session.",
        group = odc_manifest::GROUP,
        manifest = odc_manifest::PATH,
        unit = odc_manifest::UNIT,
        user_unit = USER_UNIT,
        state = odc_manifest::STATE_DIR,
    );
    let repo = repo.to_str().ok_or("the checkout's path isn't UTF-8")?;
    as_root(&["install-root", "--repo", repo])?;
    // The user's own manager reads the new unit; the helper starts in this session now, and at
    // every user's login from now on.
    Tool::Systemctl.run(&["--user", "daemon-reload"])?;
    Tool::Systemctl.run(&["--user", "restart", USER_UNIT])?;
    println!("(re)started {USER_UNIT} in this session");
    Ok(())
}

/// The dev copy's four binaries, built `--release` from the checkout's lock file, by the cargo
/// that built this tool (solution review P5-03).
const BUILD: [&str; 19] = [
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
    "-p",
    "odc-audio",
    "--bin",
    "opendeepcool-audio",
    "-p",
    "odc-app",
    "--bin",
    "opendeepcool",
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

/// Stops the sound helper in this session first, as the user: the root step disables it for
/// every user, but a running copy is each session's own.
fn teardown() -> Result<(), String> {
    refuse_root(system::effective_uid()?)?;
    // Only the dev copy's helper is the dev tool's to stop: with the package installed, the
    // running one is the package's, and a manifest of the test access alone doesn't list it.
    if lists_the_dev_helper(&fs::read_to_string(odc_manifest::PATH).unwrap_or_default())
        && Tool::Systemctl.succeeds(&["--user", "--quiet", "is-active", USER_UNIT])?
    {
        Tool::Systemctl.run(&["--user", "stop", USER_UNIT])?;
        println!("stopped {USER_UNIT} in this session");
    }
    as_root(&["remove-root"])?;
    Tool::Systemctl.run(&["--user", "daemon-reload"])
}

/// Whether the manifest lists the sound helper's enabling for every user, which only the dev
/// copy's setup makes. A manifest that can't be read or parsed lists nothing; the root step
/// reports it.
fn lists_the_dev_helper(manifest: &str) -> bool {
    Manifest::parse(manifest).is_ok_and(|m| m.contains(&odc_manifest::Entry::EnabledForEveryUser))
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

    #[test]
    fn the_dev_copy_is_refused_while_the_package_is_installed() {
        let refused = refuse_while_packaged(true).expect_err("refused");
        assert!(refused.contains("sudo pacman -R opendeepcool"), "{refused}");
        assert!(refused.contains("Nothing was changed"), "{refused}");
        assert!(refused.contains("--test-access"), "{refused}");
        assert!(refuse_while_packaged(false).is_ok());
    }

    #[test]
    fn only_a_manifest_with_the_dev_helper_has_the_helper_stopped() {
        let dev = "file /etc/udev/rules.d/70-opendeepcool.rules\nenabled-for-every-user \
                   opendeepcool-audio.service\n";
        let test_access = "file /etc/udev/rules.d/71-opendeepcool-dev-reconnect.rules\n\
                           member opendeepcool someone\n";
        assert!(lists_the_dev_helper(dev));
        assert!(!lists_the_dev_helper(test_access));
        assert!(!lists_the_dev_helper(""));
        assert!(!lists_the_dev_helper("not a manifest\n"));
    }
}
