//! The service's packaging (Phase 7 plan, Task 8): the unit's supervision, ordering, account and
//! sandbox; systemd's own check of the unit; the polkit actions, each for the active local
//! session only and owned by the service's account; and the D-Bus policy, scoped to the service.
//! pacman brings `systemd-analyze` (systemd) and `xmllint` (libarchive's libxml2), so every Arch
//! system has both, CI's container included.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use odc_ipc::{BUS_NAME, CHANGE_SETTINGS_ACTION, DEV_PATH, INTERFACE, OBJECT_PATH, USE_ACTION};

fn packaging(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packaging")
        .join(path)
}

const UNIT: &str = "systemd/opendeepcool.service";
const POLKIT: &str = "polkit/io.github.dtmarangoni.OpenDeepCool1.policy";
const DBUS: &str = "dbus/io.github.dtmarangoni.OpenDeepCool1.conf";

/// Each `key=value` line of the unit, with its section, in order.
fn unit() -> Vec<(String, String, String)> {
    let text = fs::read_to_string(packaging(UNIT)).expect("the unit file");
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

// Supervision (Recommendation): a crash or a hang ends in a restart, with no limit.
#[test]
fn the_unit_restarts_a_crash_or_a_hang() {
    for (key, value) in [
        ("Type", "notify"),
        ("NotifyAccess", "main"),
        ("WatchdogSec", "15"),
        ("Restart", "on-failure"),
        ("RestartSec", "1"),
        ("RestartSteps", "5"),
        ("RestartMaxDelaySec", "30"),
    ] {
        assert_eq!(values("Service", key), [value], "{key}");
    }
    assert_eq!(values("Unit", "StartLimitIntervalSec"), ["0"]);
}

// P6-02, P6-04: it waits for the GPU ranking and the board's sensor driver, never for the
// detection, which it only pulls in; it starts at boot with no login or session.
#[test]
fn the_unit_waits_for_the_ranking_and_the_sensor_driver_never_the_detection() {
    assert_eq!(
        values("Unit", "After"),
        ["opendeepcool-gpu-rank.service lm_sensors.service"]
    );
    assert_eq!(
        values("Unit", "Wants"),
        ["opendeepcool-gpu-rank.service opendeepcool-sensors-detect.service"]
    );
    assert!(values("Unit", "Requires").is_empty());
    for (_, key, value) in unit() {
        if key == "After" || key == "Requires" {
            assert!(!value.contains("sensors-detect"), "{key}={value}");
        }
        for session in ["graphical.target", "login", "session", "user@"] {
            assert!(
                !(["After", "Requires", "Wants"].contains(&key.as_str())
                    && value.contains(session)),
                "{key}={value}"
            );
        }
    }
    assert_eq!(values("Install", "WantedBy"), ["multi-user.target"]);
}

#[test]
fn the_unit_runs_as_its_own_account_in_its_sandbox() {
    assert_eq!(values("Service", "ExecStart"), ["/usr/bin/opendeepcoold"]);
    for (key, value) in [
        ("User", "opendeepcool"),
        ("Group", "opendeepcool"),
        ("StateDirectory", "opendeepcool"),
        ("ProtectSystem", "strict"),
        ("ProtectHome", "yes"),
        ("PrivateTmp", "yes"),
        ("NoNewPrivileges", "yes"),
        ("CapabilityBoundingSet", ""),
        ("ProtectKernelTunables", "yes"),
        ("ProtectKernelModules", "yes"),
        ("ProtectKernelLogs", "yes"),
        ("ProtectControlGroups", "yes"),
        ("ProtectClock", "yes"),
        ("ProtectHostname", "yes"),
        ("RestrictAddressFamilies", "AF_UNIX"),
        ("RestrictNamespaces", "yes"),
        ("LockPersonality", "yes"),
        ("MemoryDenyWriteExecute", "yes"),
        ("RestrictRealtime", "yes"),
        ("SystemCallArchitectures", "native"),
        // Fixed (owner, 2026-10-27): a call outside the filter fails, never kills the service.
        ("SystemCallErrorNumber", "EPERM"),
        ("DevicePolicy", "closed"),
        ("DeviceAllow", "char-hidraw rw"),
    ] {
        assert_eq!(values("Service", key), [value], "{key}");
    }
    assert_eq!(values("Service", "SystemCallFilter")[0], "@system-service");
}

/// Runs `program` with `args`; its exit status and both outputs.
fn run(program: &str, args: &[&str]) -> (bool, String) {
    let out = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("{program}: {e}; pacman brings it on every Arch system"));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

// systemd's own check, in a scratch root where the binary's path exists: it isn't installed here.
#[test]
fn systemd_accepts_the_unit() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("unit-root");
    let _ = fs::remove_dir_all(&root);
    let units = root.join("etc/systemd/system");
    fs::create_dir_all(&units).expect("a unit folder");
    fs::create_dir_all(root.join("usr/bin")).expect("a bin folder");
    let binary = root.join("usr/bin/opendeepcoold");
    fs::write(&binary, "").expect("a stand-in");
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).expect("executable");
    let unit = units.join("opendeepcool.service");
    fs::copy(packaging(UNIT), &unit).expect("the unit copied");
    let root_arg = format!("--root={}", root.display());
    let (ok, out) = run(
        "systemd-analyze",
        &["verify", &root_arg, &unit.to_string_lossy()],
    );
    assert!(ok, "{out}");
}

/// The text between the first `<{tag}` that `start` matches and its `</{tag}>`.
fn block<'a>(text: &'a str, start: &str, tag: &str) -> &'a str {
    let from = text
        .find(start)
        .unwrap_or_else(|| panic!("{start} in the file"));
    let rest = &text[from..];
    let end = rest
        .find(&format!("</{tag}>"))
        .unwrap_or_else(|| panic!("the end of {start}"));
    &rest[..end]
}

#[test]
fn the_polkit_actions_are_for_the_active_session_and_owned_by_the_service() {
    let path = packaging(POLKIT);
    let text = fs::read_to_string(&path).expect("the policy");
    assert_eq!(text.matches("<action id=").count(), 2);
    for action in [USE_ACTION, CHANGE_SETTINGS_ACTION] {
        let block = block(&text, &format!("<action id=\"{action}\">"), "action");
        for line in [
            "<allow_any>no</allow_any>",
            "<allow_inactive>no</allow_inactive>",
            "<allow_active>yes</allow_active>",
            // polkit(8): without it, only root may ask about another user's client (owner,
            // 2026-10-29).
            "<annotate key=\"org.freedesktop.policykit.owner\">unix-user:opendeepcool</annotate>",
        ] {
            assert!(block.contains(line), "{action}: {line}");
        }
    }
    // Against polkit's DTD where polkit installs it; otherwise for form only.
    let dtd = Path::new("/usr/share/polkit-1/policyconfig-1.dtd");
    let path = path.to_string_lossy();
    let (ok, out) = if dtd.is_file() {
        run(
            "xmllint",
            &[
                "--noout",
                "--nonet",
                "--dtdvalid",
                &dtd.to_string_lossy(),
                &path,
            ],
        )
    } else {
        run("xmllint", &["--noout", "--nonet", &path])
    };
    assert!(ok, "{out}");
}

/// Each `<allow …/>` and `<deny …/>` rule's attributes.
fn rules(text: &str) -> Vec<Vec<(String, String)>> {
    let mut rules = Vec::new();
    for part in text.split('<').skip(1) {
        let Some(rest) = part
            .strip_prefix("allow ")
            .or_else(|| part.strip_prefix("deny "))
        else {
            continue;
        };
        let rest = rest.split("/>").next().expect("a rule");
        let pairs: Vec<&str> = rest.split('"').collect();
        rules.push(
            pairs
                .chunks(2)
                .filter_map(|pair| match pair {
                    [key, value] => Some((
                        key.trim().trim_end_matches('=').to_owned(),
                        (*value).to_owned(),
                    )),
                    _ => None,
                })
                .collect(),
        );
    }
    rules
}

// dbus-daemon(1): the interface field of a message is optional, so every send rule names the
// destination with an interface, a path or a message type; only the service's account owns the
// name; only group opendeepcool reaches the dev path.
#[test]
fn the_dbus_policy_is_scoped_to_the_service() {
    let path = packaging(DBUS);
    let text = fs::read_to_string(&path).expect("the policy");
    let (ok, out) = run("xmllint", &["--noout", "--nonet", &path.to_string_lossy()]);
    assert!(ok, "{out}");
    let all = rules(&text);
    assert_eq!(all.len(), 5);
    for rule in &all {
        let get = |key: &str| rule.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        if let Some(name) = get("own") {
            assert_eq!(name, BUS_NAME);
            continue;
        }
        assert_eq!(get("send_destination"), Some(BUS_NAME), "{rule:?}");
        assert!(
            get("send_interface").is_some()
                || get("send_path").is_some()
                || get("send_type").is_some(),
            "{rule:?}"
        );
    }
    let owner = block(&text, "<policy user=\"opendeepcool\">", "policy");
    assert!(owner.contains(&format!("<allow own=\"{BUS_NAME}\"/>")));
    let everyone = block(&text, "<policy context=\"default\">", "policy");
    assert!(everyone.contains(&format!("send_interface=\"{INTERFACE}\"")));
    assert!(everyone.contains(&format!("send_path=\"{OBJECT_PATH}\"")));
    assert!(
        !everyone.contains(DEV_PATH),
        "the dev path isn't everyone's"
    );
    let group = block(&text, "<policy group=\"opendeepcool\">", "policy");
    assert!(group.contains(&format!("send_path=\"{DEV_PATH}\"")));
}
