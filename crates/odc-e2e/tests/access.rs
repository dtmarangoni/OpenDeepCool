//! The access check on the installed package (Phase 11 plan, Task 9): what the polkit policy must
//! hold, and what makes the check fail. Policies here are files in a test's folder.

use std::fs;
use std::path::PathBuf;

use odc_e2e::access::{COMMITTED, POLICY_FILE, check_installed, check_policy, fake};

fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("access")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a folder");
    dir
}

/// The committed policy with `from` replaced by `to`, once.
fn changed(from: &str, to: &str) -> String {
    assert_eq!(COMMITTED.matches(from).count().min(1), 1, "{from}");
    COMMITTED.replacen(from, to, 1)
}

#[test]
fn the_committed_policy_is_what_the_package_must_have() {
    let note = check_policy(COMMITTED).expect("the committed policy");
    assert!(note.contains("start-at-boot"), "{note}");
    assert!(fake().is_ok());
}

#[test]
fn every_default_that_is_looser_or_missing_fails_and_names_the_action() {
    // The first `<allow_any>no`, `<allow_inactive>no` and `<allow_active>yes` are the `use`
    // action's, and the last `<allow_active>auth_admin` is `start-at-boot`'s.
    for (from, to, says) in [
        ("<allow_any>no", "<allow_any>yes", "use: allow_any"),
        (
            "<allow_inactive>no",
            "<allow_inactive>yes",
            "use: allow_inactive",
        ),
        (
            "<allow_active>yes",
            "<allow_active>auth_admin",
            "use: allow_active",
        ),
        (
            "<allow_active>auth_admin",
            "<allow_active>yes",
            "start-at-boot: allow_active is \"yes\"",
        ),
        (
            "<allow_active>auth_admin",
            "<allow_active>auth_admin_keep",
            "start-at-boot: allow_active is \"auth_admin_keep\"",
        ),
        (
            "<allow_active>yes",
            "<allow_active>no",
            "use: allow_active is \"no\"",
        ),
    ] {
        let why = check_policy(&changed(from, to)).expect_err(to);
        assert!(why.contains(says), "{to}: {why}");
    }
}

#[test]
fn an_action_that_is_missing_extra_or_without_a_default_fails() {
    let without = |id: &str| {
        let start = COMMITTED
            .find(&format!(
                "<action id=\"io.github.dtmarangoni.OpenDeepCool1.{id}\">"
            ))
            .expect("the action");
        let end =
            start + COMMITTED[start..].find("</action>").expect("its end") + "</action>".len();
        format!("{}{}", &COMMITTED[..start], &COMMITTED[end..])
    };
    // One fewer: the count, and the extra action, are both what's wrong.
    let why = check_policy(&without("feed-audio")).expect_err("one fewer");
    assert!(why.contains("3 actions, not 4"), "{why}");
    let extra = COMMITTED.replacen(
        "</policyconfig>",
        "<action id=\"io.github.dtmarangoni.OpenDeepCool1.more\"><defaults>\
         <allow_any>yes</allow_any></defaults></action></policyconfig>",
        1,
    );
    let why = check_policy(&extra).expect_err("one more");
    assert!(why.contains("5 actions, not 4"), "{why}");
    // Four actions, but one of ours renamed: it's missing.
    let renamed = COMMITTED.replacen("OpenDeepCool1.feed-audio", "OpenDeepCool1.feed-sound", 1);
    assert!(
        check_policy(&renamed)
            .expect_err("renamed")
            .contains("no action feed-audio")
    );
    // A default that isn't in its own action but in the next one's isn't taken for it.
    let moved = COMMITTED.replacen("<allow_inactive>no</allow_inactive>", "", 1);
    let why = check_policy(&moved).expect_err("moved");
    assert!(why.contains("use: no allow_inactive"), "{why}");
    assert!(check_policy("").is_err());
}

#[test]
fn the_installed_check_wants_a_file_and_every_file_of_ours_passes() {
    let (package, dev) = (dir("package"), dir("dev"));
    let both = [package.clone(), dev.clone()];
    let why = check_installed(&both).expect_err("nothing installed");
    assert!(why.contains("no polkit policy installed"), "{why}");
    assert!(why.contains(POLICY_FILE), "{why}");

    fs::write(package.join(POLICY_FILE), COMMITTED).expect("the package's");
    let note = check_installed(&both).expect("one passes");
    assert!(note.contains(&package.display().to_string()), "{note}");
    // The dev copy's, once it exists, is held to the same: a looser one fails the whole check,
    // naming its file.
    fs::write(
        dev.join(POLICY_FILE),
        changed("<allow_any>no", "<allow_any>yes"),
    )
    .expect("a bad one");
    let why = check_installed(&both).expect_err("the dev copy's is looser");
    assert!(
        why.contains(&dev.display().to_string()) && why.contains("allow_any"),
        "{why}"
    );
    fs::write(dev.join(POLICY_FILE), COMMITTED).expect("a good one");
    assert!(check_installed(&both).is_ok());
}
