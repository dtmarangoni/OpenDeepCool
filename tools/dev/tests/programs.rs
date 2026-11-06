//! The root steps start only the system tools on their list, never `cargo` or any build (Phase 5
//! plan, Task 8; solution review P5-03). Every place the dev tool and the manifest library start
//! a program is one of these: a listed `Tool`, `pkexec` and the build on the user's side, or the
//! manifest's `getent`, `gpasswd`, `groupdel`, `userdel` and `systemctl`.

use std::fs;
use std::path::Path;

const SOURCES: [&str; 5] = [
    "src/main.rs",
    "src/system.rs",
    "src/install.rs",
    "src/check.rs",
    "../../crates/odc-manifest/src/lib.rs",
];

fn source(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
    fs::read_to_string(&path).expect("a source file")
}

#[test]
fn programs_start_only_from_the_listed_places() {
    let allowed = [
        "Command::new(self.path())",
        "Command::new(PKEXEC)",
        "Command::new(cargo)",
        "Command::new(program)",
        "Command::new(GETENT)",
        "Command::new(SYSTEMCTL)",
    ];
    let mut starts = 0;
    for name in SOURCES {
        let text = source(name);
        for line in text.lines().filter(|l| l.contains("Command::new(")) {
            assert!(
                allowed.iter().any(|a| line.contains(a)),
                "{name}: {}",
                line.trim()
            );
            starts += 1;
        }
    }
    // Phase 9 plan, Task 4: the manifest's `systemctl --global` for the sound helper's user unit
    // made it 10.
    assert_eq!(starts, 10, "a new place that starts a program needs a look");
}

#[test]
fn only_setup_builds_and_only_through_cargo() {
    for name in &SOURCES[1..] {
        let text = source(name);
        assert!(
            !text.contains("\"cargo") && !text.contains("env!(\"CARGO\")"),
            "{name} names cargo"
        );
    }
    let main = source(SOURCES[0]);
    // The cargo that built this tool, never one looked up in PATH.
    assert_eq!(main.matches("env!(\"CARGO\")").count(), 1);
    assert!(main.contains("let cargo = env!(\"CARGO\");"));
    // `build(` is called once, from `setup`, the user's side, which refuses root first.
    let calls: Vec<&str> = main
        .split("\nfn ")
        .filter(|body| body.contains("build(&"))
        .map(|body| body.split('(').next().unwrap_or(""))
        .collect();
    assert_eq!(calls, ["setup"]);
    let setup = main.split("\nfn setup()").nth(1).expect("fn setup");
    let refuse = setup.find("refuse_root(").expect("setup refuses root");
    assert!(refuse < setup.find("build(&").expect("setup builds"));
}

#[test]
fn the_manifest_library_starts_only_its_five_tools() {
    let text = source(SOURCES[4]);
    let programs: Vec<&str> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("const "))
        .filter(|l| l.contains("\"/usr/bin/"))
        .collect();
    assert_eq!(programs.len(), 5, "{programs:?}");
    for (program, path) in [
        ("GETENT", "/usr/bin/getent"),
        ("GPASSWD", "/usr/bin/gpasswd"),
        ("GROUPDEL", "/usr/bin/groupdel"),
        ("USERDEL", "/usr/bin/userdel"),
        ("SYSTEMCTL", "/usr/bin/systemctl"),
    ] {
        assert!(
            programs
                .iter()
                .any(|l| l.starts_with(program) && l.contains(path)),
            "{program}"
        );
    }
}
