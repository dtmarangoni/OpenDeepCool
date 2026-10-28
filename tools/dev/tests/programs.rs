//! The root steps start only the system tools on their list, never `cargo` or any build (Phase 5
//! plan, Task 8; solution review P5-03). Every place the dev tool and the manifest library start
//! a program is one of three: a listed `Tool`, `pkexec` on the user's side, or the manifest's
//! `getent`, `gpasswd` and `groupdel`.

use std::fs;
use std::path::Path;

const SOURCES: [&str; 5] = [
    "src/main.rs",
    "src/system.rs",
    "src/install.rs",
    "src/check.rs",
    "../../crates/odc-manifest/src/lib.rs",
];

#[test]
fn programs_start_only_from_the_listed_places() {
    let allowed = [
        "Command::new(self.path())",
        "Command::new(PKEXEC)",
        "Command::new(program)",
        "Command::new(GETENT)",
    ];
    let mut starts = 0;
    for source in SOURCES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(source);
        let text = fs::read_to_string(&path).expect("a source file");
        for line in text.lines().filter(|l| l.contains("Command::new(")) {
            assert!(
                allowed.iter().any(|a| line.contains(a)),
                "{source}: {}",
                line.trim()
            );
            starts += 1;
        }
        assert!(!text.contains("\"cargo"), "{source} names cargo");
    }
    assert_eq!(starts, 5, "a new place that starts a program needs a look");
}

#[test]
fn the_manifest_library_starts_only_getent_gpasswd_and_groupdel() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SOURCES[4]);
    let text = fs::read_to_string(path).expect("odc-manifest's source");
    let programs: Vec<&str> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("const "))
        .filter(|l| l.contains("\"/usr/bin/"))
        .collect();
    assert_eq!(programs.len(), 3, "{programs:?}");
    for (program, path) in [
        ("GETENT", "/usr/bin/getent"),
        ("GPASSWD", "/usr/bin/gpasswd"),
        ("GROUPDEL", "/usr/bin/groupdel"),
    ] {
        assert!(
            programs
                .iter()
                .any(|l| l.starts_with(program) && l.contains(path)),
            "{program}"
        );
    }
}
