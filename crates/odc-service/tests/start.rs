//! The GPUs at start-up (Phase 7 plan, Task 6; owner, 2026-11-04): a GPU added or removed since
//! the ranking was written, or a chosen GPU that's gone, clears the user's choice, which is saved
//! before the stale ranking is deleted; Phase 6's tests cover reading and deleting the ranking.
//! Fake trees with made-up addresses.

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use odc_sensors::{GpuKind, PciAddr, RANK_FILE, Ranked, RankedKind, Ranking, Unknown};
use odc_service::log::Priority;
use odc_service::settings::{Settings, SettingsStore};
use odc_service::status::gpus_at_start;
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::amdgpu_dgpu;

const A: &str = "0000:0a:00.0";
const B: &str = "0000:0b:00.0";

/// A tree with an amdgpu card at each of `present`, and a state directory whose ranking lists
/// `ranked` as dedicated, or no ranking when `ranked` is `None`.
fn setup(name: &str, present: &[&str], ranked: Option<&[&str]>) -> (FakeTree, PathBuf) {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("start")
        .join(name);
    let _ = fs::remove_dir_all(&root);
    let tree = FakeTree::create(root.join("tree")).expect("a fake tree");
    for pci in present {
        amdgpu_dgpu(&tree, pci, 40_000, 5, 20_000_000).expect("a GPU");
    }
    let state = root.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    if let Some(ranked) = ranked {
        let ranking = Ranking {
            gpus: ranked
                .iter()
                .map(|pci| Ranked {
                    pci: PciAddr::from_str(pci).expect("an address"),
                    driver: "amdgpu".to_owned(),
                    kind: RankedKind::Dedicated {
                        signal: "AMDGPU_IDS_FLAGS_FUSION clear".to_owned(),
                    },
                })
                .collect(),
        };
        fs::write(state.join(RANK_FILE), ranking.render()).expect("a ranking");
    }
    (tree, state)
}

fn chosen(pci: &str) -> Settings {
    Settings {
        mode: 4,
        rotation: 2,
        gpu_choice: Some(pci.to_owned()),
        ..Settings::default()
    }
}

#[test]
fn a_ranking_that_lists_the_gpus_gives_their_kinds() {
    let (tree, state) = setup("current", &[A], Some(&[A]));
    let mut settings = Settings::default();
    let found = gpus_at_start(
        &tree.roots().sysfs,
        &state,
        &SettingsStore::new(&state),
        &mut settings,
    );
    assert_eq!(found.lines, []);
    assert_eq!(found.gpus.len(), 1);
    assert_eq!(found.gpus[0].kind, GpuKind::Dedicated);
}

#[test]
fn a_chosen_gpu_stays_while_the_gpus_are_the_same() {
    let (tree, state) = setup("same", &[A, B], Some(&[A, B]));
    let mut settings = chosen(B);
    let found = gpus_at_start(
        &tree.roots().sysfs,
        &state,
        &SettingsStore::new(&state),
        &mut settings,
    );
    assert_eq!(found.lines, []);
    assert_eq!(settings, chosen(B));
    assert!(state.join(RANK_FILE).exists());
    assert!(!state.join("settings.toml").exists(), "nothing saved");
}

#[test]
fn a_gpu_added_or_removed_clears_the_choice_then_deletes_the_ranking() {
    let cases: [(&str, &[&str], &[&str], &str); 3] = [
        ("added", &[A, B], &[A], A),
        ("another removed", &[A], &[A, B], A),
        ("the chosen removed", &[A], &[A, B], B),
    ];
    for (name, present, ranked, pci) in cases {
        let (tree, state) = setup(
            &format!("changed-{}", name.replace(' ', "-")),
            present,
            Some(ranked),
        );
        let store = SettingsStore::new(&state);
        let mut settings = chosen(pci);
        let found = gpus_at_start(&tree.roots().sysfs, &state, &store, &mut settings);
        assert_eq!(
            settings,
            Settings {
                gpu_choice: None,
                ..chosen(pci)
            },
            "{name}: only the choice is cleared"
        );
        assert_eq!(store.load().expect("saved").0, settings, "{name}: saved");
        assert!(!state.join(RANK_FILE).exists(), "{name}: ranking deleted");
        let [(first, cleared), (second, deleted)] = found.lines.as_slice() else {
            panic!("{name}: {:?}", found.lines);
        };
        assert_eq!(
            (*first, *second),
            (Priority::Notice, Priority::Notice),
            "{name}"
        );
        assert!(
            cleared.contains(&format!("the choice of {pci} is cleared")),
            "{name}: {cleared}"
        );
        assert!(
            deleted.contains("the GPUs changed since it was written"),
            "{name}: {deleted}"
        );
    }
}

#[test]
fn a_change_while_no_gpu_is_chosen_only_deletes_the_ranking() {
    let (tree, state) = setup("unchosen", &[A, B], Some(&[A]));
    let mut settings = Settings::default();
    let found = gpus_at_start(
        &tree.roots().sysfs,
        &state,
        &SettingsStore::new(&state),
        &mut settings,
    );
    assert_eq!(found.lines.len(), 1, "{:?}", found.lines);
    assert!(found.lines[0].1.contains("the GPUs changed"));
    assert!(!state.join(RANK_FILE).exists());
    assert!(!state.join("settings.toml").exists(), "nothing saved");
    // The stale ranking still gives the kinds it lists; the new card is unknown until the next.
    let new = found
        .gpus
        .iter()
        .find(|g| g.pci.to_string() == B)
        .expect("B");
    assert_eq!(new.kind, GpuKind::Unknown(Unknown::NotRanked));
}

#[test]
fn a_chosen_gpu_that_is_gone_is_cleared_without_a_ranking_too() {
    let (tree, state) = setup("no-ranking", &[A], None);
    let store = SettingsStore::new(&state);
    let mut settings = chosen(B);
    let found = gpus_at_start(&tree.roots().sysfs, &state, &store, &mut settings);
    assert_eq!(settings.gpu_choice, None);
    assert_eq!(store.load().expect("saved").0.gpu_choice, None);
    assert_eq!(found.lines.len(), 1, "{:?}", found.lines);
    assert!(
        found.lines[0]
            .1
            .contains(&format!("the choice of {B} is cleared"))
    );
}

// A plain file where the settings' directory should be fails every write, even root's.
#[test]
fn a_cleared_choice_that_cant_be_saved_holds_for_this_run_and_keeps_the_ranking() {
    let (tree, state) = setup("unsaved", &[A, B], Some(&[A]));
    let blocked = state.with_file_name("blocked");
    fs::write(&blocked, "").expect("a plain file");
    let mut settings = chosen(A);
    let found = gpus_at_start(
        &tree.roots().sysfs,
        &state,
        &SettingsStore::new(&blocked),
        &mut settings,
    );
    assert_eq!(settings.gpu_choice, None, "cleared for this run");
    assert!(
        state.join(RANK_FILE).exists(),
        "kept, so the next start tries again"
    );
    let [(priority, line)] = found.lines.as_slice() else {
        panic!("{:?}", found.lines);
    };
    assert_eq!(*priority, Priority::Error);
    assert!(line.contains("the settings can't be saved"), "{line}");
}
