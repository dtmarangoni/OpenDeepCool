//! systemd's own score for a unit's sandbox (Phase 11 plan, Task 2): the overall exposure
//! `systemd-analyze security --offline=true` gives a unit file of the repository, from 0.0 (every
//! protection on) to 10.0. The Arch package guidelines' security page checks units this way.
//!
//! The file is scored as a copy under a name no drop-in on this PC uses, so a development copy's
//! drop-ins (`/etc/systemd/system/opendeepcool.service.d/`, which relaxes `UMask=`) can't change
//! the result. pacman brings `systemd-analyze` with systemd on every Arch system, CI's container
//! included.

use std::fs;
use std::path::Path;
use std::process::Command;

/// The service manager a unit belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Manager {
    System,
    User,
}

/// The overall exposure of `unit`, copied into `scratch` as `odc-check-<name>`.
pub fn exposure(unit: &Path, manager: Manager, scratch: &Path) -> f64 {
    let name = unit
        .file_name()
        .and_then(|n| n.to_str())
        .expect("a unit file name");
    fs::create_dir_all(scratch).expect("a scratch folder");
    let copy = scratch.join(format!("odc-check-{name}"));
    fs::copy(unit, &copy).unwrap_or_else(|e| panic!("{}: {e}", unit.display()));
    let mut command = Command::new("systemd-analyze");
    command.args(["security", "--offline=true", "--no-pager"]);
    if manager == Manager::User {
        command.arg("--user");
    }
    let out = command
        .arg(&copy)
        .output()
        .expect("systemd-analyze, which pacman brings with systemd");
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text
        .lines()
        .find(|line| line.contains("Overall exposure level"))
        .unwrap_or_else(|| {
            panic!(
                "no exposure level for {name}: {text}{}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    line.rsplit(':')
        .next()
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|level| level.parse().ok())
        .unwrap_or_else(|| panic!("{line}: no number"))
}

/// Runs systemd's own check of a system `unit` (`systemd-analyze verify`) in a scratch root
/// under `scratch`, where a stand-in for its program `exec` exists, since it isn't installed here.
/// The error is systemd's output.
pub fn verify(unit: &Path, exec: &str, scratch: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let root = scratch.join("root");
    let _ = fs::remove_dir_all(&root);
    let units = root.join("etc/systemd/system");
    fs::create_dir_all(&units).expect("a unit folder");
    let program = root.join(exec.trim_start_matches('/'));
    fs::create_dir_all(program.parent().expect("a program folder")).expect("a program folder");
    fs::write(&program, "").expect("a stand-in");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("executable");
    let name = unit.file_name().expect("a unit file name");
    let copy = units.join(name);
    fs::copy(unit, &copy).unwrap_or_else(|e| panic!("{}: {e}", unit.display()));
    let out = Command::new("systemd-analyze")
        .arg("verify")
        .arg(format!("--root={}", root.display()))
        .arg(&copy)
        .output()
        .expect("systemd-analyze, which pacman brings with systemd");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if out.status.success() && text.trim().is_empty() {
        Ok(())
    } else {
        Err(text)
    }
}
