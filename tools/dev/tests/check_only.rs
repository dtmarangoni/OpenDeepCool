//! `odc-dev teardown --check-only` (Phase 5 plan, Task 8; solution review P4-08): it accepts the
//! manifest the setup writes, and refuses a malformed one, saying why, with nothing removed.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const GOOD: &str = "file /etc/sysusers.d/opendeepcool.conf
file /etc/udev/rules.d/70-opendeepcool.rules
file /etc/udev/rules.d/71-opendeepcool-dev-reconnect.rules
group opendeepcool
member opendeepcool dev_user
";

fn check_only(name: &str, text: &str) -> (bool, String) {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::write(&path, text).expect("a manifest to check");
    let out = Command::new(env!("CARGO_BIN_EXE_odc-dev"))
        .args(["teardown", "--check-only"])
        .arg(&path)
        .output()
        .expect("odc-dev runs");
    let said =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), said)
}

#[test]
fn the_manifest_the_setup_writes_is_accepted() {
    let (ok, said) = check_only("good.manifest", GOOD);
    assert!(ok, "{said}");
    assert!(said.contains("accepted, 5 entries"), "{said}");
}

#[test]
fn malformed_manifests_are_refused_with_the_line() {
    let cases = [
        ("empty-line.manifest", format!("{GOOD}\n")),
        ("root.manifest", format!("{GOOD}file /\n")),
        (
            "dotdot.manifest",
            format!("{GOOD}file /etc/udev/rules.d/../passwd\n"),
        ),
        (
            "relative.manifest",
            format!("{GOOD}file etc/opendeepcool.conf\n"),
        ),
        (
            "outside.manifest",
            format!("{GOOD}file /usr/lib/opendeepcool.conf\n"),
        ),
    ];
    for (name, text) in cases {
        let (ok, said) = check_only(name, &text);
        assert!(!ok, "{name} was accepted: {said}");
        assert!(said.contains("line 6:"), "{name}: {said}");
        assert!(said.contains("nothing would be removed"), "{name}: {said}");
    }
}
