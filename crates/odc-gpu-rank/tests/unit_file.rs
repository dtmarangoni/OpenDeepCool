//! The helper's systemd unit (Phase 6 plan, Task 9; solution review P6-03, P6-14): a one-shot
//! with a start time limit, run as the service's account, which makes the state directory itself
//! on a fresh install, runs only while no ranking exists, and orders itself before the display
//! service. Its sandbox keeps only what that job needs (Phase 11 plan, Task 2).

use std::fs;
use std::path::Path;

use odc_sensors::RANK_FILE;
use odc_testkit::units::{self, Manager};

/// Each `key=value` line of the unit, with its section, in order.
fn unit() -> Vec<(String, String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/systemd/opendeepcool-gpu-rank.service");
    let text = fs::read_to_string(&path).expect("the unit file");
    let mut section = String::new();
    let mut lines = Vec::new();
    for line in text.lines().map(str::trim) {
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

/// Every value of `key` in `section`.
fn values(section: &str, key: &str) -> Vec<String> {
    unit()
        .into_iter()
        .filter(|(s, k, _)| s == section && k == key)
        .map(|(_, _, v)| v)
        .collect()
}

#[test]
fn a_one_shot_as_the_services_account_with_a_time_limit_and_its_state_directory() {
    assert_eq!(values("Service", "Type"), ["oneshot"]);
    assert_eq!(values("Service", "User"), ["opendeepcool"]);
    assert_eq!(
        values("Service", "ExecStart"),
        ["/usr/lib/opendeepcool/odc-gpu-rank"]
    );
    assert_eq!(values("Service", "TimeoutStartSec"), ["30"]);
    assert_eq!(values("Service", "StateDirectory"), ["opendeepcool"]);
}

#[test]
fn it_runs_only_while_no_ranking_exists_and_before_the_display_service() {
    assert_eq!(
        values("Unit", "ConditionPathExists"),
        [format!("!/var/lib/opendeepcool/{RANK_FILE}")]
    );
    assert_eq!(values("Unit", "Before"), ["opendeepcool.service"]);
    assert_eq!(values("Install", "WantedBy"), ["opendeepcool.service"]);
}

/// Every protection the helper's job allows (Phase 11 plan, Task 2), each pinned, so loosening
/// one fails here.
#[test]
fn its_sandbox_keeps_only_what_the_job_needs() {
    for (key, value) in [
        ("UMask", "0077"),
        ("ProtectSystem", "strict"),
        ("ProtectHome", "yes"),
        ("PrivateTmp", "yes"),
        ("PrivateUsers", "yes"),
        ("PrivateNetwork", "yes"),
        ("IPAddressDeny", "any"),
        ("RestrictAddressFamilies", "AF_UNIX"),
        ("NoNewPrivileges", "yes"),
        ("CapabilityBoundingSet", ""),
        ("ProtectKernelTunables", "yes"),
        ("ProtectKernelModules", "yes"),
        ("ProtectKernelLogs", "yes"),
        ("ProtectControlGroups", "yes"),
        ("ProtectClock", "yes"),
        ("ProtectHostname", "yes"),
        ("ProtectProc", "invisible"),
        ("RestrictNamespaces", "yes"),
        ("RestrictRealtime", "yes"),
        ("RestrictSUIDSGID", "yes"),
        ("RemoveIPC", "yes"),
        ("LockPersonality", "yes"),
        ("MemoryDenyWriteExecute", "yes"),
        ("SystemCallArchitectures", "native"),
        ("SystemCallErrorNumber", "EPERM"),
        ("DevicePolicy", "closed"),
    ] {
        assert_eq!(values("Service", key), [value], "{key}");
    }
    assert_eq!(
        values("Service", "SystemCallFilter"),
        ["@system-service", "~@privileged @resources"]
    );
}

/// Only the GPUs' devices: the DRM render nodes and NVIDIA's three groups for CUDA, named
/// exactly, with no wildcard that would let in the driver's other groups.
#[test]
fn it_opens_only_the_gpus_devices() {
    assert_eq!(
        values("Service", "DeviceAllow"),
        [
            "char-drm rw",
            "char-nvidiactl rw",
            "char-nvidia rw",
            "char-nvidia-uvm rw"
        ]
    );
}

/// systemd's own score for the sandbox stays at 0.4 "SAFE" or better (5.5 before Phase 11).
#[test]
fn systemds_score_for_the_sandbox_stays_safe() {
    let unit = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/systemd/opendeepcool-gpu-rank.service");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gpu-rank-unit-score");
    let level = units::exposure(&unit, Manager::System, &scratch);
    assert!(level <= 0.4, "exposure {level}");
}

#[test]
fn systemd_accepts_the_unit() {
    let unit = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging/systemd/opendeepcool-gpu-rank.service");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gpu-rank-unit-verify");
    units::verify(&unit, "/usr/lib/opendeepcool/odc-gpu-rank", &scratch).unwrap();
}
