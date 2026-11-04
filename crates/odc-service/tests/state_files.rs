//! The dev teardown removes the service's saved data file by file, and keeps the folder when it
//! finds a file it doesn't know (`odc_manifest::is_state_file`; Phase 7 plan, Task 8). Every name
//! the service and its GPU rank helper write into the state folder is one it knows, the
//! temporary files too: the writers themselves make each one here.

use std::fs;
use std::path::{Path, PathBuf};

use odc_device::fake::{FakeConnector, Presence};
use odc_device::{DisplayPort, Lp360, Recording};
use odc_protocol::Frame;
use odc_sensors::{RANK_FILE, Ranking};
use odc_service::settings::{FILE, Settings, SettingsStore};
use odc_service::{RECORDING, STATE_DIR};

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch folder");
    dir
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("the folder")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .collect();
    names.sort();
    names
}

#[test]
fn every_file_the_service_and_its_helper_write_is_one_the_teardown_removes() {
    assert_eq!(STATE_DIR, odc_manifest::STATE_DIR);

    // What stays: the settings and a refused copy, the ranking, and the recording rotated once.
    let dir = scratch("state-files");
    let store = SettingsStore::new(&dir);
    store.load().expect("the defaults, saved");
    fs::write(store.path(), "not toml [").expect("a broken settings file");
    store.load().expect("the broken file moved aside");
    odc_gpu_rank::write_ranking(&dir, &Ranking::default()).expect("a ranking");
    let recording = Recording::dev(
        FakeConnector::new([Presence::Present]),
        dir.join(RECORDING),
        1,
    )
    .expect("a recording");
    let handle = recording.handle();
    let mut lp360 = Lp360::new(recording);
    lp360.show(&Frame::blank());
    handle.flush();

    // What a stop between a write and its rename leaves: here the rename fails, onto a folder.
    let temps = scratch("state-temps");
    fs::create_dir_all(temps.join(FILE).join("x")).expect("a folder in the way");
    fs::create_dir_all(temps.join(RANK_FILE).join("x")).expect("a folder in the way");
    SettingsStore::new(&temps)
        .store(&Settings::default())
        .expect_err("the rename fails");
    odc_gpu_rank::write_ranking(&temps, &Ranking::default()).expect_err("the rename fails");

    let written = names(&dir);
    let left = names(&temps);
    for (expected, among) in [
        ("settings.toml", &written),
        ("settings.toml.invalid-", &written),
        ("gpu-rank.json", &written),
        ("dev-frames.log", &written),
        ("dev-frames.log.1", &written),
        ("settings.toml.new", &left),
        (".gpu-rank.json.new", &left),
    ] {
        assert!(
            among.iter().any(|name| name.starts_with(expected)),
            "{expected} in {among:?}"
        );
    }
    for name in written.iter().chain(&left) {
        assert!(odc_manifest::is_state_file(name), "{name}");
    }
}
