//! Removal in a temporary folder (Phase 5 plan, Task 8; solution review P11-03): the last entry
//! first, one at a time, never a folder's contents, and a second run finds everything gone. From
//! Phase 7 also the dev copy of the service: its unit stopped and disabled first, its saved data,
//! its account, and the folders the setup created, each only when empty. From Phase 9 the sound
//! helper's user unit, disabled for every user before anything else goes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use odc_manifest::{Entry, GROUP, Host, Manifest, STATE_DIR, UNIT, USER, USER_UNIT};

/// Files under a temporary root; the group, the account and the unit in memory.
struct TestHost {
    root: PathBuf,
    members: Option<Vec<String>>,
    user: bool,
    /// Enabled, running.
    unit: (bool, bool),
    /// The user unit enabled for every user.
    user_unit: bool,
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
            user: true,
            unit: (true, true),
            user_unit: true,
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

    fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
        fs::remove_dir(self.under_root(path))
    }

    fn list_dir(&mut self, path: &Path) -> io::Result<Vec<String>> {
        fs::read_dir(self.under_root(path))?
            .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
            .collect()
    }

    fn user_exists(&mut self, user: &str) -> io::Result<bool> {
        assert_eq!(user, USER);
        Ok(self.user)
    }

    fn remove_user(&mut self, user: &str) -> io::Result<()> {
        assert_eq!(user, USER);
        assert!(
            !self.unit.1,
            "the account goes only once its service stopped"
        );
        self.user = false;
        Ok(())
    }

    fn disable(&mut self, unit: &str) -> io::Result<bool> {
        assert_eq!(unit, UNIT);
        let was = self.unit.0 || self.unit.1;
        self.unit = (false, false);
        Ok(was)
    }

    fn disable_for_every_user(&mut self, unit: &str) -> io::Result<bool> {
        assert_eq!(unit, USER_UNIT);
        let was = self.user_unit;
        self.user_unit = false;
        Ok(was)
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

/// The dev copy of the service, recorded after Phase 5's entries, as `odc-dev setup` records it.
const SERVICE_DIRS: [&str; 4] = [
    "/usr/local/libexec",
    "/usr/local/libexec/opendeepcool-dev",
    "/etc/systemd/system/opendeepcool.service.d",
    "/etc/systemd/user/opendeepcool-audio.service.d",
];
const SERVICE_FILES: [&str; 6] = [
    "/usr/local/libexec/opendeepcool-dev/opendeepcoold",
    "/usr/local/libexec/opendeepcool-dev/opendeepcool-audio",
    "/etc/systemd/system/opendeepcool.service",
    "/etc/systemd/system/opendeepcool.service.d/dev.conf",
    "/etc/systemd/user/opendeepcool-audio.service",
    "/etc/systemd/user/opendeepcool-audio.service.d/dev.conf",
];

fn service_manifest() -> Manifest {
    let mut m = manifest();
    for d in SERVICE_DIRS {
        m.push(Entry::dir(d).expect("a created folder"))
            .expect("new");
    }
    for f in SERVICE_FILES {
        m.push(Entry::file(f).expect("a dev file")).expect("new");
    }
    for entry in [
        Entry::User,
        Entry::State,
        Entry::Enabled,
        Entry::EnabledForEveryUser,
    ] {
        m.push(entry).expect("new");
    }
    m
}

/// The dev copy in place under the host's root, with the service's saved data.
fn install_service(host: &TestHost) {
    for d in SERVICE_DIRS {
        fs::create_dir_all(host.under_root(Path::new(d))).expect("a folder");
    }
    fs::create_dir_all(host.root.join("etc/systemd/system")).expect("a folder");
    fs::create_dir_all(host.root.join("etc/systemd/user")).expect("a folder");
    for f in SERVICE_FILES {
        fs::write(host.under_root(Path::new(f)), "x").expect("a dev file");
    }
    for f in FILES {
        fs::write(host.under_root(Path::new(f)), "x").expect("a dev file");
    }
    let state = host.under_root(Path::new(STATE_DIR));
    fs::create_dir_all(&state).expect("the state folder");
    for name in [
        "settings.toml",
        "gpu-rank.json",
        "dev-frames.log",
        "dev-frames.log.1",
    ] {
        fs::write(state.join(name), "x").expect("saved data");
    }
}

#[test]
fn the_service_stops_before_its_data_its_account_and_its_files_go() {
    let mut host = TestHost::new("removal-service");
    install_service(&host);
    let failures = service_manifest().remove_all(&mut host);
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(
        host.said[..10],
        [
            "disabled opendeepcool-audio.service for every user",
            "stopped and disabled opendeepcool.service",
            "removed /var/lib/opendeepcool/dev-frames.log",
            "removed /var/lib/opendeepcool/dev-frames.log.1",
            "removed /var/lib/opendeepcool/gpu-rank.json",
            "removed /var/lib/opendeepcool/settings.toml",
            "removed /var/lib/opendeepcool",
            "removed the user opendeepcool",
            "removed /etc/systemd/user/opendeepcool-audio.service.d/dev.conf",
            "removed /etc/systemd/user/opendeepcool-audio.service",
        ][..],
        "{:?}",
        host.said
    );
    assert!(!host.user);
    assert_eq!(host.unit, (false, false));
    assert!(!host.user_unit);
    for d in SERVICE_DIRS {
        assert!(!host.under_root(Path::new(d)).exists(), "{d}");
    }
    // A second run finds everything gone.
    host.said.clear();
    let failures = service_manifest().remove_all(&mut host);
    assert!(failures.is_empty(), "{failures:?}");
    assert!(
        host.said.iter().all(|l| l.ends_with("already gone")),
        "{:?}",
        host.said
    );
}

#[test]
fn a_shared_folder_that_isnt_empty_and_files_the_service_doesnt_write_are_kept() {
    let mut host = TestHost::new("removal-kept");
    install_service(&host);
    // Something else put a file in /usr/local/libexec, and in the state folder.
    let other = host.root.join("usr/local/libexec/other-tool");
    fs::write(&other, "x").expect("another program's file");
    let notes = host.under_root(Path::new(STATE_DIR)).join("notes.txt");
    fs::write(&notes, "x").expect("a file the service doesn't write");
    let failures = service_manifest().remove_all(&mut host);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("notes.txt"), "{failures:?}");
    assert!(notes.exists() && other.exists(), "never another's file");
    assert!(
        host.said
            .contains(&"kept /usr/local/libexec: not empty".to_owned()),
        "{:?}",
        host.said
    );
    assert!(
        !host
            .under_root(Path::new(STATE_DIR))
            .join("settings.toml")
            .exists(),
        "the service's own files go"
    );
}
