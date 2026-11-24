//! The service's packaging (Phase 7 plan, Task 8): the unit's supervision, ordering, account and
//! sandbox; systemd's own check of the unit; the polkit actions, each for the active local
//! session only and owned by the service's account; and the D-Bus policy, scoped to the service.
//! And the Arch package's own files (Phase 11 plan, Task 4), read as text: `package()` installs
//! exactly `FILES.md`, `build()` builds exactly the crates whose programs ship, the scriptlets
//! enable and start nothing, their removal undoes every change under OpenDeepCool's names, and
//! the restart hook runs after systemd's and D-Bus's.
//! pacman brings `systemd-analyze` (systemd) and `xmllint` (libarchive's libxml2), so every Arch
//! system has both, CI's container included.

use std::collections::BTreeSet;

use std::fs;

use std::os::unix::fs::PermissionsExt;

use std::path::{Path, PathBuf};

use std::process::Command;

use odc_ipc::{
    BUS_NAME, CHANGE_SETTINGS_ACTION, DEV_PATH, FEED_AUDIO_ACTION, INTERFACE, OBJECT_PATH,
    USE_ACTION,
};

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

/// `packaging/FILES.md`'s first table, "Installed by the package": each row's path, kind, mode
/// and source, without the backticks.
fn installed() -> Vec<(String, String, String, String)> {
    let text = fs::read_to_string(packaging("FILES.md")).expect("packaging/FILES.md");
    let table = text
        .split("## Installed by the package")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .expect("the section \"Installed by the package\"");
    let bare = |cell: &str| cell.trim().trim_matches('`').to_owned();
    table
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').collect();
            assert_eq!(cells.len(), 5, "{line}");
            (
                bare(cells[0]),
                bare(cells[1]),
                bare(cells[3]),
                bare(cells[4]),
            )
        })
        .collect()
}

/// The paths in the first column of every table row of `text`: each backticked word that starts
/// with `/`. Words that are not paths (`<unit>.d/`, an account's name) are left out on both sides.
fn table_paths(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|line| line.starts_with("| "))
        .filter_map(|line| line.trim_start_matches('|').split('|').next())
        .flat_map(|cell| cell.split('`').skip(1).step_by(2))
        .filter(|word| word.starts_with('/'))
        .map(str::to_owned)
        .collect()
}

/// `FILES.md`'s "The PKGBUILD's own files": the files that build the package, never installed.
fn pkgbuild_files() -> Vec<String> {
    let text = fs::read_to_string(packaging("FILES.md")).expect("packaging/FILES.md");
    let table = text
        .split("## The PKGBUILD's own files")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .expect("the section \"The PKGBUILD's own files\"");
    table
        .lines()
        .filter(|line| line.starts_with("| `"))
        .filter_map(|line| line.split('`').nth(1).map(str::to_owned))
        .collect()
}

/// Every file under `dir`, relative to the repository, sorted.
fn files_under(dir: &Path, root: &Path, found: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("a packaging folder") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            files_under(&path, root, found);
        } else {
            let relative = path.strip_prefix(root).expect("inside the repository");
            found.push(relative.to_string_lossy().into_owned());
        }
    }
}

/// The PKGBUILD, or another file of the Arch package's.
fn arch(name: &str) -> String {
    fs::read_to_string(packaging(&format!("arch/{name}"))).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The body of shell function `name` in `text`: its lines after `name() {`, up to the `}` that
/// closes it at a line's start.
fn function<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let start = format!("{name}() {{");
    let mut lines = text.lines().skip_while(|line| *line != start);
    assert!(lines.next().is_some(), "no {name}()");
    lines.take_while(|line| *line != "}").collect()
}

/// The commands of `body`: lines joined at a trailing backslash, comments and blank lines left
/// out.
fn commands(body: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for line in body {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match line.strip_suffix('\\') {
            Some(start) => {
                current.push_str(start.trim_end());
                current.push(' ');
            }
            None => {
                current.push_str(line);
                out.push(std::mem::take(&mut current));
            }
        }
    }
    out
}

/// A PKGBUILD variable's value, as `name=value` gives it at a line's start, unquoted.
fn variable(text: &str, name: &str) -> String {
    let prefix = format!("{name}=");
    let value = text
        .lines()
        .find_map(|line| line.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("no {name}="));
    value.trim_matches(|c| c == '\'' || c == '"').to_owned()
}

/// What `package()` installs: each `install -Dm<mode> <from> <to>` as (to, mode, from), with
/// `$pkgdir` left out and its own variables written out; anything else in it fails.
fn package_installs() -> BTreeSet<(String, String, String)> {
    let text = arch("PKGBUILD");
    let pkgname = variable(&text, "pkgname");
    let body = commands(&function(&text, "package"));
    let id = body
        .iter()
        .find_map(|command| command.strip_prefix("local _id="))
        .expect("package()'s _id")
        .to_owned();
    let expand = |word: &str| {
        word.trim_matches('"')
            .replace("${_id}", &id)
            .replace("$_id", &id)
            .replace("$pkgname", &pkgname)
            .replace("$pkgdir", "")
    };
    let mut installs = BTreeSet::new();
    for command in &body {
        if command == "cd \"$pkgname\"" || command.starts_with("local _id=") {
            continue;
        }
        let words: Vec<&str> = command.split_whitespace().collect();
        match words.as_slice() {
            ["install", mode, from, to] if mode.starts_with("-Dm") => {
                let mode = mode.trim_start_matches("-Dm").to_owned();
                assert!(
                    installs.insert((expand(to), mode, expand(from))),
                    "{command}: installed twice"
                );
            }
            _ => panic!("package() runs {command:?}: only `install -Dm` is expected"),
        }
    }
    installs
}

/// The workspace crates whose programs are `names`, by each crate's `[[bin]]` names (or its own
/// name, for a `src/main.rs` with none).
fn crates_of(names: &BTreeSet<String>) -> BTreeSet<String> {
    let crates = packaging("../crates");
    let mut found = BTreeSet::new();
    for entry in fs::read_dir(&crates).expect("crates/") {
        let dir = entry.expect("an entry").path();
        let Ok(text) = fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let manifest: toml::Table = text.parse().expect("a Cargo.toml");
        let package = manifest
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(toml::Value::as_str)
            .expect("a package name")
            .to_owned();
        let mut bins: Vec<String> = manifest
            .get("bin")
            .and_then(toml::Value::as_array)
            .map(|bins| {
                bins.iter()
                    .filter_map(|bin| bin.get("name").and_then(toml::Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if bins.is_empty() && dir.join("src/main.rs").is_file() {
            bins.push(package.clone());
        }
        if bins.iter().any(|bin| names.contains(bin)) {
            found.insert(package);
        }
    }
    found
}

/// Each `rm` and `systemctl` that `pre_remove` runs, with its arguments, in a root that is a
/// chroot or not, booted by systemd or not: bash sources the scriptlets with both commands, and
/// the two checks systemd's hook script makes, replaced by shell functions, and a `PATH` with
/// nothing in it, so nothing is removed or disabled.
fn pre_remove_runs(chroot: bool, booted: bool) -> Vec<String> {
    let script = format!(
        "rm() {{ echo rm \"$@\"; }}\n\
         systemctl() {{ echo systemctl \"$@\"; }}\n\
         systemd-detect-virt() {{ return {}; }}\n\
         systemd-notify() {{ return {}; }}\n\
         . \"$1\"\n\
         pre_remove\n",
        u8::from(!chroot),
        u8::from(!booted)
    );
    let out = Command::new("/usr/bin/bash")
        .args(["-c", &script, "pre_remove"])
        .arg(packaging("arch/opendeepcool.install"))
        .env_clear()
        .env("PATH", "/nonexistent")
        .output()
        .expect("bash");
    let said = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    assert!(
        out.status.success() && out.stderr.is_empty(),
        "pre_remove: {}",
        said(&out.stderr)
    );
    said(&out.stdout).lines().map(str::to_owned).collect()
}

/// Every path where the system reads a file under the same name as one the package installs,
/// in front of it or beside it, in `/etc` and, outside a chroot, `/run`: a unit's copy or mask
/// and its drop-in folders (systemd.unit(5), the load path; `set-property` writes
/// `system.control`), and the same name for the account, udev, polkit, D-Bus and pacman's hooks
/// (sysusers.d(5), udev(7), polkit(8), dbus-daemon(1), alpm-hooks(5); the last two read no
/// folder in `/run`). A new kind of file fails until it's decided.
fn overrides(chroot: bool) -> BTreeSet<String> {
    let dirs: &[&str] = if chroot { &["/etc"] } else { &["/etc", "/run"] };
    let mut found = BTreeSet::new();
    for (path, kind, _, _) in installed() {
        if kind == "folder" {
            continue;
        }
        let (folder, name) = path.rsplit_once('/').expect("a full path");
        let under: Vec<String> = match folder {
            "/usr/lib/systemd/system" => ["system/{n}", "system/{n}.d", "system.control/{n}.d"]
                .iter()
                .map(|place| format!("systemd/{place}"))
                .collect(),
            "/usr/lib/systemd/user" => vec!["systemd/user/{n}".into(), "systemd/user/{n}.d".into()],
            "/usr/lib/sysusers.d" => vec!["sysusers.d/{n}".into()],
            "/usr/lib/udev/rules.d" => vec!["udev/rules.d/{n}".into()],
            "/usr/share/polkit-1/actions" => vec!["polkit-1/actions/{n}".into()],
            "/usr/share/dbus-1/system.d" => {
                found.insert(format!("/etc/dbus-1/system.d/{name}"));
                vec![]
            }
            "/usr/share/libalpm/hooks" => {
                found.insert(format!("/etc/pacman.d/hooks/{name}"));
                vec![]
            }
            // Programs, and files read only from /usr/local and home folders besides /usr.
            "/usr/bin"
            | "/usr/lib/opendeepcool"
            | "/usr/share/applications"
            | "/usr/share/icons/hicolor/scalable/apps"
            | "/usr/share/licenses/opendeepcool" => vec![],
            _ => panic!("{path}: where does the system read a file of the same name before it?"),
        };
        for dir in dirs {
            for place in &under {
                found.insert(format!("{dir}/{}", place.replace("{n}", name)));
            }
        }
    }
    found
}

mod files_md;
mod pkgbuild;
mod policy;
mod scriptlets;
mod unit;
