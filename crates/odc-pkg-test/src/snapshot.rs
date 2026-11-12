//! A throwaway root's state as the comparison sees it (Phase 11 plan, Task 5): every path under
//! it, with its type, mode, owner, group, size, link target and, for a regular file, the SHA-256
//! of its content. No time is recorded, so a file rewritten with the same content and metadata
//! is unchanged.
//!
//! The walk never follows a link and reads every file, so it runs inside the user namespace that
//! owns the root's files (`--snapshot`, through `unshare`), where it is root.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use sha2::{Digest, Sha256};

/// One path's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `f` a regular file, `d` a folder, `l` a symbolic link, `p` a FIFO, `s` a socket, `c` and
    /// `b` device nodes.
    pub kind: char,
    /// The permission bits with set-ID and sticky bits, as `stat` gives them.
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    /// A link's target, a regular file's SHA-256 in hex, or nothing.
    pub data: String,
}

/// Every path under a root, as `/etc/passwd`, by its escaped name (`escape`).
pub type Snapshot = BTreeMap<String, Entry>;

/// A path or link target as text: printable ASCII as it is, every other byte and the backslash
/// as `\xNN`, so a line holds one entry whatever the name.
pub fn escape(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if (0x20..0x7f).contains(&b) && b != b'\\' {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("\\x{b:02x}"));
        }
    }
    out
}

/// A file's SHA-256 in hex.
pub fn sha256(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// The state of every path under `root`, the root itself as `/`, leaving out the subtrees in
/// `skip` (paths such as `/home/builder`, which stay in the snapshot themselves).
pub fn take(root: &Path, skip: &[&str]) -> io::Result<Snapshot> {
    let mut snapshot = Snapshot::new();
    walk(root, "/", skip, &mut snapshot)?;
    Ok(snapshot)
}

fn walk(path: &Path, name: &str, skip: &[&str], snapshot: &mut Snapshot) -> io::Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|e| at(path, e))?;
    let kind = meta.file_type();
    let (letter, data) = if kind.is_symlink() {
        let target = fs::read_link(path).map_err(|e| at(path, e))?;
        ('l', escape(target.as_os_str().as_bytes()))
    } else if kind.is_file() {
        ('f', sha256(path).map_err(|e| at(path, e))?)
    } else if kind.is_dir() {
        ('d', String::new())
    } else if kind.is_fifo() {
        ('p', String::new())
    } else if kind.is_socket() {
        ('s', String::new())
    } else if kind.is_char_device() {
        ('c', format!("{:x}", meta.rdev()))
    } else {
        ('b', format!("{:x}", meta.rdev()))
    };
    snapshot.insert(
        name.to_owned(),
        Entry {
            kind: letter,
            mode: meta.mode() & 0o7777,
            uid: meta.uid(),
            gid: meta.gid(),
            // A folder's size depends on the file system's history, not on what it holds.
            size: if letter == 'd' { 0 } else { meta.len() },
            data,
        },
    );
    if letter == 'd' && !skip.contains(&name) {
        let mut children: Vec<_> = fs::read_dir(path)
            .map_err(|e| at(path, e))?
            .collect::<io::Result<_>>()
            .map_err(|e| at(path, e))?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let escaped = escape(child.file_name().as_bytes());
            let child_name = if name == "/" {
                format!("/{escaped}")
            } else {
                format!("{name}/{escaped}")
            };
            walk(&child.path(), &child_name, skip, snapshot)?;
        }
    }
    Ok(())
}

fn at(path: &Path, e: io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

/// The snapshot as text, one entry a line: name, type, mode in octal, owner, group, size and
/// data, separated by tabs.
pub fn render(snapshot: &Snapshot) -> String {
    let mut out = String::new();
    for (name, e) in snapshot {
        out.push_str(&format!(
            "{name}\t{}\t{:o}\t{}\t{}\t{}\t{}\n",
            e.kind, e.mode, e.uid, e.gid, e.size, e.data
        ));
    }
    out
}

/// The text `render` writes.
pub fn parse(text: &str) -> Result<Snapshot, String> {
    let mut snapshot = Snapshot::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let [name, kind, mode, uid, gid, size, data] = fields[..] else {
            return Err(format!("not a snapshot line: {line:?}"));
        };
        let number = |field: &str| -> Result<u64, String> {
            field
                .parse()
                .map_err(|_| format!("not a number in {line:?}"))
        };
        let small = |field: &str| -> Result<u32, String> {
            u32::try_from(number(field)?).map_err(|_| format!("too large in {line:?}"))
        };
        let mut letters = kind.chars();
        let (Some(kind), None) = (letters.next(), letters.next()) else {
            return Err(format!("not a type in {line:?}"));
        };
        snapshot.insert(
            name.to_owned(),
            Entry {
                kind,
                mode: u32::from_str_radix(mode, 8)
                    .map_err(|_| format!("not a mode in {line:?}"))?,
                uid: small(uid)?,
                gid: small(gid)?,
                size: number(size)?,
                data: data.to_owned(),
            },
        );
    }
    Ok(snapshot)
}

/// The differences the comparison accepts, each for a stated reason.
#[derive(Clone, Debug, Default)]
pub struct Allowed {
    /// Paths whose every change is accepted, with the subtree under each.
    pub anything: Vec<String>,
    /// Backup files (`/etc/passwd-`) that may change, as long as they then hold what the file
    /// they back up (`/etc/passwd`) held before.
    pub backups: Vec<(String, String)>,
    /// Paths whose names start so, with the subtree under each: pacman's database entries of
    /// one package's releases (`/var/lib/pacman/local/opendeepcool-`).
    pub starts: Vec<String>,
    /// A backup's other accepted content, by its SHA-256: what its file held just before the
    /// change being compared, which shadow-utils saves in it.
    pub held: Vec<(String, String)>,
}

impl Allowed {
    fn covers(&self, name: &str) -> bool {
        self.starts
            .iter()
            .any(|start| name.starts_with(start.as_str()))
            || self.anything.iter().any(|path| {
                name == path
                    || name
                        .strip_prefix(path.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            })
    }
}

/// A path that differs between two snapshots: missing before (added), missing after (removed),
/// or both (changed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    pub name: String,
    pub before: Option<Entry>,
    pub after: Option<Entry>,
}

impl std::fmt::Display for Difference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = &self.name;
        match (&self.before, &self.after) {
            (Some(_), None) => write!(f, "removed {name}"),
            (None, Some(a)) => write!(f, "added {name} ({})", describe(a)),
            (Some(b), Some(a)) => write!(f, "changed {name}: {} → {}", describe(b), describe(a)),
            (None, None) => write!(f, "{name}"),
        }
    }
}

/// Every difference between `before` and `after` that `allowed` doesn't accept, in path order.
pub fn compare(before: &Snapshot, after: &Snapshot, allowed: &Allowed) -> Vec<Difference> {
    let mut out = Vec::new();
    let names: std::collections::BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    for name in names {
        if allowed.covers(name) {
            continue;
        }
        let (b, a) = (before.get(name), after.get(name));
        if b == a {
            continue;
        }
        if let Some(a) = a
            && a.kind == 'f'
            && let Some((_, original)) = allowed.backups.iter().find(|(file, _)| file == name)
            && (before.get(original).map(|e| &e.data) == Some(&a.data)
                || allowed
                    .held
                    .iter()
                    .any(|(file, sha)| file == name && *sha == a.data))
        {
            continue;
        }
        out.push(Difference {
            name: name.clone(),
            before: b.cloned(),
            after: a.cloned(),
        });
    }
    out
}

/// The differences as text, one a line, at most `limit` of them and how many more.
pub fn lines(differences: &[Difference], limit: usize) -> String {
    let mut out: Vec<String> = differences
        .iter()
        .take(limit)
        .map(ToString::to_string)
        .collect();
    if differences.len() > limit {
        out.push(format!("… and {} more", differences.len() - limit));
    }
    out.join("; ")
}

fn describe(e: &Entry) -> String {
    let data = if e.data.is_empty() {
        String::new()
    } else if e.kind == 'f' {
        format!(" sha256 {}", &e.data[..e.data.len().min(12)])
    } else {
        format!(" → {}", e.data)
    };
    format!(
        "{} {:o} {}:{} {} bytes{data}",
        e.kind, e.mode, e.uid, e.gid, e.size
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("odc-pkg-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn every_path_is_recorded_without_following_links() {
        let root = scratch("snapshot-walk");
        fs::create_dir_all(root.join("etc/systemd")).unwrap();
        fs::write(root.join("etc/passwd"), "root:x:0:0::/root:/bin/bash\n").unwrap();
        fs::set_permissions(root.join("etc/passwd"), fs::Permissions::from_mode(0o644)).unwrap();
        symlink("/usr/lib/x.service", root.join("etc/systemd/x.service")).unwrap();
        symlink("/", root.join("etc/up")).unwrap();
        fs::create_dir_all(root.join("home/builder/big")).unwrap();
        fs::write(root.join("home/builder/big/file"), "x").unwrap();
        let snapshot = take(&root, &["/home/builder"]).unwrap();
        let names: Vec<&str> = snapshot.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            [
                "/",
                "/etc",
                "/etc/passwd",
                "/etc/systemd",
                "/etc/systemd/x.service",
                "/etc/up",
                "/home",
                "/home/builder"
            ]
        );
        let passwd = &snapshot["/etc/passwd"];
        assert_eq!((passwd.kind, passwd.mode, passwd.size), ('f', 0o644, 28));
        assert_eq!(passwd.data.len(), 64);
        assert_eq!(
            snapshot["/etc/systemd/x.service"].data,
            "/usr/lib/x.service"
        );
        assert_eq!(snapshot["/etc/up"].kind, 'l');
        assert_eq!(parse(&render(&snapshot)).unwrap(), snapshot);
    }

    #[test]
    fn sha256_is_the_standard_one() {
        let root = scratch("snapshot-sha");
        fs::write(root.join("f"), "hello world").unwrap();
        assert_eq!(
            sha256(&root.join("f")).unwrap(),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn names_with_any_byte_stay_on_one_line() {
        assert_eq!(escape(b"a b\tc\nd\\e\xff"), "a b\\x09c\\x0ad\\x5ce\\xff");
    }

    fn file(data: &str) -> Entry {
        Entry {
            kind: 'f',
            mode: 0o644,
            uid: 0,
            gid: 0,
            size: 1,
            data: data.to_owned(),
        }
    }

    #[test]
    fn only_the_allowed_differences_pass() {
        let mut before = Snapshot::new();
        before.insert("/etc/passwd".into(), file("old"));
        before.insert("/etc/passwd-".into(), file("older"));
        before.insert("/var/log/pacman.log".into(), file("a"));
        before.insert("/etc/gone".into(), file("x"));
        before.insert("/etc/mode".into(), file("m"));
        let mut after = before.clone();
        after.insert("/etc/passwd-".into(), file("old"));
        after.insert("/var/log/pacman.log".into(), file("b"));
        after.insert("/var/log/pacman.log/x".into(), file("b"));
        after.remove("/etc/gone");
        after.insert("/etc/new".into(), file("n"));
        after.get_mut("/etc/mode").unwrap().mode = 0o600;
        let allowed = Allowed {
            anything: vec!["/var/log/pacman.log".into()],
            backups: vec![("/etc/passwd-".into(), "/etc/passwd".into())],
            starts: vec!["/var/lib/pacman/local/opendeepcool-".into()],
            held: Vec::new(),
        };
        let mut db = after.clone();
        db.insert(
            "/var/lib/pacman/local/opendeepcool-0.1.0-2/desc".into(),
            file("d"),
        );
        assert_eq!(
            compare(&after, &db, &allowed).len(),
            0,
            "a release's database entry"
        );
        let found: Vec<String> = compare(&before, &after, &allowed)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            found,
            [
                "removed /etc/gone",
                "changed /etc/mode: f 644 0:0 1 bytes sha256 m → f 600 0:0 1 bytes sha256 m",
                "added /etc/new (f 644 0:0 1 bytes sha256 n)",
            ]
        );
        // A backup that holds anything else is a difference, and so is one that didn't exist
        // and holds something else.
        after.insert("/etc/passwd-".into(), file("forged"));
        assert_eq!(compare(&before, &after, &allowed).len(), 4);
        before.remove("/etc/passwd-");
        after.insert("/etc/passwd-".into(), file("old"));
        assert_eq!(compare(&before, &after, &allowed).len(), 3);
        // What the file held just before the change is accepted too, when it's given.
        after.insert("/etc/passwd-".into(), file("between"));
        assert_eq!(compare(&before, &after, &allowed).len(), 4);
        let held = Allowed {
            held: vec![("/etc/passwd-".into(), "between".into())],
            ..allowed.clone()
        };
        assert_eq!(compare(&before, &after, &held).len(), 3);
        // A prefix of a name isn't its folder.
        let near = Allowed {
            anything: vec!["/var/log/pacman".into()],
            ..Allowed::default()
        };
        assert!(!near.covers("/var/log/pacman.log"));
        assert!(near.covers("/var/log/pacman/x"));
    }
}
