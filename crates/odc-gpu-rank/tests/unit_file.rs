//! The helper's systemd unit (Phase 6 plan, Task 9; solution review P6-03, P6-14): a one-shot
//! with a start time limit, run as the service's account, which makes the state directory itself
//! on a fresh install, runs only while no ranking exists, and orders itself before the display
//! service. The sandbox lines are the plan's; Phase 11 checks them again.

use std::fs;
use std::path::Path;

use odc_sensors::RANK_FILE;

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
    assert_eq!(values("Service", "ExecStart"), ["/usr/bin/odc-gpu-rank"]);
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

#[test]
fn its_sandbox_allows_only_the_gpu_devices() {
    for (key, value) in [
        ("ProtectSystem", "strict"),
        ("ProtectHome", "yes"),
        ("NoNewPrivileges", "yes"),
        ("PrivateNetwork", "yes"),
        ("CapabilityBoundingSet", ""),
        ("DevicePolicy", "closed"),
    ] {
        assert_eq!(values("Service", key), [value], "{key}");
    }
    assert_eq!(
        values("Service", "DeviceAllow"),
        ["char-drm rw", "char-nvidia* rw"]
    );
}
