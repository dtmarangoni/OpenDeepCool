//! The oracle stays independent of the program it checks (Phase 5 plan, Task 0): it depends on no
//! crate, and no crate under `crates/` depends on it.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

#[test]
fn the_oracle_depends_on_no_crate() {
    let manifest = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .expect("tools/oracle/Cargo.toml is readable");
    for line in manifest.lines().map(str::trim) {
        assert!(
            !(line.starts_with('[') && line.contains("dependencies")),
            "tools/oracle/Cargo.toml has a dependency table: {line}"
        );
    }
}

#[test]
fn no_crate_of_the_program_depends_on_the_oracle() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates");
    let entries = match fs::read_dir(&crates) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return, // no crate yet
        Err(e) => panic!("{}: {e}", crates.display()),
    };
    for entry in entries {
        let manifest = entry.expect("a directory entry").path().join("Cargo.toml");
        match fs::read_to_string(&manifest) {
            Ok(text) => assert!(
                !text.contains("odc-oracle"),
                "{} names odc-oracle",
                manifest.display()
            ),
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => panic!("{}: {e}", manifest.display()),
        }
    }
}
