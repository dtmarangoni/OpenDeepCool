//! The ranking the helper writes, from fake sysfs trees and the fake kinds (Phase 6 plan, Task 9):
//! one dedicated GPU, integrated and dedicated together, a failed query and a driver without a
//! signal, each as the exact file the display service reads.

use std::fs;
use std::path::PathBuf;

use odc_gpu_rank::fake::FakeKinds;
use odc_gpu_rank::{Answer, Signal, rank, write_ranking};
use odc_sensors::{RANK_FILE, Ranking, review_ranking};
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{amdgpu_apu, amdgpu_dgpu, i915, nouveau, nvidia_proprietary, xe};

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("rank")
        .join(name)
}

fn tree(name: &str) -> FakeTree {
    FakeTree::create(dir(name).join("tree")).expect("a fake tree")
}

/// A fresh, empty state directory.
fn state(name: &str) -> PathBuf {
    let state = dir(name).join("state");
    let _ = fs::remove_dir_all(&state);
    fs::create_dir_all(&state).expect("a state directory");
    state
}

/// Ranks `t` with `kinds`, writes the file into a fresh state directory and returns its text.
fn ranked(name: &str, t: &FakeTree, kinds: &mut FakeKinds) -> String {
    let state = state(name);
    let path = write_ranking(&state, &rank(&t.roots().sysfs, kinds)).expect("written");
    assert_eq!(path, state.join(RANK_FILE));
    fs::read_to_string(path).expect("the file")
}

#[test]
fn one_dedicated_gpu() {
    let t = tree("one-dgpu");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    let mut kinds = FakeKinds::new(&[("0000:0a:00.0", Ok(Answer::Dedicated))]);
    assert_eq!(
        ranked("one-dgpu", &t, &mut kinds),
        r#"{
  "version": 1,
  "gpus": [
    {"pci": "0000:0a:00.0", "driver": "amdgpu", "kind": "dedicated", "signal": "AMDGPU_IDS_FLAGS_FUSION clear"}
  ]
}
"#
    );
    assert_eq!(
        kinds.asked(),
        [(
            Signal::AmdgpuFusion,
            "0000:0a:00.0".parse().expect("an address")
        )]
    );
}

#[test]
fn integrated_and_dedicated_gpus() {
    let t = tree("igpu-dgpu");
    i915(&t, "0000:00:02.0", 52_000, 7_000_000).expect("i915 graphics");
    xe(&t, "0000:0b:00.0", 63_000, 9_000_000).expect("an xe card");
    let mut kinds = FakeKinds::new(&[
        ("0000:00:02.0", Ok(Answer::Integrated)),
        ("0000:0b:00.0", Ok(Answer::Dedicated)),
    ]);
    assert_eq!(
        ranked("igpu-dgpu", &t, &mut kinds),
        r#"{
  "version": 1,
  "gpus": [
    {"pci": "0000:00:02.0", "driver": "i915", "kind": "integrated", "signal": "no I915_MEMORY_CLASS_DEVICE region"},
    {"pci": "0000:0b:00.0", "driver": "xe", "kind": "dedicated", "signal": "DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM set"}
  ]
}
"#
    );
}

#[test]
fn a_failed_query_is_recorded_with_its_error() {
    let t = tree("query-failed");
    amdgpu_apu(&t, "0000:0c:00.0", 45_000, 9, 11_000_000).expect("an amdgpu APU");
    let mut kinds = FakeKinds::new(&[(
        "0000:0c:00.0",
        Err("DRM_IOCTL_AMDGPU_INFO: Permission denied (os error 13)"),
    )]);
    assert_eq!(
        ranked("query-failed", &t, &mut kinds),
        r#"{
  "version": 1,
  "gpus": [
    {"pci": "0000:0c:00.0", "driver": "amdgpu", "kind": "unknown", "why": "query-failed", "error": "DRM_IOCTL_AMDGPU_INFO: Permission denied (os error 13)"}
  ]
}
"#
    );
}

#[test]
fn a_driver_without_a_signal_is_recorded_and_never_asked() {
    let t = tree("no-signal");
    nouveau(&t, "0000:0e:00.0").expect("a GPU on nouveau");
    let mut kinds = FakeKinds::new(&[]);
    assert_eq!(
        ranked("no-signal", &t, &mut kinds),
        r#"{
  "version": 1,
  "gpus": [
    {"pci": "0000:0e:00.0", "driver": "nouveau", "kind": "unknown", "why": "no-signal"}
  ]
}
"#
    );
    assert!(kinds.asked().is_empty(), "{:?}", kinds.asked());
}

#[test]
fn each_driver_is_asked_for_its_own_signal_and_each_answer_named() {
    let t = tree("signals");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    i915(&t, "0000:00:02.0", 52_000, 7_000_000).expect("i915 graphics");
    xe(&t, "0000:0b:00.0", 63_000, 9_000_000).expect("an xe card");
    nvidia_proprietary(&t, "0000:0d:00.0").expect("an NVIDIA GPU");
    let mut kinds = FakeKinds::new(&[
        ("0000:00:02.0", Ok(Answer::Dedicated)),
        ("0000:0a:00.0", Ok(Answer::Integrated)),
        ("0000:0b:00.0", Ok(Answer::Integrated)),
        ("0000:0d:00.0", Ok(Answer::Integrated)),
    ]);
    let ranking = rank(&t.roots().sysfs, &mut kinds);
    let asked: Vec<(Signal, String)> = kinds
        .asked()
        .iter()
        .map(|(signal, pci)| (*signal, pci.to_string()))
        .collect();
    assert_eq!(
        asked,
        [
            (Signal::I915DeviceMemory, "0000:00:02.0".to_owned()),
            (Signal::AmdgpuFusion, "0000:0a:00.0".to_owned()),
            (Signal::XeHasVram, "0000:0b:00.0".to_owned()),
            (Signal::CudaIntegrated, "0000:0d:00.0".to_owned()),
        ]
    );
    let text = ranking.render();
    for signal in [
        "an I915_MEMORY_CLASS_DEVICE region",
        "AMDGPU_IDS_FLAGS_FUSION set",
        "DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM clear",
        "CU_DEVICE_ATTRIBUTE_INTEGRATED 1",
    ] {
        assert!(text.contains(signal), "{signal}: {text}");
    }
    assert_eq!(
        Signal::CudaIntegrated.describe(Answer::Dedicated),
        "CU_DEVICE_ATTRIBUTE_INTEGRATED 0"
    );
}

#[test]
fn the_display_service_reads_the_file_back_and_keeps_it() {
    let t = tree("round-trip");
    i915(&t, "0000:00:02.0", 52_000, 7_000_000).expect("i915 graphics");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    nouveau(&t, "0000:0e:00.0").expect("a GPU on nouveau");
    let mut kinds = FakeKinds::new(&[
        ("0000:00:02.0", Ok(Answer::Integrated)),
        ("0000:0a:00.0", Ok(Answer::Dedicated)),
    ]);
    let ranking = rank(&t.roots().sysfs, &mut kinds);
    let state = state("round-trip");
    let path = write_ranking(&state, &ranking).expect("written");
    let review = review_ranking(&t.roots().sysfs, &path, false);
    assert_eq!(review.deleted, None);
    assert_eq!(review.ranking, Some(ranking));
}

#[test]
fn writing_replaces_a_leftover_and_leaves_only_the_file() {
    let t = tree("leftover");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    let state = state("leftover");
    fs::write(state.join(format!(".{RANK_FILE}.new")), "half a file").expect("a leftover");
    fs::write(state.join(RANK_FILE), "an older ranking").expect("an older file");
    let ranking = rank(
        &t.roots().sysfs,
        &mut FakeKinds::new(&[("0000:0a:00.0", Ok(Answer::Dedicated))]),
    );
    write_ranking(&state, &ranking).expect("written");
    let names: Vec<String> = fs::read_dir(&state)
        .expect("the state directory")
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    assert_eq!(names, [RANK_FILE]);
    let text = fs::read_to_string(state.join(RANK_FILE)).expect("the file");
    assert_eq!(Ranking::parse(&text), Ok(ranking));
}

#[test]
fn a_missing_state_directory_is_an_error() {
    let missing = dir("missing").join("state");
    let _ = fs::remove_dir_all(&missing);
    assert!(write_ranking(&missing, &Ranking::default()).is_err());
}
