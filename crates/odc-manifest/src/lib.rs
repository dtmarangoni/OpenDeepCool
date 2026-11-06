//! The development setup's manifest: one line for each thing it created, so its teardown, and
//! later the package's install script, remove exactly those (PRD, System Footprint: Dev
//! manifest; owner, 2026-10-24).
//!
//! The whole manifest is checked before anything is removed (owner, 2026-10-27; solution review
//! P4-08). Entries are removed one at a time, the last first, never recursively, and each removal
//! is said (P11-03). One implementation serves the dev tool and the package (owner, 2026-10-28).

use std::fmt;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the development setup keeps its manifest.
pub const PATH: &str = "/var/lib/opendeepcool-dev/manifest";

/// The only group a manifest may name.
pub const GROUP: &str = "opendeepcool";

/// The shared folders a recorded file may be in; its name must contain `opendeepcool` too,
/// whatever the letter case.
pub const DEV_DIRS: &[&str] = &[
    "/etc/sysusers.d",
    "/etc/udev/rules.d",
    "/etc/systemd/system",
    "/etc/dbus-1/system.d",
    "/etc/polkit-1/actions",
    "/etc/systemd/user",
];

/// The dev copy's own folders (Phase 7 plan, Task 8; Phase 9 plan, Task 4): any file in them is
/// OpenDeepCool's.
pub const OWN_DIRS: &[&str] = &[
    "/usr/local/libexec/opendeepcool-dev",
    "/etc/systemd/system/opendeepcool.service.d",
    "/etc/systemd/system/opendeepcool-gpu-rank.service.d",
    "/etc/systemd/user/opendeepcool-audio.service.d",
];

/// The folders the setup may create, when they're missing, and so remove: its own, and the
/// shared ones Arch doesn't create (checked on the development PC, 2026-10-29). A folder is
/// removed only when empty.
pub const CREATED_DIRS: &[&str] = &[
    "/usr/local/libexec",
    "/usr/local/libexec/opendeepcool-dev",
    "/etc/dbus-1",
    "/etc/dbus-1/system.d",
    "/etc/polkit-1/actions",
    "/etc/systemd/system/opendeepcool.service.d",
    "/etc/systemd/system/opendeepcool-gpu-rank.service.d",
    "/etc/systemd/user/opendeepcool-audio.service.d",
];

/// The service's account (Phase 7 plan, Task 8: `u!` in the sysusers file).
pub const USER: &str = "opendeepcool";

/// The service's state folder, systemd's `StateDirectory=opendeepcool`: its saved data.
pub const STATE_DIR: &str = "/var/lib/opendeepcool";

/// The unit the setup enables at boot.
pub const UNIT: &str = "opendeepcool.service";

/// The user unit the setup enables for every user (Phase 9 plan, Task 4): the sound helper.
pub const USER_UNIT: &str = "opendeepcool-audio.service";

/// Whether a file in the state folder is one the service or its GPU rank helper writes: the
/// settings, their temporary and refused copies (`odc-service`, settings.rs), the ranking and its
/// temporary file (`odc-gpu-rank`), and the dev recording with its previous file (`odc-device`,
/// record.rs). Nothing else there is removed.
pub fn is_state_file(name: &str) -> bool {
    matches!(
        name,
        "settings.toml"
            | "settings.toml.new"
            | "gpu-rank.json"
            | ".gpu-rank.json.new"
            | "dev-frames.log"
            | "dev-frames.log.1"
    ) || name
        .strip_prefix("settings.toml.invalid-")
        .is_some_and(|stamp| {
            !stamp.is_empty() && stamp.chars().all(|c| c.is_ascii_digit() || c == '-')
        })
}

/// One thing the development setup created, or took over from the hand-made setup of 2026-10-24.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// `file <absolute path>`: a file it installed.
    File(PathBuf),
    /// `dir <absolute path>`: a folder it created, one of [`CREATED_DIRS`].
    Dir(PathBuf),
    /// `group opendeepcool`: the group.
    Group,
    /// `member opendeepcool <user>`: a user it added to the group.
    Member(String),
    /// `user opendeepcool`: the service's account.
    User,
    /// `state /var/lib/opendeepcool`: the service's saved data.
    State,
    /// `enabled opendeepcool.service`: the unit it enabled at boot.
    Enabled,
    /// `enabled-for-every-user opendeepcool-audio.service`: the user unit it enabled for every
    /// user, `systemctl --global enable`.
    EnabledForEveryUser,
}

impl Entry {
    /// A file entry, checked.
    pub fn file(path: impl Into<PathBuf>) -> Result<Entry, String> {
        let entry = Entry::File(path.into());
        entry.check()?;
        Ok(entry)
    }

    /// A folder entry, checked.
    pub fn dir(path: impl Into<PathBuf>) -> Result<Entry, String> {
        let entry = Entry::Dir(path.into());
        entry.check()?;
        Ok(entry)
    }

    /// A membership entry, checked.
    pub fn member(user: &str) -> Result<Entry, String> {
        let entry = Entry::Member(user.to_owned());
        entry.check()?;
        Ok(entry)
    }

    fn parse(line: &str) -> Result<Entry, String> {
        let Some((kind, rest)) = line.split_once(' ') else {
            return Err(format!("{line:?} isn't `<kind> <value>`"));
        };
        let entry = match kind {
            "file" => Entry::File(PathBuf::from(rest)),
            "dir" => Entry::Dir(PathBuf::from(rest)),
            "group" if rest == GROUP => Entry::Group,
            "group" => return Err(format!("the group {rest:?} isn't {GROUP}")),
            "member" => match rest.split_once(' ') {
                Some((GROUP, user)) => Entry::Member(user.to_owned()),
                _ => return Err(format!("{line:?} isn't `member {GROUP} <user>`")),
            },
            "user" if rest == USER => Entry::User,
            "user" => return Err(format!("the user {rest:?} isn't {USER}")),
            "state" if rest == STATE_DIR => Entry::State,
            "state" => return Err(format!("{rest:?} isn't {STATE_DIR}")),
            "enabled" if rest == UNIT => Entry::Enabled,
            "enabled" => return Err(format!("the unit {rest:?} isn't {UNIT}")),
            "enabled-for-every-user" if rest == USER_UNIT => Entry::EnabledForEveryUser,
            "enabled-for-every-user" => {
                return Err(format!("the user unit {rest:?} isn't {USER_UNIT}"));
            }
            _ => return Err(format!("{kind:?} isn't a kind of entry")),
        };
        entry.check()?;
        Ok(entry)
    }

    fn check(&self) -> Result<(), String> {
        match self {
            Entry::File(path) => check_file(path),
            Entry::Dir(path) => match path.to_str() {
                Some(text) if CREATED_DIRS.contains(&text) => Ok(()),
                _ => Err(format!(
                    "{} isn't a folder the setup creates",
                    path.display()
                )),
            },
            Entry::Group
            | Entry::User
            | Entry::State
            | Entry::Enabled
            | Entry::EnabledForEveryUser => Ok(()),
            Entry::Member(user) => check_user(user),
        }
    }

    fn render(&self) -> String {
        match self {
            Entry::File(path) => format!("file {}", path.display()),
            Entry::Dir(path) => format!("dir {}", path.display()),
            Entry::Group => format!("group {GROUP}"),
            Entry::Member(user) => format!("member {GROUP} {user}"),
            Entry::User => format!("user {USER}"),
            Entry::State => format!("state {STATE_DIR}"),
            Entry::Enabled => format!("enabled {UNIT}"),
            Entry::EnabledForEveryUser => format!("enabled-for-every-user {USER_UNIT}"),
        }
    }
}

/// An absolute, plain path, of an OpenDeepCool file in a dev location.
fn check_file(path: &Path) -> Result<(), String> {
    let Some(text) = path.to_str() else {
        return Err(format!("{} isn't UTF-8", path.display()));
    };
    if !text.starts_with('/') {
        return Err(format!("{text:?} isn't an absolute path"));
    }
    if text.chars().any(char::is_control)
        || text.contains("//")
        || text.ends_with('/')
        || text.split('/').any(|c| c == "." || c == "..")
    {
        return Err(format!("{text:?} isn't a plain path"));
    }
    let (dir, name) = text.rsplit_once('/').expect("an absolute path has a slash");
    let shared = DEV_DIRS.contains(&dir) && name.to_lowercase().contains("opendeepcool");
    if !shared && !OWN_DIRS.contains(&dir) {
        return Err(format!(
            "{text:?} isn't an OpenDeepCool file in {} or in {}",
            DEV_DIRS.join(", "),
            OWN_DIRS.join(", ")
        ));
    }
    Ok(())
}

/// A user name as shadow's tools take it: a lower-case letter or `_`, then up to 31 of `a-z`,
/// `0-9`, `_` and `-`. It can't start with `-`, so it can't pass for an option.
fn check_user(user: &str) -> Result<(), String> {
    let mut chars = user.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
    let rest_ok =
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if first_ok && rest_ok && user.len() <= 32 {
        Ok(())
    } else {
        Err(format!("{user:?} isn't a user name"))
    }
}

/// Why a manifest was refused. Nothing is removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The line, from 1.
    pub line: usize,
    pub reason: String,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.reason)
    }
}

impl std::error::Error for Refusal {}

/// Everything the development setup created, in the order it created it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    entries: Vec<Entry>,
}

impl Manifest {
    /// Checks every line before anything else happens: an empty line, `/`, a `..` component, a
    /// relative path, a path outside the dev locations, another group, a duplicate or anything
    /// else refuses the whole manifest.
    pub fn parse(text: &str) -> Result<Manifest, Refusal> {
        let mut manifest = Manifest::default();
        for (i, line) in text.lines().enumerate() {
            let refusal = |reason| Refusal {
                line: i + 1,
                reason,
            };
            let entry = Entry::parse(line).map_err(refusal)?;
            manifest.push(entry).map_err(refusal)?;
        }
        Ok(manifest)
    }

    /// Adds an entry, checked; an entry already there is refused.
    pub fn push(&mut self, entry: Entry) -> Result<(), String> {
        entry.check()?;
        if self.contains(&entry) {
            return Err(format!("{:?} is there twice", entry.render()));
        }
        self.entries.push(entry);
        Ok(())
    }

    pub fn contains(&self, entry: &Entry) -> bool {
        self.entries.contains(entry)
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The text of the manifest file, one entry per line.
    pub fn render(&self) -> String {
        self.entries.iter().map(|e| e.render() + "\n").collect()
    }

    /// Removes every entry, the last first, one at a time, and says each removal. What's already
    /// gone is said and passed over, so a teardown can run again. Returns what couldn't be
    /// removed.
    pub fn remove_all(&self, host: &mut impl Host) -> Vec<String> {
        let mut failures = Vec::new();
        for entry in self.entries.iter().rev() {
            let result = match entry {
                Entry::File(path) => match host.remove_file(path) {
                    Ok(()) => Ok(format!("removed {}", path.display())),
                    Err(e) if e.kind() == ErrorKind::NotFound => {
                        Ok(format!("{}: already gone", path.display()))
                    }
                    Err(e) => Err(format!("{}: {e}", path.display())),
                },
                Entry::Member(user) => match host.group_members(GROUP) {
                    Ok(Some(members)) if members.contains(user) => host
                        .remove_member(GROUP, user)
                        .map(|()| format!("removed {user} from {GROUP}"))
                        .map_err(|e| format!("{user} in {GROUP}: {e}")),
                    Ok(_) => Ok(format!("{user} in {GROUP}: already gone")),
                    Err(e) => Err(format!("{GROUP}: {e}")),
                },
                Entry::Group => match host.group_members(GROUP) {
                    Ok(Some(_)) => host
                        .remove_group(GROUP)
                        .map(|()| format!("removed the group {GROUP}"))
                        .map_err(|e| format!("the group {GROUP}: {e}")),
                    Ok(None) => Ok(format!("the group {GROUP}: already gone")),
                    Err(e) => Err(format!("{GROUP}: {e}")),
                },
                Entry::Dir(path) => match host.remove_dir(path) {
                    Ok(()) => Ok(format!("removed {}", path.display())),
                    Err(e) if e.kind() == ErrorKind::NotFound => {
                        Ok(format!("{}: already gone", path.display()))
                    }
                    // Something else keeps files there now: the folder is no longer only ours.
                    Err(e) if e.kind() == ErrorKind::DirectoryNotEmpty => {
                        Ok(format!("kept {}: not empty", path.display()))
                    }
                    Err(e) => Err(format!("{}: {e}", path.display())),
                },
                Entry::User => match host.user_exists(USER) {
                    Ok(true) => host
                        .remove_user(USER)
                        .map(|()| format!("removed the user {USER}"))
                        .map_err(|e| format!("the user {USER}: {e}")),
                    Ok(false) => Ok(format!("the user {USER}: already gone")),
                    Err(e) => Err(format!("{USER}: {e}")),
                },
                Entry::State => remove_state(host),
                Entry::Enabled => match host.disable(UNIT) {
                    Ok(true) => Ok(format!("stopped and disabled {UNIT}")),
                    Ok(false) => Ok(format!("{UNIT}: already gone")),
                    Err(e) => Err(format!("{UNIT}: {e}")),
                },
                Entry::EnabledForEveryUser => match host.disable_for_every_user(USER_UNIT) {
                    Ok(true) => Ok(format!("disabled {USER_UNIT} for every user")),
                    Ok(false) => Ok(format!("{USER_UNIT} for every user: already gone")),
                    Err(e) => Err(format!("{USER_UNIT}: {e}")),
                },
            };
            match result {
                Ok(line) => host.say(&line),
                Err(line) => {
                    host.say(&format!("failed: {line}"));
                    failures.push(line);
                }
            }
        }
        failures
    }
}

/// Removes the service's saved data: each file it writes, then the folder. A file it doesn't write
/// is kept, and said, and the folder with it.
fn remove_state(host: &mut impl Host) -> Result<String, String> {
    let dir = Path::new(STATE_DIR);
    let mut names = match host.list_dir(dir) {
        Ok(names) => names,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Ok(format!("{STATE_DIR}: already gone"));
        }
        Err(e) => return Err(format!("{STATE_DIR}: {e}")),
    };
    // In order, so a teardown says the same every time.
    names.sort();
    let (ours, others): (Vec<String>, Vec<String>) =
        names.into_iter().partition(|name| is_state_file(name));
    for name in &ours {
        let path = dir.join(name);
        match host.remove_file(&path) {
            Ok(()) => host.say(&format!("removed {}", path.display())),
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    if !others.is_empty() {
        return Err(format!(
            "{STATE_DIR}: kept, with files the service doesn't write: {}",
            others.join(", ")
        ));
    }
    host.remove_dir(dir)
        .map(|()| format!("removed {STATE_DIR}"))
        .map_err(|e| format!("{STATE_DIR}: {e}"))
}

/// What removing needs from the system.
pub trait Host {
    /// Removes one file or link, never a folder.
    fn remove_file(&mut self, path: &Path) -> io::Result<()>;
    /// Removes one empty folder, never its contents.
    fn remove_dir(&mut self, path: &Path) -> io::Result<()>;
    /// The names in a folder.
    fn list_dir(&mut self, path: &Path) -> io::Result<Vec<String>>;
    fn user_exists(&mut self, user: &str) -> io::Result<bool>;
    /// Removes the account, never a home folder.
    fn remove_user(&mut self, user: &str) -> io::Result<()>;
    /// Stops and disables a unit; `false` when it was neither enabled nor running.
    fn disable(&mut self, unit: &str) -> io::Result<bool>;
    /// Disables a user unit enabled for every user; `false` when it wasn't. Running copies in
    /// users' sessions are each user's to stop.
    fn disable_for_every_user(&mut self, unit: &str) -> io::Result<bool>;
    /// The group's members, or `None` when there's no such group.
    fn group_members(&mut self, group: &str) -> io::Result<Option<Vec<String>>>;
    fn remove_member(&mut self, group: &str, user: &str) -> io::Result<()>;
    fn remove_group(&mut self, group: &str) -> io::Result<()>;
    /// Says what was done, one line each.
    fn say(&mut self, line: &str);
}

const GETENT: &str = "/usr/bin/getent";
const GPASSWD: &str = "/usr/bin/gpasswd";
const GROUPDEL: &str = "/usr/bin/groupdel";
const USERDEL: &str = "/usr/bin/userdel";
const SYSTEMCTL: &str = "/usr/bin/systemctl";

/// The real system: the files themselves, and `getent`, `gpasswd`, `groupdel`, `userdel` and
/// `systemctl`, started without a shell and by their full paths.
pub struct SystemHost;

impl Host for SystemHost {
    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_dir(path)
    }

    fn list_dir(&mut self, path: &Path) -> io::Result<Vec<String>> {
        fs::read_dir(path)?
            .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
            .collect()
    }

    fn user_exists(&mut self, user: &str) -> io::Result<bool> {
        let out = Command::new(GETENT).args(["passwd", user]).output()?;
        match out.status.code() {
            Some(0) => Ok(true),
            Some(2) => Ok(false),
            _ => Err(io::Error::other(format!(
                "getent passwd {user}: {}",
                out.status
            ))),
        }
    }

    fn remove_user(&mut self, user: &str) -> io::Result<()> {
        // Never `-r`: the account's home is `/` (sysusers.d(5)).
        run(USERDEL, &[user])
    }

    fn disable(&mut self, unit: &str) -> io::Result<bool> {
        // Each fails for a unit that isn't so, or whose file is gone, alike.
        let is = |state: &str| -> io::Result<bool> {
            Ok(Command::new(SYSTEMCTL)
                .args(["--quiet", state, unit])
                .status()?
                .success())
        };
        let (enabled, active) = (is("is-enabled")?, is("is-active")?);
        if enabled {
            run(SYSTEMCTL, &["disable", "--now", unit])?;
        } else if active {
            run(SYSTEMCTL, &["stop", unit])?;
        }
        Ok(enabled || active)
    }

    fn disable_for_every_user(&mut self, unit: &str) -> io::Result<bool> {
        // It fails for a unit that isn't enabled, or whose file is gone, alike; disabling removes
        // the link and its folder, when empty (checked in a scratch root, 2026-11-05).
        let enabled = Command::new(SYSTEMCTL)
            .args(["--global", "--quiet", "is-enabled", unit])
            .status()?
            .success();
        if enabled {
            run(SYSTEMCTL, &["--global", "disable", unit])?;
        }
        Ok(enabled)
    }

    fn group_members(&mut self, group: &str) -> io::Result<Option<Vec<String>>> {
        group_entry(group).map(|e| e.map(|(_, members)| members))
    }

    fn remove_member(&mut self, group: &str, user: &str) -> io::Result<()> {
        run(GPASSWD, &["-d", user, group])
    }

    fn remove_group(&mut self, group: &str) -> io::Result<()> {
        run(GROUPDEL, &[group])
    }

    fn say(&mut self, line: &str) {
        println!("{line}");
    }
}

/// A group's ID and members from `getent group`, or `None` when it doesn't exist (exit status
/// 2, checked on the development PC, 2026-10-28).
pub fn group_entry(group: &str) -> io::Result<Option<(u32, Vec<String>)>> {
    let out = Command::new(GETENT).args(["group", group]).output()?;
    match out.status.code() {
        Some(0) => {
            let text = String::from_utf8_lossy(&out.stdout);
            let fields: Vec<&str> = text.trim_end().split(':').collect();
            let [_, _, gid, members] = fields.as_slice() else {
                return Err(io::Error::other(format!("getent group {group}: {text:?}")));
            };
            let gid = gid
                .parse()
                .map_err(|_| io::Error::other(format!("getent group {group}: gid {gid:?}")))?;
            let members = members
                .split(',')
                .filter(|m| !m.is_empty())
                .map(str::to_owned)
                .collect();
            Ok(Some((gid, members)))
        }
        Some(2) => Ok(None),
        _ => Err(io::Error::other(format!(
            "getent group {group}: {}",
            out.status
        ))),
    }
}

fn run(program: &'static str, args: &[&str]) -> io::Result<()> {
    let status = Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{program} {}: {status}",
            args.join(" ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the development setup writes in Phase 5.
    fn setup_manifest() -> Manifest {
        let mut m = Manifest::default();
        for f in [
            "/etc/sysusers.d/opendeepcool.conf",
            "/etc/udev/rules.d/70-opendeepcool.rules",
            "/etc/udev/rules.d/71-opendeepcool-dev-reconnect.rules",
        ] {
            m.push(Entry::file(f).expect("a dev file")).expect("new");
        }
        m.push(Entry::Group).expect("new");
        m.push(Entry::member("dev_user").expect("a user"))
            .expect("new");
        m
    }

    #[test]
    fn what_the_setup_writes_is_accepted() {
        let m = setup_manifest();
        assert_eq!(
            m.render(),
            "file /etc/sysusers.d/opendeepcool.conf\n\
             file /etc/udev/rules.d/70-opendeepcool.rules\n\
             file /etc/udev/rules.d/71-opendeepcool-dev-reconnect.rules\n\
             group opendeepcool\n\
             member opendeepcool dev_user\n"
        );
        assert_eq!(Manifest::parse(&m.render()), Ok(m));
        assert_eq!(Manifest::parse(""), Ok(Manifest::default()));
    }

    /// What the setup adds in Phase 7, after Phase 5's entries: the dev copy of the service.
    fn service_entries() -> Vec<Entry> {
        let mut entries: Vec<Entry> = [
            "/usr/local/libexec",
            "/usr/local/libexec/opendeepcool-dev",
            "/etc/systemd/system/opendeepcool.service.d",
            "/etc/systemd/system/opendeepcool-gpu-rank.service.d",
            "/etc/dbus-1",
            "/etc/dbus-1/system.d",
            "/etc/polkit-1/actions",
        ]
        .into_iter()
        .map(|d| Entry::dir(d).expect("a created folder"))
        .collect();
        for f in [
            "/usr/local/libexec/opendeepcool-dev/opendeepcoold",
            "/usr/local/libexec/opendeepcool-dev/odc-gpu-rank",
            "/etc/systemd/system/opendeepcool.service",
            "/etc/systemd/system/opendeepcool-gpu-rank.service",
            "/etc/systemd/system/opendeepcool.service.d/dev.conf",
            "/etc/systemd/system/opendeepcool-gpu-rank.service.d/dev.conf",
            "/etc/dbus-1/system.d/io.github.dtmarangoni.OpenDeepCool1.conf",
            "/etc/polkit-1/actions/io.github.dtmarangoni.OpenDeepCool1.policy",
        ] {
            entries.push(Entry::file(f).expect("a dev file"));
        }
        entries.extend([Entry::User, Entry::State, Entry::Enabled]);
        entries
    }

    #[test]
    fn what_the_setup_writes_for_the_service_is_accepted() {
        let mut m = setup_manifest();
        for entry in service_entries() {
            m.push(entry).expect("new");
        }
        let text = m.render();
        assert!(text.ends_with(
            "user opendeepcool\nstate /var/lib/opendeepcool\nenabled opendeepcool.service\n"
        ));
        assert_eq!(Manifest::parse(&text), Ok(m));
    }

    // Phase 9 plan, Task 4: the sound helper's binary, its user unit and dev drop-in, the
    // drop-in's folder, and the unit enabled for every user, last.
    #[test]
    fn what_the_setup_writes_for_the_sound_helper_is_accepted() {
        let mut m = setup_manifest();
        for entry in service_entries() {
            m.push(entry).expect("new");
        }
        m.push(Entry::dir("/etc/systemd/user/opendeepcool-audio.service.d").expect("created"))
            .expect("new");
        for f in [
            "/usr/local/libexec/opendeepcool-dev/opendeepcool-audio",
            "/etc/systemd/user/opendeepcool-audio.service",
            "/etc/systemd/user/opendeepcool-audio.service.d/dev.conf",
        ] {
            m.push(Entry::file(f).expect("a dev file")).expect("new");
        }
        m.push(Entry::EnabledForEveryUser).expect("new");
        let text = m.render();
        assert!(text.ends_with("enabled-for-every-user opendeepcool-audio.service\n"));
        assert_eq!(Manifest::parse(&text), Ok(m));
    }

    #[test]
    fn only_the_files_the_service_and_its_helper_write_are_state_files() {
        for name in [
            "settings.toml",
            "settings.toml.new",
            "settings.toml.invalid-1793232821",
            "settings.toml.invalid-1793232821-2",
            "gpu-rank.json",
            ".gpu-rank.json.new",
            "dev-frames.log",
            "dev-frames.log.1",
        ] {
            assert!(is_state_file(name), "{name}");
        }
        for name in [
            "settings.toml.invalid-",
            "settings.toml.invalid-x",
            "settings.toml.old",
            "dev-frames.log.2",
            "notes.txt",
            "..",
        ] {
            assert!(!is_state_file(name), "{name}");
        }
    }

    #[test]
    fn a_malformed_manifest_is_refused_whole() {
        let good = "file /etc/udev/rules.d/70-opendeepcool.rules\n";
        let refused = [
            ("\n", "isn't `<kind> <value>`"),
            ("file /", "isn't a plain path"),
            ("file /etc/udev/rules.d/../shadow", "isn't a plain path"),
            (
                "file /etc/udev/rules.d/./70-opendeepcool.rules",
                "isn't a plain path",
            ),
            (
                "file /etc/udev//rules.d/70-opendeepcool.rules",
                "isn't a plain path",
            ),
            (
                "file etc/udev/rules.d/70-opendeepcool.rules",
                "isn't an absolute path",
            ),
            ("file /etc/passwd", "isn't an OpenDeepCool file"),
            (
                "file /etc/udev/rules.d/99-other.rules",
                "isn't an OpenDeepCool file",
            ),
            (
                "file /etc/udev/rules.d/x/opendeepcool.rules",
                "isn't an OpenDeepCool file",
            ),
            ("group wheel", "isn't opendeepcool"),
            (
                "member wheel dev_user",
                "isn't `member opendeepcool <user>`",
            ),
            ("member opendeepcool -a", "isn't a user name"),
            ("member opendeepcool Dev", "isn't a user name"),
            ("dir /etc/udev/rules.d", "isn't a folder the setup creates"),
            ("dir /etc", "isn't a folder the setup creates"),
            (
                "dir /usr/local/libexec/opendeepcool-dev/..",
                "isn't a folder the setup creates",
            ),
            (
                "file /usr/local/libexec/opendeepcool-dev/x/opendeepcoold",
                "isn't an OpenDeepCool file",
            ),
            ("file /usr/local/libexec/sh", "isn't an OpenDeepCool file"),
            (
                "file /etc/systemd/system/sshd.service",
                "isn't an OpenDeepCool file",
            ),
            (
                "file /etc/systemd/system/opendeepcool.service.d/../../shadow",
                "isn't a plain path",
            ),
            ("user root", "isn't opendeepcool"),
            ("state /var/lib", "isn't /var/lib/opendeepcool"),
            (
                "state /var/lib/opendeepcool/..",
                "isn't /var/lib/opendeepcool",
            ),
            ("enabled sshd.service", "isn't opendeepcool.service"),
            (
                "enabled-for-every-user pipewire.service",
                "isn't opendeepcool-audio.service",
            ),
            (
                "file /etc/systemd/user/pipewire.service",
                "isn't an OpenDeepCool file",
            ),
            ("dir /etc/systemd/user", "isn't a folder the setup creates"),
            ("tmpfiles /etc/tmpfiles.d", "isn't a kind of entry"),
            (good.trim_end(), "is there twice"),
        ];
        for (bad, reason) in refused {
            let text = format!("{good}{bad}\n");
            let refusal = Manifest::parse(&text).expect_err(bad);
            assert_eq!(refusal.line, 2, "{bad:?}");
            assert!(
                refusal.reason.contains(reason),
                "{bad:?}: {}",
                refusal.reason
            );
        }
    }
}
