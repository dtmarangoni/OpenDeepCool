//! `odc-dev check`: whether the development setup is in place, as the user, with no change.

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use odc_manifest::{GROUP, Manifest, STATE_DIR, UNIT, USER, USER_UNIT};
use odc_sensors::{PciAddr, RANK_FILE, RankedKind, Ranking, display_devices};

use crate::system::{
    BINARIES, BUS_NAME, FILES, SERVICE_FILES, Tool, effective_uid, lp360_node, lp360_usb_device,
    package_zones, parse_groups, process_status, repo_root, user_name,
};

/// Reports each installed file and binary against the checkout and its release build, the group
/// and the user's membership, the cooler node's group and mode, the reconnect switch's group
/// write, the group read of the CPU's package energy counters, and the dev copy of the display
/// service. Fails on any mismatch; with no cooler attached, its two checks can't run and say so,
/// as does the energy check without RAPL zones.
pub fn check() -> Result<(), String> {
    let mut problems = Vec::new();
    let repo = repo_root();
    for file in FILES.iter().chain(&SERVICE_FILES).chain(&BINARIES) {
        let wanted = match fs::read(repo.join(file.source)) {
            Ok(wanted) => wanted,
            Err(e) => {
                problems.push(format!("{}: {e}", file.source));
                continue;
            }
        };
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

    let zones = package_zones();
    if zones.is_empty() {
        println!("skipped  no RAPL package zones: CPU power can't be checked");
    }
    if let Some(gid) = gid {
        for zone in zones {
            let energy = zone.join("energy_uj");
            let meta = fs::metadata(&energy).map_err(|e| format!("{}: {e}", energy.display()))?;
            if meta.gid() == gid && meta.mode() & 0o040 != 0 {
                println!("ok       {}: group-readable", energy.display());
            } else {
                problems.push(format!(
                    "{}: group ID {}, mode {:04o}; expected group-readable by {GROUP}",
                    energy.display(),
                    meta.gid(),
                    meta.mode() & 0o777
                ));
            }
        }
    }

    check_service(&mut problems)?;
    check_helper(&mut problems)?;
    check_ranking(&mut problems);

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

/// The dev copy of the display service: enabled at boot and active, started from its dev build
/// with `--dev`, and owning its bus name as the user `opendeepcool`.
fn check_service(problems: &mut Vec<String>) -> Result<(), String> {
    for (state, word) in [("is-enabled", "enabled at boot"), ("is-active", "active")] {
        if Tool::Systemctl.succeeds(&["--quiet", state, UNIT])? {
            println!("ok       {UNIT}: {word}");
        } else {
            problems.push(format!("{UNIT} isn't {word}"));
        }
    }

    let pid = Tool::Systemctl
        .output(&["show", "--property=MainPID", "--value", UNIT])?
        .unwrap_or_default();
    let pid = pid.trim();
    let wanted = [BINARIES[0].target, "--dev"];
    // systemd says 0 when the unit has no main process.
    let cmdline = match pid {
        "" | "0" => Err(None),
        pid => fs::read(format!("/proc/{pid}/cmdline")).map_err(Some),
    };
    match cmdline {
        Err(None) => problems.push(format!("{UNIT} has no main process")),
        Ok(cmdline) => {
            let args: Vec<String> = cmdline
                .split(|b| *b == 0)
                .filter(|arg| !arg.is_empty())
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect();
            if args == wanted {
                println!("ok       {UNIT}: runs {}", wanted.join(" "));
            } else {
                problems.push(format!(
                    "{UNIT} runs {args:?}, not {}: is the dev drop-in in place?",
                    wanted.join(" ")
                ));
            }
        }
        Err(Some(e)) => problems.push(format!("{UNIT}'s main process {pid}: {e}")),
    }

    let uid = Tool::Getent
        .output(&["passwd", USER])?
        .and_then(|line| line.split(':').nth(2)?.parse::<u32>().ok());
    let owner = Tool::Busctl.output(&[
        "--system",
        "call",
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "GetConnectionUnixUser",
        "s",
        BUS_NAME,
    ]);
    // busctl prints the reply as `u <user ID>`.
    let owner = owner.map(|out| out.and_then(|o| o.trim().strip_prefix("u ")?.parse::<u32>().ok()));
    match (owner, uid) {
        (Ok(Some(owner)), Some(uid)) if owner == uid => {
            println!("ok       {BUS_NAME}: owned by the user {USER} ({uid})");
        }
        (Ok(Some(owner)), _) => problems.push(format!(
            "{BUS_NAME} is owned by the user ID {owner}, not by {USER} ({uid:?})"
        )),
        (Ok(None), _) => problems.push(format!("{BUS_NAME}: busctl's reply isn't a user ID")),
        (Err(e), _) => problems.push(format!("{BUS_NAME} has no owner on the system bus ({e})")),
    }
    Ok(())
}

/// The sound helper (Phase 9 plan, Task 4): enabled for every user, active in this session from
/// its dev build, and, while the display service wants sound, its sound connected.
fn check_helper(problems: &mut Vec<String>) -> Result<(), String> {
    if Tool::Systemctl.succeeds(&["--global", "--quiet", "is-enabled", USER_UNIT])? {
        println!("ok       {USER_UNIT}: enabled for every user");
    } else {
        problems.push(format!("{USER_UNIT} isn't enabled for every user"));
    }
    if Tool::Systemctl.succeeds(&["--user", "--quiet", "is-active", USER_UNIT])? {
        println!("ok       {USER_UNIT}: active in this session");
    } else {
        problems.push(format!(
            "{USER_UNIT} isn't active in this session: `systemctl --user start {USER_UNIT}`"
        ));
        return Ok(());
    }
    let pid = Tool::Systemctl
        .output(&["--user", "show", "--property=MainPID", "--value", USER_UNIT])?
        .unwrap_or_default();
    let runs = fs::read(format!("/proc/{}/cmdline", pid.trim()))
        .map(|cmdline| {
            cmdline
                .split(|b| *b == 0)
                .filter(|arg| !arg.is_empty())
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if runs == [BINARIES[2].target] {
        println!("ok       {USER_UNIT}: runs {}", BINARIES[2].target);
    } else {
        problems.push(format!(
            "{USER_UNIT} runs {runs:?}, not {}: is the dev drop-in in place?",
            BINARIES[2].target
        ));
    }
    // The state's two sound keys, as busctl prints them: `"AudioWanted" b true`.
    let state = Tool::Busctl
        .output(&[
            "--system",
            "call",
            BUS_NAME,
            "/io/github/dtmarangoni/OpenDeepCool1",
            BUS_NAME,
            "GetState",
        ])?
        .unwrap_or_default();
    let key = |name: &str| {
        let after = state.split(&format!("\"{name}\" b ")).nth(1)?;
        Some(after.starts_with("true"))
    };
    match (key("AudioWanted"), key("AudioConnected")) {
        (Some(true), Some(true)) => {
            println!("ok       a Music mode is shown: its sound is connected")
        }
        (Some(true), _) => problems.push(
            "a Music mode is shown, but no sound is connected: `journalctl --user -u \
             opendeepcool-audio` says why"
                .into(),
        ),
        (Some(false), connected) => println!(
            "ok       no Music mode is shown: sound {}",
            if connected == Some(true) {
                "still connected"
            } else {
                "not taken"
            }
        ),
        (None, _) => problems.push("GetState doesn't say AudioWanted".into()),
    }
    Ok(())
}

/// The ranking the GPU rank helper wrote: well-formed, and listing exactly this PC's GPUs.
fn check_ranking(problems: &mut Vec<String>) {
    let path = Path::new(STATE_DIR).join(RANK_FILE);
    let ranking = match fs::read_to_string(&path).map(|text| Ranking::parse(&text)) {
        Ok(Ok(ranking)) => ranking,
        Ok(Err(why)) => return problems.push(format!("{}: {why}", path.display())),
        Err(e) => return problems.push(format!("{}: {e}", path.display())),
    };
    let present: BTreeSet<PciAddr> = display_devices(Path::new("/sys"))
        .into_iter()
        .map(|(pci, _)| pci)
        .collect();
    let ranked: BTreeSet<PciAddr> = ranking.gpus.iter().map(|gpu| gpu.pci).collect();
    if present != ranked {
        let list = |set: &BTreeSet<PciAddr>| {
            set.iter()
                .map(PciAddr::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        return problems.push(format!(
            "{} ranks [{}], but this PC's GPUs are [{}]",
            path.display(),
            list(&ranked),
            list(&present)
        ));
    }
    println!("ok       {}: ranks this PC's GPUs", path.display());
    for gpu in &ranking.gpus {
        let kind = match &gpu.kind {
            RankedKind::Dedicated { signal } => format!("dedicated ({signal})"),
            RankedKind::Integrated { signal } => format!("integrated ({signal})"),
            RankedKind::NoSignal => "unknown, no signal".to_owned(),
            RankedKind::QueryFailed { error } => format!("unknown, the query failed: {error}"),
        };
        println!("           {} ({}): {kind}", gpu.pci, gpu.driver);
    }
}
