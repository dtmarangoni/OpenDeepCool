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
    // The fourth is pkexec's, for the Start at boot switch: `odc-boot`'s tests check it.
    assert_eq!(text.matches("<action id=").count(), 4);
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

/// The README's "Installed on your system" lists exactly the paths in `FILES.md`'s tables
/// (Phase 11 plan, Task 6; AGENTS.md rule 7), so what the README tells a user the package puts
/// on the PC can't drift from what it does: the other test here checks `FILES.md` against the
/// PKGBUILD and the repository.
#[test]
fn the_readme_lists_what_files_md_lists() {
    let files = fs::read_to_string(packaging("FILES.md")).expect("packaging/FILES.md");
    let readme = fs::read_to_string(packaging("../README.md")).expect("README.md");
    let section = readme
        .split("\n## Installed on your system\n")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .expect("the README's section \"Installed on your system\"");
    let listed = table_paths(section);
    let expected = table_paths(&files);
    assert!(
        expected.len() > 30,
        "{}: FILES.md's tables hold the package's paths",
        expected.len()
    );
    let missing: Vec<_> = expected.difference(&listed).collect();
    let extra: Vec<_> = listed.difference(&expected).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "the README misses {missing:?} and lists {extra:?} beyond FILES.md"
    );
}

#[test]
fn the_table_paths_are_the_first_column_s_backticked_paths() {
    let text = "| Path | What |\n|---|---|\n| `/a/b` | `/not/first` |\n\
                | `/c/<unit>`, `<unit>.d/` and `/d/` | x |\n| The account `opendeepcool` | y |\n\
                `/prose` is no table row\n";
    let paths: Vec<String> = table_paths(text).into_iter().collect();
    assert_eq!(paths, ["/a/b", "/c/<unit>", "/d/"]);
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

/// `FILES.md` matches the repository (Phase 11 plan, Task 1): every source it names exists,
/// every file under `packaging/` is installed from it, programs live only in `/usr/bin` (the app)
/// and `/usr/lib/opendeepcool/` (Arch package guidelines), and each unit starts a listed program.
#[test]
fn files_md_lists_what_the_package_installs() {
    let root = packaging("..");
    let rows = installed();
    assert!(!rows.is_empty(), "no rows");
    let mut sources = Vec::new();
    for (path, kind, _, from) in &rows {
        assert!(
            path.starts_with("/usr/"),
            "{path}: only /usr, never /etc or /usr/local"
        );
        assert!(!path.starts_with("/usr/local/"), "{path}");
        match kind.as_str() {
            "program" => {
                assert!(
                    path == "/usr/bin/opendeepcool" || path.starts_with("/usr/lib/opendeepcool/"),
                    "{path}: the app in /usr/bin, every other program in /usr/lib/opendeepcool/"
                );
                let name = path.rsplit('/').next().expect("a name");
                assert_eq!(from, &format!("target/release/{name}"), "{path}");
            }
            "file" => {
                assert!(root.join(from).is_file(), "{from} doesn't exist");
                sources.push(from.clone());
            }
            "folder" => assert_eq!(from, "-", "{path}"),
            other => panic!("{path}: unknown kind {other:?}"),
        }
    }
    let own = pkgbuild_files();
    assert_eq!(own.len(), 4, "{own:?}");
    for file in &own {
        assert!(root.join(file).is_file(), "{file} doesn't exist");
    }
    let mut packaged = Vec::new();
    files_under(&root.join("packaging"), &root, &mut packaged);
    for file in packaged.iter().filter(|f| *f != "packaging/FILES.md") {
        assert!(
            sources.contains(file) || own.contains(file),
            "{file} is in packaging/ but not in FILES.md"
        );
    }
    let programs: Vec<&str> = rows
        .iter()
        .filter(|(_, kind, _, _)| kind == "program")
        .map(|(path, _, _, _)| path.as_str())
        .collect();
    for unit in [
        "systemd/opendeepcool.service",
        "systemd/opendeepcool-gpu-rank.service",
        "systemd/opendeepcool-sensors-detect.service",
        "systemd-user/opendeepcool-audio.service",
    ] {
        let text = fs::read_to_string(packaging(unit)).expect("a unit");
        let exec = text
            .lines()
            .find_map(|line| line.strip_prefix("ExecStart="))
            .unwrap_or_else(|| panic!("{unit} has no ExecStart="));
        assert!(programs.contains(&exec), "{unit}: {exec} isn't installed");
        assert!(exec.starts_with("/usr/lib/opendeepcool/"), "{unit}: {exec}");
    }
}

/// systemd's own score for the sandbox stays at 0.4 "SAFE" or better (0.5 before Phase 11).
#[test]
fn systemds_score_for_the_sandbox_stays_safe() {
    let scratch = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("service-unit-score");
    let level = odc_testkit::units::exposure(
        &packaging(UNIT),
        odc_testkit::units::Manager::System,
        &scratch,
    );
    assert!(level <= 0.4, "exposure {level}");
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

/// `package()` installs exactly `FILES.md`'s first table, each file with its mode, from its
/// source; and nothing that enables a unit.
#[test]
fn package_installs_exactly_files_md() {
    let want: BTreeSet<(String, String, String)> = installed()
        .into_iter()
        .filter(|(_, kind, _, _)| kind != "folder")
        .map(|(path, _, mode, from)| (path, mode, from))
        .collect();
    let got = package_installs();
    assert_eq!(got, want);
    for (to, _, _) in &got {
        assert!(
            !to.contains(".wants/") && !to.contains(".requires/") && !to.starts_with("/etc/"),
            "{to}: the package enables nothing and ships nothing in /etc"
        );
    }
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

/// `build()` builds exactly the crates whose programs `package()` installs, so no test crate is
/// built on a user's PC; the functions follow Arch's Rust package guidelines, without
/// `--all-features`, which would start the app's weston tests.
#[test]
fn build_names_exactly_the_crates_whose_programs_ship() {
    let text = arch("PKGBUILD");
    let programs: BTreeSet<String> = installed()
        .into_iter()
        .filter(|(_, kind, _, _)| kind == "program")
        .map(|(path, _, _, _)| path.rsplit('/').next().expect("a name").to_owned())
        .collect();
    let build = commands(&function(&text, "build"));
    let cargo = build
        .iter()
        .find(|command| command.starts_with("cargo build "))
        .expect("build()'s cargo build");
    assert!(
        cargo.starts_with("cargo build --frozen --release -p "),
        "{cargo}"
    );
    let words: Vec<&str> = cargo.split_whitespace().collect();
    let built: BTreeSet<String> = words
        .windows(2)
        .filter(|pair| pair[0] == "-p")
        .map(|pair| pair[1].to_owned())
        .collect();
    assert_eq!(built.len(), words.iter().filter(|w| **w == "-p").count());
    assert_eq!(built, crates_of(&programs));
    assert_eq!(built.len(), 6, "{built:?}");

    let prepare = commands(&function(&text, "prepare"));
    assert!(prepare.contains(&"cargo fetch --locked --target host-tuple".to_owned()));
    let check = commands(&function(&text, "check"));
    assert!(
        check.contains(
            &"cargo test --frozen --workspace --exclude odc-e2e --exclude odc-ui-e2e --exclude \
          odc-pkg-test"
                .to_owned()
        )
    );
    for (name, body) in [("prepare", &prepare), ("build", &build), ("check", &check)] {
        assert!(
            body.contains(&"export RUSTUP_TOOLCHAIN=stable".to_owned()),
            "{name}()"
        );
        assert!(
            body.iter().all(|c| !c.contains("--all-features")),
            "{name}()"
        );
    }
    for (name, body) in [("build", &build), ("check", &check)] {
        assert!(
            body.contains(&"export CARGO_TARGET_DIR=target".to_owned()),
            "{name}()"
        );
    }
}

/// The PKGBUILD's fields: the release's signed tag through git, `v` and the version, a plain
/// `major.minor.patch` (0.1.z the alpha, 0.9.z the beta, 1.0.0 the first release; owner,
/// 2026-11-11), with no `pkgver()` (a release candidate is built from a temporary copy), and the
/// scriptlets' file; `.SRCINFO` holds the same version and dependencies.
#[test]
fn the_pkgbuild_builds_the_signed_tag() {
    let text = arch("PKGBUILD");
    assert_eq!(variable(&text, "pkgname"), "opendeepcool");
    assert_eq!(variable(&text, "arch"), "('x86_64')");
    assert_eq!(variable(&text, "license"), "('MIT')");
    assert_eq!(variable(&text, "install"), "opendeepcool.install");
    assert_eq!(variable(&text, "makedepends"), "('cargo' 'git')");
    // One package, so `pacman -R opendeepcool` removes everything: no split debug package
    // (owner, 2026-11-16), and no other option that changes what makepkg builds or strips.
    assert_eq!(variable(&text, "options"), "('!debug')");
    assert!(
        !text.contains("pkgname=("),
        "no split packages: pkgname is one name"
    );
    assert!(!text.contains("pkgver()"), "no pkgver()");
    assert_eq!(
        variable(&text, "source"),
        "(\"$pkgname::git+$url.git?signed#tag=v$pkgver\")"
    );
    let pkgver = variable(&text, "pkgver");
    let parts: Vec<&str> = pkgver.split('.').collect();
    assert!(
        parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
        "{pkgver}: major.minor.patch, digits only, so git, Cargo and pacman agree"
    );
    let srcinfo = arch(".SRCINFO");
    assert!(srcinfo.contains(&format!("\tpkgver = {pkgver}\n")));
    assert!(srcinfo.contains("\toptions = !debug\n"));
    assert_eq!(
        srcinfo.matches("pkgname = ").count(),
        1,
        ".SRCINFO names one package"
    );
    let depends: Vec<String> = text
        .split("depends=(\n")
        .nth(1)
        .and_then(|rest| rest.split("\n)").next())
        .expect("depends=( … )")
        .lines()
        .map(|line| line.trim().trim_matches('\'').to_owned())
        .collect();
    let listed: Vec<String> = srcinfo
        .lines()
        .filter_map(|line| line.strip_prefix("\tdepends = "))
        .map(str::to_owned)
        .collect();
    assert_eq!(listed, depends);
}

/// The scriptlets enable, start, restart and reload nothing. OpenDeepCool's units are stopped
/// and disabled only in `pre_remove`; `post_remove` may disable only `lm_sensors.service`, when
/// the detection's record says the detection set it up.
#[test]
fn the_scriptlets_enable_and_start_nothing() {
    let text = arch("opendeepcool.install");
    // The install message prints its commands as text, between `cat <<'EOF'` and `EOF`.
    let mut message = false;
    let run: Vec<&str> = text
        .lines()
        .filter(|line| {
            let inside = message;
            if line.trim_start().starts_with("cat <<'EOF'") {
                message = true;
            } else if *line == "EOF" {
                message = false;
            }
            !inside
        })
        .collect();
    for word in [
        "enable", "start", "restart", "reload", "--now", "preset", "link", "mask",
    ] {
        for line in run
            .iter()
            .filter(|l| l.trim_start().starts_with("systemctl"))
        {
            assert!(
                !line.split_whitespace().any(|w| w == word),
                "{line}: the scriptlets never {word}"
            );
        }
    }
    let code = run.join("\n");
    let systemctl = |name: &str| -> Vec<String> {
        commands(&function(&code, name))
            .into_iter()
            .filter(|line| line.starts_with("systemctl"))
            .collect()
    };
    assert_eq!(
        systemctl("_odc_undo_detection"),
        ["systemctl disable lm_sensors.service"]
    );
    for name in [
        "post_install",
        "post_remove",
        "_odc_power_counters",
        "_odc_overrides",
    ] {
        assert!(systemctl(name).is_empty(), "{name}()");
    }
    let message: String = function(&text, "post_install").join("\n");
    for command in [
        "sudo systemctl enable --now opendeepcool.service",
        "sudo systemctl --global enable opendeepcool-audio.service",
        "systemctl --user start opendeepcool-audio.service",
    ] {
        assert!(
            message.contains(command),
            "the install message prints {command}"
        );
    }
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

/// The removal undoes every change made to OpenDeepCool's units and files under its names
/// (owner, 2026-11-11): first the files an administrator made (copies, masks and drop-ins, and
/// same-named overrides), since `systemctl disable` skips a masked unit, then every link to the
/// units. In a chroot nothing in `/run` is touched, nor is the service stopped; and the service
/// is stopped only where systemd runs.
#[test]
fn the_removal_undoes_every_change_under_opendeepcools_names() {
    let names = |folder: &str| -> String {
        installed()
            .into_iter()
            .filter_map(|(path, ..)| {
                path.strip_prefix(folder)
                    .filter(|name| !name.contains('/'))
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let (system, user) = (
        names("/usr/lib/systemd/system/"),
        names("/usr/lib/systemd/user/"),
    );
    for (chroot, booted) in [(false, true), (false, false), (true, false), (true, true)] {
        let runs = pre_remove_runs(chroot, booted);
        let removed: BTreeSet<String> = runs
            .iter()
            .filter_map(|line| line.strip_prefix("rm -rfv "))
            .flat_map(|paths| paths.split(' ').map(str::to_owned))
            .collect();
        assert_eq!(removed, overrides(chroot), "chroot {chroot}");
        let mut wanted = Vec::new();
        if !chroot && booted {
            wanted.push("systemctl stop opendeepcool.service".to_owned());
        }
        wanted.push(format!("systemctl -q --global disable {user}"));
        wanted.push(format!("systemctl -q disable {system}"));
        if !chroot {
            wanted.push(format!("systemctl -q --global --runtime disable {user}"));
            wanted.push(format!("systemctl -q --runtime disable {system}"));
        }
        let rest: Vec<String> = runs
            .iter()
            .filter(|line| !line.starts_with("rm -rfv "))
            .cloned()
            .collect();
        assert_eq!(rest, wanted, "chroot {chroot}, booted {booted}");
        // The stop comes before anything is removed, and every removal before the links.
        let first_link = runs.iter().position(|l| l.contains(" disable ")).unwrap();
        let last_rm = runs.iter().rposition(|l| l.starts_with("rm ")).unwrap();
        assert!(last_rm < first_link, "{runs:#?}");
        if !chroot && booted {
            assert!(runs[0].starts_with("systemctl stop"), "{runs:#?}");
        }
    }
}

/// The restart hook: after an upgrade of this package, systemd's own script restarts the service
/// if it runs; its file name sorts after systemd's numbered hooks and `dbus-reload.hook`, and,
/// on an Arch system, after every systemd and D-Bus hook installed (alpm-hooks(5): hooks run in
/// the alphabetical order of their file names).
#[test]
fn the_restart_hook_runs_after_systemds_and_dbus_s() {
    let hooks: Vec<String> = fs::read_dir(packaging("arch/hooks"))
        .expect("packaging/arch/hooks")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(hooks, ["opendeepcool-restart.hook"]);
    let name = hooks[0].as_str();
    let text = arch(&format!("hooks/{name}"));
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        lines,
        [
            "[Trigger]",
            "Operation = Upgrade",
            "Type = Package",
            "Target = opendeepcool",
            "[Action]",
            "Description = Restart a running opendeepcool.service",
            "When = PostTransaction",
            "Exec = /usr/share/libalpm/scripts/systemd-hook restart opendeepcool.service",
        ]
    );
    // systemd's hooks are numbered; D-Bus's is `dbus-reload.hook`.
    for before in ["99-", "dbus-reload.hook"] {
        assert!(name > before, "{name} runs before {before}…");
    }
    if let Ok(hooks) = fs::read_dir("/usr/share/libalpm/hooks") {
        for hook in hooks.flatten() {
            let other = hook.file_name().to_string_lossy().into_owned();
            if other.contains("systemd") || other.contains("dbus") {
                assert!(name > other.as_str(), "{other} runs after {name}");
            }
        }
    }
}

/// The package sources' own license is the 0BSD text Arch publishes for them, word for word
/// (Arch package guidelines, Package sources licenses; AUR submission guidelines).
#[test]
fn the_package_sources_license_is_arch_s_0bsd() {
    assert_eq!(
        arch("LICENSE"),
        "Copyright Arch Linux Contributors\n\
         \n\
         Permission to use, copy, modify, and/or distribute this software for\n\
         any purpose with or without fee is hereby granted.\n\
         \n\
         THE SOFTWARE IS PROVIDED “AS IS” AND THE AUTHOR DISCLAIMS ALL\n\
         WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES\n\
         OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE\n\
         FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY\n\
         DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN\n\
         AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT\n\
         OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.\n"
    );
}
