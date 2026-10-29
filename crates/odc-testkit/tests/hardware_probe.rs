//! The hardware probe against fake trees (Phase 5 plan, Task 9; Phase 6 plan, Task 9): absent,
//! usable, and present but unusable with the fix. "Unusable" uses a folder where the node should
//! be: opening a folder for writing fails for every account, root included, which CI runs as.

use std::fs;
use std::path::PathBuf;

use odc_testkit::fake_tree::{FakeTree, LP360_HID_ID};
use odc_testkit::fixtures::{
    amdgpu_apu, amdgpu_dgpu, i915, nouveau, nvidia_proprietary, render_node, xe,
};
use odc_testkit::hardware::{Capability, Probe, probe};
use odc_testkit::sysfs::{LP360_PRODUCT, LP360_VENDOR};

fn tree(name: &str) -> FakeTree {
    FakeTree::create(
        PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("probe")
            .join(name),
    )
    .expect("a fake tree")
}

#[test]
fn no_lp360_is_absent() {
    let t = tree("empty");
    assert!(matches!(
        probe(Capability::Lp360, &t.roots()),
        Probe::Absent { .. }
    ));
    assert!(matches!(
        probe(Capability::Lp360Reconnect, &t.roots()),
        Probe::Absent { .. }
    ));
}

#[test]
fn a_writable_lp360_is_real() {
    let t = tree("real");
    t.hidraw(3, LP360_HID_ID).expect("a node");
    t.usb_device("1-6", LP360_VENDOR, LP360_PRODUCT, "1\n")
        .expect("a USB device");
    assert_eq!(probe(Capability::Lp360, &t.roots()), Probe::Real);
    assert_eq!(probe(Capability::Lp360Reconnect, &t.roots()), Probe::Real);
}

#[test]
fn an_lp360_without_access_is_unusable_with_the_setup_fix() {
    let t = tree("unusable");
    let node = t.hidraw(3, LP360_HID_ID).expect("a node");
    fs::remove_file(&node).expect("the stand-in");
    fs::create_dir(&node).expect("a folder in its place");
    let usb = t
        .usb_device("1-6", LP360_VENDOR, LP360_PRODUCT, "1\n")
        .expect("a USB device");
    fs::remove_file(usb.join("authorized")).expect("the switch");
    fs::create_dir(usb.join("authorized")).expect("a folder in its place");
    for capability in [Capability::Lp360, Capability::Lp360Reconnect] {
        match probe(capability, &t.roots()) {
            Probe::Unusable { fix, .. } => assert!(fix.contains("odc-dev -- setup"), "{fix}"),
            other => panic!("{capability:?}: {other:?}"),
        }
    }
}

const GPUS: [Capability; 5] = [
    Capability::GpuAmdgpu,
    Capability::GpuI915,
    Capability::GpuXe,
    Capability::GpuNvidia,
    Capability::GpuOther,
];

#[test]
fn no_gpu_is_absent_for_every_gpu_capability() {
    let t = tree("no-gpu");
    for capability in GPUS {
        assert!(
            matches!(probe(capability, &t.roots()), Probe::Absent { .. }),
            "{capability:?}"
        );
    }
}

#[test]
fn gpus_whose_render_nodes_open_are_real_each_under_its_own_driver() {
    let t = tree("gpus");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    render_node(&t, "0000:0a:00.0", 128).expect("its render node");
    i915(&t, "0000:00:02.0", 52_000, 7_000_000).expect("i915 graphics");
    render_node(&t, "0000:00:02.0", 129).expect("its render node");
    xe(&t, "0000:0b:00.0", 63_000, 9_000_000).expect("an xe card");
    render_node(&t, "0000:0b:00.0", 130).expect("its render node");
    nvidia_proprietary(&t, "0000:0d:00.0").expect("an NVIDIA GPU");
    nouveau(&t, "0000:0e:00.0").expect("a GPU on nouveau");
    for capability in GPUS {
        assert_eq!(probe(capability, &t.roots()), Probe::Real, "{capability:?}");
    }
}

#[test]
fn a_gpu_whose_render_node_wont_open_or_is_missing_is_unusable_with_the_fix() {
    let t = tree("render-unusable");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    let node = render_node(&t, "0000:0a:00.0", 128).expect("its render node");
    fs::remove_file(&node).expect("the stand-in");
    fs::create_dir(&node).expect("a folder in its place");
    xe(&t, "0000:0b:00.0", 63_000, 9_000_000).expect("an xe card without a render node");
    for capability in [Capability::GpuAmdgpu, Capability::GpuXe] {
        match probe(capability, &t.roots()) {
            Probe::Unusable { fix, .. } => assert!(fix.contains("render node"), "{fix}"),
            other => panic!("{capability:?}: {other:?}"),
        }
    }
}

#[test]
fn every_gpu_of_a_driver_must_be_usable() {
    let t = tree("one-of-two");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    render_node(&t, "0000:0a:00.0", 128).expect("its render node");
    amdgpu_apu(&t, "0000:0c:00.0", 45_000, 9, 11_000_000).expect("an amdgpu APU, no node");
    assert!(matches!(
        probe(Capability::GpuAmdgpu, &t.roots()),
        Probe::Unusable { .. }
    ));
}

#[test]
fn capability_names_are_what_require_takes() {
    assert_eq!(Capability::from_name("lp360"), Some(Capability::Lp360));
    assert_eq!(
        Capability::from_name("lp360-reconnect"),
        Some(Capability::Lp360Reconnect)
    );
    for (name, capability) in [
        ("gpu-amdgpu", Capability::GpuAmdgpu),
        ("gpu-i915", Capability::GpuI915),
        ("gpu-xe", Capability::GpuXe),
        ("gpu-nvidia", Capability::GpuNvidia),
        ("gpu-other", Capability::GpuOther),
    ] {
        assert_eq!(Capability::from_name(name), Some(capability));
        assert_eq!(capability.name(), name);
    }
    assert_eq!(Capability::from_name("lp36"), None);
    assert_eq!(Capability::from_name("gpu"), None);
}
