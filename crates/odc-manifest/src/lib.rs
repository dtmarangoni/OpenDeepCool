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

/// The folders a recorded file may be in. Its name must contain `opendeepcool` too.
pub const DEV_DIRS: &[&str] = &["/etc/sysusers.d", "/etc/udev/rules.d"];

/// One thing the development setup created, or took over from the hand-made setup of 2026-10-24.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// `file <absolute path>`: a file it installed.
    File(PathBuf),
    /// `group opendeepcool`: the group.
    Group,
    /// `member opendeepcool <user>`: a user it added to the group.
    Member(String),
}

impl Entry {
    /// A file entry, checked.
    pub fn file(path: impl Into<PathBuf>) -> Result<Entry, String> {
        let entry = Entry::File(path.into());
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
            "group" if rest == GROUP => Entry::Group,
            "group" => return Err(format!("the group {rest:?} isn't {GROUP}")),
            "member" => match rest.split_once(' ') {
                Some((GROUP, user)) => Entry::Member(user.to_owned()),
                _ => return Err(format!("{line:?} isn't `member {GROUP} <user>`")),
            },
            _ => return Err(format!("{kind:?} isn't a kind of entry")),
        };
        entry.check()?;
        Ok(entry)
    }

    fn check(&self) -> Result<(), String> {
        match self {
            Entry::File(path) => check_file(path),
            Entry::Group => Ok(()),
            Entry::Member(user) => check_user(user),
        }
    }

    fn render(&self) -> String {
        match self {
            Entry::File(path) => format!("file {}", path.display()),
            Entry::Group => format!("group {GROUP}"),
            Entry::Member(user) => format!("member {GROUP} {user}"),
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
    if !DEV_DIRS.contains(&dir) || !name.contains("opendeepcool") {
        return Err(format!(
            "{text:?} isn't an OpenDeepCool file in {}",
            DEV_DIRS.join(" or ")
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

/// What removing needs from the system.
pub trait Host {
    /// Removes one file or link, never a folder.
    fn remove_file(&mut self, path: &Path) -> io::Result<()>;
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

/// The real system: the files themselves, and `getent`, `gpasswd` and `groupdel`, started
/// without a shell and by their full paths.
pub struct SystemHost;

impl Host for SystemHost {
    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
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
            ("dir /etc/udev/rules.d", "isn't a kind of entry"),
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
