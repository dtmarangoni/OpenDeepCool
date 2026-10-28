//! Removal in a temporary folder (Phase 5 plan, Task 8; solution review P11-03): the last entry
//! first, one at a time, never a folder's contents, and a second run finds everything gone.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use odc_manifest::{Entry, GROUP, Host, Manifest};

/// Files under a temporary root, and the group in memory.
struct TestHost {
    root: PathBuf,
    members: Option<Vec<String>>,
    said: Vec<String>,
}

impl TestHost {
    fn new(name: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("etc/sysusers.d")).expect("a temporary /etc");
        fs::create_dir_all(root.join("etc/udev/rules.d")).expect("a temporary /etc");
        TestHost {
            root,
            members: Some(vec!["dev_user".to_owned(), "other".to_owned()]),
            said: Vec::new(),
        }
    }

    fn under_root(&self, path: &Path) -> PathBuf {
        self.root
            .join(path.strip_prefix("/").expect("an absolute path"))
    }
}

impl Host for TestHost {
    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_file(self.under_root(path))
    }

    fn group_members(&mut self, group: &str) -> io::Result<Option<Vec<String>>> {
        assert_eq!(group, GROUP);
        Ok(self.members.clone())
    }

    fn remove_member(&mut self, _: &str, user: &str) -> io::Result<()> {
        if let Some(members) = &mut self.members {
            members.retain(|m| m != user);
        }
        Ok(())
    }

    fn remove_group(&mut self, _: &str) -> io::Result<()> {
        self.members = None;
        Ok(())
    }

    fn say(&mut self, line: &str) {
        self.said.push(line.to_owned());
    }
}

const FILES: [&str; 3] = [
    "/etc/sysusers.d/opendeepcool.conf",
    "/etc/udev/rules.d/70-opendeepcool.rules",
    "/etc/udev/rules.d/71-opendeepcool-dev-reconnect.rules",
];

fn manifest() -> Manifest {
    let mut m = Manifest::default();
    for f in FILES {
        m.push(Entry::file(f).expect("a dev file")).expect("new");
    }
    m.push(Entry::Group).expect("new");
    m.push(Entry::member("dev_user").expect("a user"))
        .expect("new");
    m
}

#[test]
fn entries_go_last_first_and_a_second_run_finds_them_gone() {
    let mut host = TestHost::new("removal-order");
    for f in FILES {
        fs::write(host.under_root(Path::new(f)), "x").expect("a dev file");
    }
    let unrelated = host.root.join("etc/udev/rules.d/99-other.rules");
    fs::write(&unrelated, "x").expect("an unrelated file");

    let failures = manifest().remove_all(&mut host);
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(
        host.said,
        [
            "removed dev_user from opendeepcool",
            "removed the group opendeepcool",
            "removed /etc/udev/rules.d/71-opendeepcool-dev-reconnect.rules",
            "removed /etc/udev/rules.d/70-opendeepcool.rules",
            "removed /etc/sysusers.d/opendeepcool.conf",
        ]
    );
    for f in FILES {
        assert!(!host.under_root(Path::new(f)).exists(), "{f}");
    }
    assert!(
        unrelated.exists(),
        "nothing outside the manifest is touched"
    );

    host.said.clear();
    let failures = manifest().remove_all(&mut host);
    assert!(failures.is_empty(), "{failures:?}");
    assert!(
        host.said.iter().all(|l| l.ends_with("already gone")),
        "{:?}",
        host.said
    );
}

#[test]
fn a_folder_at_a_recorded_path_keeps_its_contents() {
    let mut host = TestHost::new("removal-folder");
    let folder = host.under_root(Path::new(FILES[1]));
    fs::create_dir(&folder).expect("a folder where a file was recorded");
    fs::write(folder.join("inside"), "x").expect("a file inside it");

    let failures = manifest().remove_all(&mut host);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].starts_with(FILES[1]), "{failures:?}");
    assert!(folder.join("inside").exists(), "never a folder's contents");
}
