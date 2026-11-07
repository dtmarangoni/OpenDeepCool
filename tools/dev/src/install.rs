//! The two steps that run as root, through `pkexec`: `install-root` and `remove-root`. They copy
//! files, run the listed system tools and keep the manifest; they never build (solution review
//! P5-03).

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, chown};
use std::path::Path;

use odc_manifest::{Entry, GROUP, Manifest, SystemHost, UNIT, USER, USER_UNIT};

use crate::system::{
    APP_FILES, BINARIES, DIRS, FILES, Install, SERVICE_FILES, Tool, effective_uid, lp360_node,
    lp360_usb_device, package_zones, user_name,
};

/// Refuses to run unless the effective user is root.
pub fn require_root(euid: u32) -> Result<(), String> {
    if euid == 0 {
        Ok(())
    } else {
        Err("this step runs as root, through `odc-dev setup` or `odc-dev teardown`".into())
    }
}

/// Copies `file` from the checkout with `mode`, unless it's there already, and records it.
fn install(manifest: &mut Manifest, repo: &Path, file: &Install, mode: u32) -> Result<(), String> {
    let source = repo.join(file.source);
    let content = fs::read(&source).map_err(|e| format!("{}: {e}", source.display()))?;
    let target = Path::new(file.target);
    if fs::read(target).is_ok_and(|current| current == content) {
        println!("{}: up to date", file.target);
    } else {
        write_atomic(target, &content, mode)?;
        println!("installed {}", file.target);
    }
    record(manifest, Entry::file(file.target)?)
}

/// Installs the dev copies of the udev rule, the sysusers file and the reconnect rule, adds the
/// user who ran `odc-dev setup` to the group, and from Phase 7 installs the dev copy of the
/// display service, enabled at boot and started (Phase 7 plan, Task 8). Each step is recorded in
/// the manifest, in the order that lets the teardown undo it backwards: the service stops first,
/// then its saved data, its account, its files and its folders go. Run it again and it changes
/// only what differs; what the hand-made setup of 2026-10-24 already has, it takes over.
pub fn install_root(repo: &Path) -> Result<(), String> {
    require_root(effective_uid()?)?;
    let uid = std::env::var("PKEXEC_UID")
        .ok()
        .and_then(|u| u.parse::<u32>().ok())
        .ok_or("PKEXEC_UID isn't set: run `odc-dev setup` instead")?;
    let user = user_name(uid)?;
    let member = Entry::member(&user)?;
    let mut manifest = load_manifest()?.unwrap_or_default();
    // This step never builds (P5-03): the user's side did.
    for binary in &BINARIES {
        let source = repo.join(binary.source);
        if !source.is_file() {
            return Err(format!(
                "{} isn't built: run `odc-dev setup`, which builds it first",
                source.display()
            ));
        }
    }

    for file in &FILES {
        install(&mut manifest, repo, file, 0o644)?;
    }
    for dir in DIRS {
        let path = Path::new(dir);
        if !path.is_dir() {
            fs::create_dir(path).map_err(|e| format!("{dir}: {e}"))?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("{dir}: {e}"))?;
            record(&mut manifest, Entry::dir(dir)?)?;
            println!("created {dir}");
        }
    }
    for binary in &BINARIES {
        install(&mut manifest, repo, binary, 0o755)?;
    }
    for file in SERVICE_FILES.iter().chain(&APP_FILES) {
        install(&mut manifest, repo, file, 0o644)?;
    }

    // The sysusers file makes the service's account, and the group with it.
    let had_user = Tool::Getent.output(&["passwd", USER])?.is_some();
    Tool::SystemdSysusers.run(&[FILES[0].target])?;
    let members = odc_manifest::group_entry(GROUP)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("systemd-sysusers didn't create the group {GROUP}"))?
        .1;
    record(&mut manifest, Entry::Group)?;
    if !had_user {
        if Tool::Getent.output(&["passwd", USER])?.is_none() {
            return Err(format!("systemd-sysusers didn't create the user {USER}"));
        }
        record(&mut manifest, Entry::User)?;
        println!("created the user {USER}");
    }
    if members.contains(&user) {
        println!("{user} is in {GROUP} already");
    } else {
        Tool::Gpasswd.run(&["-a", &user, GROUP])?;
        println!("added {user} to {GROUP}: it takes effect at the next login");
    }
    record(&mut manifest, member)?;

    Tool::Udevadm.run(&["control", "--reload"])?;
    retrigger()?;

    // The bus reads the new policy, polkit the new actions, systemd the new units; then the
    // service starts at once and at every boot, pulling the GPU rank helper in while no ranking
    // exists.
    reload_policies_and_units()?;
    record(&mut manifest, Entry::State)?;
    Tool::Systemctl.run(&["enable", UNIT])?;
    record(&mut manifest, Entry::Enabled)?;
    Tool::Systemctl.run(&["restart", UNIT])?;
    println!("enabled and (re)started {UNIT}");
    // The sound helper starts at every user's login (Phase 9 plan, Task 4); the user's side of
    // setup starts it in the session that ran it.
    Tool::Systemctl.run(&["--global", "enable", USER_UNIT])?;
    record(&mut manifest, Entry::EnabledForEveryUser)?;
    println!("enabled {USER_UNIT} for every user");
    Ok(())
}

/// Tells the system bus to read its policies again (the bus driver's `ReloadConfig`), polkit its
/// actions, and systemd its units. polkitd didn't read the actions file put into
/// `/etc/polkit-1/actions`, a folder made after it started, until it reloaded (seen on the
/// development PC, 2026-11-04). Its reload, SIGHUP, reads every action again (polkitd.c,
/// `on_sighup`); `try-reload-or-restart` does nothing while polkit isn't running (systemctl(1)).
fn reload_policies_and_units() -> Result<(), String> {
    Tool::Busctl.run(&[
        "call",
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "ReloadConfig",
    ])?;
    Tool::Systemctl.run(&["try-reload-or-restart", "polkit.service"])?;
    Tool::Systemctl.run(&["daemon-reload"])
}

/// Removes what the manifest lists, checked whole first, then ends the cooler's access at once.
/// The manifest stays when something couldn't be removed, so a second run can finish.
pub fn remove_root() -> Result<(), String> {
    require_root(effective_uid()?)?;
    let Some(manifest) = load_manifest()? else {
        println!(
            "no dev manifest at {}: nothing to remove",
            odc_manifest::PATH
        );
        return Ok(());
    };
    let failures = manifest.remove_all(&mut SystemHost);
    reload_policies_and_units()?;
    Tool::Udevadm.run(&["control", "--reload"])?;
    if let Some(usb) = lp360_usb_device() {
        // A re-trigger can't take back the reconnect rule's chgrp and chmod: reset the switch to
        // the kernel's root:root 0644.
        let authorized = usb.join("authorized");
        chown(&authorized, Some(0), Some(0))
            .and_then(|()| fs::set_permissions(&authorized, fs::Permissions::from_mode(0o644)))
            .map_err(|e| format!("{}: {e}", authorized.display()))?;
        println!("reset {}", authorized.display());
    }
    for zone in package_zones() {
        // Likewise for the CPU power line: give each package counter's group back to root and
        // take the group's read away (Phase 6 plan, Task 6).
        let energy = zone.join("energy_uj");
        fs::metadata(&energy)
            .and_then(|meta| {
                chown(&energy, None, Some(0))?;
                let mode = meta.mode() & 0o7777 & !0o040;
                fs::set_permissions(&energy, fs::Permissions::from_mode(mode))
            })
            .map_err(|e| format!("{}: {e}", energy.display()))?;
        println!("reset {}", energy.display());
    }
    retrigger()?;
    if !failures.is_empty() {
        return Err(format!(
            "{} entries couldn't be removed; {} stays for a second run",
            failures.len(),
            odc_manifest::PATH
        ));
    }
    let path = Path::new(odc_manifest::PATH);
    fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(dir) = path.parent() {
        fs::remove_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    println!("removed {}", odc_manifest::PATH);
    Ok(())
}

/// The manifest, checked whole; `None` when there isn't one.
fn load_manifest() -> Result<Option<Manifest>, String> {
    match fs::read_to_string(odc_manifest::PATH) {
        Ok(text) => Manifest::parse(&text)
            .map(Some)
            .map_err(|r| format!("{}: {r}; nothing was changed", odc_manifest::PATH)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", odc_manifest::PATH)),
    }
}

/// Adds an entry the manifest doesn't have yet, and saves it at once, so even a setup that stops
/// halfway can be torn down.
fn record(manifest: &mut Manifest, entry: Entry) -> Result<(), String> {
    if manifest.contains(&entry) {
        return Ok(());
    }
    manifest.push(entry)?;
    let path = Path::new(odc_manifest::PATH);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    write_atomic(path, manifest.render().as_bytes(), 0o644)
}

/// Writes a file whole or not at all: a hidden temporary file beside it, then a rename.
fn write_atomic(target: &Path, content: &[u8], mode: u32) -> Result<(), String> {
    let name = target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{} has no file name", target.display()))?;
    let temporary = target.with_file_name(format!(".{name}.odc-dev"));
    let write = || -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(mode)
            .open(&temporary)?;
        file.write_all(content)?;
        file.sync_all()?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(mode))?;
        fs::rename(&temporary, target)
    };
    write().map_err(|e| {
        let _ = fs::remove_file(&temporary);
        format!("{}: {e}", target.display())
    })
}

/// Sends the cooler's hidraw node and USB device, and the CPU's package energy zones, through
/// udev's rules again, so a change of access takes effect at once.
fn retrigger() -> Result<(), String> {
    let node = lp360_node()?;
    let usb = lp360_usb_device();
    if let Some(node) = &node {
        let path = node.sysfs.to_string_lossy();
        Tool::Udevadm.run(&["trigger", "--settle", "--action=change", &path])?;
    }
    if let Some(usb) = &usb {
        let path = usb.to_string_lossy();
        Tool::Udevadm.run(&["trigger", "--settle", "--action=add", &path])?;
    }
    if node.is_none() && usb.is_none() {
        println!("no LP360 attached: the rules apply when it's plugged in");
    }
    let zones = package_zones();
    for zone in &zones {
        let path = zone.to_string_lossy();
        Tool::Udevadm.run(&["trigger", "--settle", "--action=change", &path])?;
    }
    if zones.is_empty() {
        println!("no RAPL package zones: the CPU power line has nothing to apply to");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_steps_refuse_to_run_without_root() {
        assert!(require_root(0).is_ok());
        assert!(require_root(1000).is_err());
    }
}
