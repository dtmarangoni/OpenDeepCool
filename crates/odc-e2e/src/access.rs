//! The access check on the installed package (Phase 11 plan, Task 9): the polkit policy the PC
//! has holds `use`, `change-settings` and `feed-audio` for the active local session only, and
//! `start-at-boot` for the active local session with the administrator's password at every
//! change. `GetState` answering from this session, which polkit's `use` lets through, is the
//! service check's own first call. Where there's no service, the same check runs on the policy
//! the repository holds, which is the one the package installs.

use std::fs;
use std::path::PathBuf;

use odc_ipc::{CHANGE_SETTINGS_ACTION, FEED_AUDIO_ACTION, USE_ACTION};

/// The policy file's name, and where polkit reads policies from: the package's, and the dev
/// copy's.
pub const POLICY_FILE: &str = "io.github.dtmarangoni.OpenDeepCool1.policy";
pub const POLICY_DIRS: [&str; 2] = ["/usr/share/polkit-1/actions", "/etc/polkit-1/actions"];

/// The repository's policy, which the package installs.
pub const COMMITTED: &str =
    include_str!("../../../packaging/polkit/io.github.dtmarangoni.OpenDeepCool1.policy");

const START_AT_BOOT_ON: &str = odc_boot::ACTION_ON;
const START_AT_BOOT_OFF: &str = odc_boot::ACTION_OFF;

/// Each action and the one default that may be set: `allow_active`. `allow_any` and
/// `allow_inactive` are `no` in every action.
const EXPECTED: [(&str, &str); 5] = [
    (USE_ACTION, "yes"),
    (CHANGE_SETTINGS_ACTION, "yes"),
    (FEED_AUDIO_ACTION, "yes"),
    (START_AT_BOOT_ON, "auth_admin"),
    (START_AT_BOOT_OFF, "auth_admin"),
];

/// The text between `<tag>` and `</tag>` in `text`, trimmed.
fn element<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let rest = &text[text.find(&format!("<{tag}>"))? + tag.len() + 2..];
    Some(rest[..rest.find(&format!("</{tag}>"))?].trim())
}

/// The block of one action, from its `<action id=…>` to its `</action>`.
fn action<'a>(text: &'a str, id: &str) -> Option<&'a str> {
    let rest = &text[text.find(&format!("<action id=\"{id}\">"))?..];
    Some(&rest[..rest.find("</action>")?])
}

/// Whether `policy` holds the five actions with exactly those defaults, and no other action.
///
/// # Errors
///
/// The first action that differs, and how.
pub fn check_policy(policy: &str) -> Result<String, String> {
    let actions = policy.matches("<action id=").count();
    if actions != EXPECTED.len() {
        return Err(format!(
            "{actions} actions, not {}: nothing else may be allowed",
            EXPECTED.len()
        ));
    }
    for (id, active) in EXPECTED {
        let name = id.rsplit('.').next().unwrap_or(id);
        let block = action(policy, id).ok_or_else(|| format!("no action {name}"))?;
        for (tag, want) in [
            ("allow_any", "no"),
            ("allow_inactive", "no"),
            ("allow_active", active),
        ] {
            match element(block, tag) {
                Some(got) if got == want => {}
                Some(got) => return Err(format!("{name}: {tag} is {got:?}, not {want:?}")),
                None => return Err(format!("{name}: no {tag}")),
            }
        }
    }
    Ok(
        "use, change-settings and feed-audio are for the active local session only, \
        start-at-boot-on and start-at-boot-off ask the administrator's password"
            .to_owned(),
    )
}

/// Every policy file of ours under `dirs`, as polkit would read them.
pub fn installed(dirs: &[PathBuf]) -> Vec<(PathBuf, String)> {
    dirs.iter()
        .filter_map(|dir| {
            let path = dir.join(POLICY_FILE);
            let text = fs::read_to_string(&path).ok()?;
            Some((path, text))
        })
        .collect()
}

/// The check on what this PC has: every policy file of ours must pass, and there must be one.
///
/// # Errors
///
/// No policy file, or the first one that fails.
pub fn check_installed(dirs: &[PathBuf]) -> Result<String, String> {
    let found = installed(dirs);
    if found.is_empty() {
        let names: Vec<String> = dirs
            .iter()
            .map(|d| d.join(POLICY_FILE).display().to_string())
            .collect();
        return Err(format!(
            "no polkit policy installed: {}",
            names.join(" or ")
        ));
    }
    let mut notes = Vec::new();
    for (path, text) in found {
        let note = check_policy(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        notes.push(format!("{}: {note}", path.display()));
    }
    Ok(notes.join("; "))
}

/// The check where a service runs: this PC's installed policy.
pub fn real() -> Result<String, String> {
    let dirs: Vec<PathBuf> = POLICY_DIRS.iter().map(PathBuf::from).collect();
    check_installed(&dirs).map(|note| format!("GetState answered for this session; {note}"))
}

/// The check without a service: the policy the package installs.
pub fn fake() -> Result<String, String> {
    check_policy(COMMITTED).map(|note| format!("the repository's policy: {note}"))
}
