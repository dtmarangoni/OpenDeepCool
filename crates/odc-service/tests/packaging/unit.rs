use super::*;

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

// P6-02: it waits for the GPU ranking and for nothing else; it starts at boot with no login or
// session.
#[test]
fn the_unit_waits_for_the_ranking_and_starts_at_boot_with_no_session() {
    assert_eq!(values("Unit", "After"), ["opendeepcool-gpu-rank.service"]);
    assert_eq!(values("Unit", "Wants"), ["opendeepcool-gpu-rank.service"]);
    assert!(values("Unit", "Requires").is_empty());
    for (_, key, value) in unit() {
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
    assert_eq!(
        values("Service", "ExecStart"),
        ["/usr/lib/opendeepcool/opendeepcoold"]
    );
    for (key, value) in [
        ("User", "opendeepcool"),
        ("Group", "opendeepcool"),
        ("StateDirectory", "opendeepcool"),
        ("ProtectSystem", "strict"),
        ("ProtectHome", "yes"),
        ("PrivateTmp", "yes"),
        ("PrivateUsers", "yes"),
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
    ] {
        assert_eq!(values("Service", key), [value], "{key}");
    }
    assert_eq!(values("Service", "SystemCallFilter")[0], "@system-service");
}

/// The only devices the service may open: the cooler's hidraw node, and the two device groups
/// NVIDIA's library reads its GPUs through, named exactly so no other NVIDIA group opens (Phase 11
/// plan, Task 1). No wildcard, which would let in every group whose name starts the same.
#[test]
fn the_unit_opens_the_cooler_and_nvidias_two_reading_devices_only() {
    assert_eq!(
        values("Service", "DeviceAllow"),
        ["char-hidraw rw", "char-nvidiactl rw", "char-nvidia rw"]
    );
}

// systemd's own check, in a scratch root where the binary's path exists: it isn't installed here.
#[test]
fn systemd_accepts_the_unit() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("unit-root");
    let _ = fs::remove_dir_all(&root);
    let units = root.join("etc/systemd/system");
    fs::create_dir_all(&units).expect("a unit folder");
    fs::create_dir_all(root.join("usr/lib/opendeepcool")).expect("a program folder");
    let binary = root.join("usr/lib/opendeepcool/opendeepcoold");
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

#[test]
fn the_polkit_actions_are_for_the_active_session_and_owned_by_the_service() {
    let path = packaging(POLKIT);
    let text = fs::read_to_string(&path).expect("the policy");
    // The fourth and fifth are pkexec's, for the Start at boot switch, one each for turning it on
    // and off: `odc-boot`'s tests check them.
    assert_eq!(text.matches("<action id=").count(), 5);
    for action in [USE_ACTION, CHANGE_SETTINGS_ACTION, FEED_AUDIO_ACTION] {
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
