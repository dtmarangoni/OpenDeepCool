//! The two steps that run as root, through `pkexec`: `install-root` and `remove-root`. They copy
//! files, run the listed system tools and keep the manifest; they never build (solution review
//! P5-03).

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, chown};
use std::path::Path;

use odc_manifest::{Entry, GROUP, Manifest, SystemHost};

use crate::system::{FILES, Tool, effective_uid, lp360_node, lp360_usb_device, user_name};

/// Refuses to run unless the effective user is root.
pub fn require_root(euid: u32) -> Result<(), String> {
    if euid == 0 {
        Ok(())
    } else {
        Err("this step runs as root, through `odc-dev setup` or `odc-dev teardown`".into())
    }
}

/// Installs the dev copies of the udev rule and the group file and the reconnect rule, adds the
/// user who ran `odc-dev setup` to the group, and records each step in the manifest. Run it again
/// and it changes only what differs; what the hand-made setup of 2026-10-24 already has, it takes
/// over.
pub fn install_root(repo: &Path) -> Result<(), String> {
    require_root(effective_uid()?)?;
    let uid = std::env::var("PKEXEC_UID")
        .ok()
        .and_then(|u| u.parse::<u32>().ok())
        .ok_or("PKEXEC_UID isn't set: run `odc-dev setup` instead")?;
    let user = user_name(uid)?;
    let member = Entry::member(&user)?;
    let mut manifest = load_manifest()?.unwrap_or_default();

    for file in FILES {
        let source = repo.join(file.source);
        let content = fs::read(&source).map_err(|e| format!("{}: {e}", source.display()))?;
        let target = Path::new(file.target);
        if fs::read(target).is_ok_and(|current| current == content) {
            println!("{}: up to date", file.target);
        } else {
            write_atomic(target, &content, 0o644)?;
            println!("installed {}", file.target);
        }
        record(&mut manifest, Entry::file(file.target)?)?;
    }

    Tool::SystemdSysusers.run(&[FILES[0].target])?;
    let members = odc_manifest::group_entry(GROUP)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("systemd-sysusers didn't create the group {GROUP}"))?
        .1;
    record(&mut manifest, Entry::Group)?;
    if members.contains(&user) {
        println!("{user} is in {GROUP} already");
    } else {
        Tool::Gpasswd.run(&["-a", &user, GROUP])?;
        println!("added {user} to {GROUP}: it takes effect at the next login");
    }
    record(&mut manifest, member)?;

    Tool::Udevadm.run(&["control", "--reload"])?;
    retrigger()
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

/// Sends the cooler's hidraw node and USB device through udev's rules again, so a change of
/// access takes effect at once.
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
