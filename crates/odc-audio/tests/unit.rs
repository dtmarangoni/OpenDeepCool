//! The sound helper's user unit (Phase 9 plan, Task 4): never for system accounts, started again
//! on failure, enabled at login, the sandbox the plan lists (Phase 11 plan, Task 2: as tight as
//! its job allows), its score, and systemd's own check. systemd's `--user` check can't take a
//! scratch root, so it checks a copy whose `ExecStart` names a stand-in; everything else is the
//! unit as written.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const UNIT: &str = "../../packaging/systemd-user/opendeepcool-audio.service";
const EXEC: &str = "/usr/lib/opendeepcool/opendeepcool-audio";

fn unit_text() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(UNIT)).expect("the unit")
}

/// Each `key=value` line with its section, in order.
fn lines() -> Vec<(String, String, String)> {
    let mut section = String::new();
    let mut lines = Vec::new();
    for line in unit_text().lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.to_owned();
            continue;
        }
        let (key, value) = line.split_once('=').expect("a key=value line");
        lines.push((section.clone(), key.to_owned(), value.to_owned()));
    }
    lines
}

fn values(section: &str, key: &str) -> Vec<String> {
    lines()
        .into_iter()
        .filter(|(s, k, _)| s == section && k == key)
        .map(|(_, _, v)| v)
        .collect()
}

#[test]
fn the_unit_runs_for_people_only_restarts_and_is_sandboxed() {
    assert_eq!(values("Unit", "ConditionUser"), ["!@system"]);
    assert_eq!(values("Install", "WantedBy"), ["default.target"]);
    assert_eq!(values("Service", "ExecStart"), [EXEC]);
    for (key, value) in [
        ("Restart", "on-failure"),
        // Phase 11 plan, Task 2: every protection a user manager allows for the job.
        ("PrivateUsers", "yes"),
        ("NoNewPrivileges", "yes"),
        ("CapabilityBoundingSet", ""),
        ("UMask", "0077"),
        ("KeyringMode", "private"),
        ("ProtectSystem", "strict"),
        ("ProtectHome", "read-only"),
        ("PrivateTmp", "yes"),
        ("PrivateDevices", "yes"),
        ("PrivateNetwork", "yes"),
        ("RestrictAddressFamilies", "AF_UNIX"),
        ("ProtectKernelTunables", "yes"),
        ("ProtectKernelModules", "yes"),
        ("ProtectKernelLogs", "yes"),
        ("ProtectControlGroups", "yes"),
        ("ProtectClock", "yes"),
        ("ProtectHostname", "yes"),
        ("ProtectProc", "invisible"),
        ("ProcSubset", "pid"),
        ("RestrictNamespaces", "yes"),
        ("RestrictSUIDSGID", "yes"),
        ("LockPersonality", "yes"),
        ("MemoryDenyWriteExecute", "yes"),
        ("SystemCallArchitectures", "native"),
        ("SystemCallErrorNumber", "EPERM"),
        // PipeWire's client may ask for realtime scheduling: unverified, so not restricted.
        ("RestrictRealtime", "no"),
    ] {
        assert_eq!(values("Service", key), [value], "{key}");
    }
    assert_eq!(
        values("Service", "SystemCallFilter"),
        ["@system-service", "~@privileged"]
    );
}

/// systemd's own score for the sandbox stays at 0.6 "SAFE" or better (7.9 before Phase 11).
#[test]
fn systemds_score_for_the_sandbox_stays_safe() {
    let unit = Path::new(env!("CARGO_MANIFEST_DIR")).join(UNIT);
    let scratch = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("audio-unit-score");
    let level = odc_testkit::units::exposure(&unit, odc_testkit::units::Manager::User, &scratch);
    assert!(level <= 0.6, "exposure {level}");
}

#[test]
fn systemd_accepts_the_unit() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("user-unit");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a folder");
    let stand_in = dir.join("opendeepcool-audio");
    fs::write(&stand_in, "").expect("a stand-in");
    fs::set_permissions(&stand_in, fs::Permissions::from_mode(0o755)).expect("executable");
    let copy = dir.join("opendeepcool-audio.service");
    let text = unit_text().replace(
        &format!("ExecStart={EXEC}"),
        &format!("ExecStart={}", stand_in.display()),
    );
    fs::write(&copy, text).expect("the copy");
    // A user manager's verify needs a runtime folder, which a session sets in XDG_RUNTIME_DIR and
    // CI's container, with no session, lacks: it gets one of its own.
    let runtime = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("user-rt");
    let _ = fs::remove_dir_all(&runtime);
    fs::create_dir_all(&runtime).expect("a runtime folder");
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).expect("private");
    let out = Command::new("systemd-analyze")
        .args(["verify", "--user"])
        .arg(&copy)
        .env("XDG_RUNTIME_DIR", &runtime)
        .output()
        .expect("systemd-analyze: pacman brings it on every Arch system");
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
