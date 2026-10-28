//! `odc-dev check`: whether the development setup is in place, as the user, with no change.

use std::fs;
use std::os::unix::fs::MetadataExt;

use odc_manifest::{GROUP, Manifest};

use crate::system::{
    FILES, effective_uid, lp360_node, lp360_usb_device, parse_groups, process_status, repo_root,
    user_name,
};

/// Reports each installed file against the checkout, the group and the user's membership, the
/// cooler node's group and mode, and the reconnect switch's group write. Fails on any mismatch;
/// with no cooler attached, its two checks can't run and say so.
pub fn check() -> Result<(), String> {
    let mut problems = Vec::new();
    let repo = repo_root();
    for file in FILES {
        let wanted =
            fs::read(repo.join(file.source)).map_err(|e| format!("{}: {e}", file.source))?;
        match fs::read(file.target) {
            Ok(current) if current == wanted => println!("ok       {}", file.target),
            Ok(_) => problems.push(format!("{} differs from {}", file.target, file.source)),
            Err(e) => problems.push(format!("{}: {e}", file.target)),
        }
    }

    let group = odc_manifest::group_entry(GROUP).map_err(|e| e.to_string())?;
    let user = user_name(effective_uid()?)?;
    let gid = match group {
        None => {
            problems.push(format!("there's no group {GROUP}"));
            None
        }
        Some((gid, members)) => {
            println!("ok       group {GROUP} ({gid})");
            if !members.contains(&user) {
                problems.push(format!("{user} isn't in {GROUP}"));
            } else if !parse_groups(&process_status()?).contains(&gid) {
                problems.push(format!(
                    "{user} is in {GROUP}, but not in this session yet: log in again, or use \
                     `newgrp {GROUP}`"
                ));
            } else {
                println!("ok       {user} is in {GROUP}, in this session too");
            }
            Some(gid)
        }
    };

    match (lp360_node()?, gid) {
        (None, _) => println!("skipped  no LP360 attached: its node can't be checked"),
        (Some(node), Some(gid)) => {
            let meta =
                fs::metadata(&node.dev).map_err(|e| format!("{}: {e}", node.dev.display()))?;
            let mode = meta.mode() & 0o777;
            if meta.gid() == gid && mode == 0o660 {
                println!("ok       {}: group {GROUP}, mode 0660", node.dev.display());
            } else {
                problems.push(format!(
                    "{}: group ID {}, mode {mode:04o}; expected {GROUP} and 0660",
                    node.dev.display(),
                    meta.gid()
                ));
            }
        }
        (Some(_), None) => {}
    }

    match (lp360_usb_device(), gid) {
        (None, _) => println!("skipped  no LP360 attached: its USB switch can't be checked"),
        (Some(usb), Some(gid)) => {
            let authorized = usb.join("authorized");
            let meta =
                fs::metadata(&authorized).map_err(|e| format!("{}: {e}", authorized.display()))?;
            if meta.gid() == gid && meta.mode() & 0o020 != 0 {
                println!("ok       {}: group-writable", authorized.display());
            } else {
                problems.push(format!(
                    "{}: group ID {}, mode {:04o}; expected group-writable by {GROUP}",
                    authorized.display(),
                    meta.gid(),
                    meta.mode() & 0o777
                ));
            }
        }
        (Some(_), None) => {}
    }

    match fs::read_to_string(odc_manifest::PATH) {
        Ok(text) => match Manifest::parse(&text) {
            Ok(m) => println!(
                "ok       {}: {} entries",
                odc_manifest::PATH,
                m.entries().len()
            ),
            Err(r) => problems.push(format!("{}: {r}", odc_manifest::PATH)),
        },
        Err(e) => problems.push(format!("{}: {e}", odc_manifest::PATH)),
    }

    for problem in &problems {
        println!("PROBLEM  {problem}");
    }
    if problems.is_empty() {
        println!("the development setup is in place");
        Ok(())
    } else {
        Err(format!(
            "{} problems; `odc-dev setup` fixes most",
            problems.len()
        ))
    }
}
