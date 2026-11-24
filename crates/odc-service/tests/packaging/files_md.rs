use super::*;

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

/// The commands the install message prints to turn OpenDeepCool on are in the README word for
/// word, so the message and the README tell a user the same thing.
#[test]
fn the_readme_gives_the_install_message_s_commands() {
    let install = arch("opendeepcool.install");
    let message = install
        .split("cat <<'EOF'\n")
        .nth(1)
        .and_then(|rest| rest.split("\nEOF\n").next())
        .expect("the install message");
    let commands: Vec<&str> = message
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("sudo systemctl") || line.starts_with("systemctl"))
        .collect();
    assert_eq!(commands.len(), 3, "{commands:?}");
    let readme = fs::read_to_string(packaging("../README.md")).expect("README.md");
    for command in &commands {
        assert!(
            readme.lines().any(|line| line == *command),
            "the README lacks the install message's {command:?}"
        );
    }
}

#[test]
fn the_table_paths_are_the_first_column_s_backticked_paths() {
    let text = "| Path | What |\n|---|---|\n| `/a/b` | `/not/first` |\n\
                | `/c/<unit>`, `<unit>.d/` and `/d/` | x |\n| The account `opendeepcool` | y |\n\
                `/prose` is no table row\n";
    let paths: Vec<String> = table_paths(text).into_iter().collect();
    assert_eq!(paths, ["/a/b", "/c/<unit>", "/d/"]);
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
