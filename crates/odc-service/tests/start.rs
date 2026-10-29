//! The GPUs at start-up (Phase 7 plan, Task 6): the service passes whether a GPU is chosen to
//! Phase 6's `review_ranking` and logs the line it returns when it deletes the ranking; the
//! deletion rules themselves are Phase 6's tests. Fake trees with made-up addresses.

use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use odc_sensors::{GpuKind, PciAddr, RANK_FILE, Ranked, RankedKind, Ranking, Unknown};
use odc_service::settings::Settings;
use odc_service::status::gpus_at_start;
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::amdgpu_dgpu;

fn setup(name: &str, ranked: &str) -> (FakeTree, PathBuf) {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("start")
        .join(name);
    let _ = fs::remove_dir_all(&root);
    let tree = FakeTree::create(root.join("tree")).expect("a fake tree");
    amdgpu_dgpu(&tree, "0000:0a:00.0", 40_000, 5, 20_000_000).expect("a GPU");
    let state = root.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    let ranking = Ranking {
        gpus: vec![Ranked {
            pci: PciAddr::from_str(ranked).expect("an address"),
            driver: "amdgpu".to_owned(),
            kind: RankedKind::Dedicated {
                signal: "AMDGPU_IDS_FLAGS_FUSION clear".to_owned(),
            },
        }],
    };
    fs::write(state.join(RANK_FILE), ranking.render()).expect("a ranking");
    (tree, state)
}

#[test]
fn a_ranking_that_lists_the_gpus_gives_their_kinds() {
    let (tree, state) = setup("current", "0000:0a:00.0");
    let found = gpus_at_start(&tree.roots().sysfs, &state, &Settings::default());
    assert_eq!(found.deleted, None);
    assert_eq!(found.gpus.len(), 1);
    assert_eq!(found.gpus[0].kind, GpuKind::Dedicated);
}

#[test]
fn a_stale_ranking_is_deleted_only_while_no_gpu_is_chosen() {
    let (tree, state) = setup("stale", "0000:0b:00.0");
    let chosen = Settings {
        gpu_choice: Some("0000:0a:00.0".to_owned()),
        ..Settings::default()
    };
    let kept = gpus_at_start(&tree.roots().sysfs, &state, &chosen);
    assert_eq!(kept.deleted, None, "the user chose: kept");
    assert!(state.join(RANK_FILE).exists());
    let gone = gpus_at_start(&tree.roots().sysfs, &state, &Settings::default());
    let why = gone
        .deleted
        .expect("deleted, with the reason for the journal");
    assert!(why.contains("GPUs changed"), "{why}");
    assert!(!state.join(RANK_FILE).exists());
    assert_eq!(gone.gpus[0].kind, GpuKind::Unknown(Unknown::NotRanked));
}
