//! The real kinds against fake trees (Phase 6 plan, Task 9): before any request, the helper finds
//! the GPU's render node through sysfs and opens it only if it's the character device sysfs names.
//! A fake tree's stand-ins are regular files, so no request is ever sent here; each GPU ends up
//! unknown, with why. GPUs on NVIDIA's driver are left out: asking them loads NVIDIA's library.

use std::path::PathBuf;

use odc_gpu_rank::{DriverKinds, rank};
use odc_sensors::RankedKind;
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{amdgpu_dgpu, i915, nouveau, render_node, xe};

fn tree(name: &str) -> FakeTree {
    FakeTree::create(
        PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("drivers")
            .join(name),
    )
    .expect("a fake tree")
}

/// Each GPU's error, by address, in the ranking of `t`.
fn errors(t: &FakeTree) -> Vec<(String, String)> {
    let roots = t.roots();
    rank(
        &roots.sysfs,
        &mut DriverKinds::new(&roots.sysfs, &roots.dev),
    )
    .gpus
    .into_iter()
    .map(|gpu| {
        let why = match gpu.kind {
            RankedKind::QueryFailed { error } => error,
            other => format!("not a failed query: {other:?}"),
        };
        (gpu.pci.to_string(), why)
    })
    .collect()
}

#[test]
fn a_gpu_without_a_render_node_is_unknown_with_why() {
    let t = tree("no-node");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 17, 23_000_000).expect("an amdgpu card");
    let errors = errors(&t);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].1.ends_with("drm: no render node"), "{errors:?}");
}

#[test]
fn a_node_that_isnt_the_device_sysfs_names_is_never_used() {
    let t = tree("not-the-device");
    i915(&t, "0000:00:02.0", 52_000, 7_000_000).expect("i915 graphics");
    render_node(&t, "0000:00:02.0", 128).expect("a regular file as its node");
    xe(&t, "0000:0b:00.0", 63_000, 9_000_000).expect("an xe card");
    let missing = render_node(&t, "0000:0b:00.0", 129).expect("its node");
    std::fs::remove_file(&missing).expect("no node in dev/dri");
    let errors = errors(&t);
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert_eq!(errors[0].0, "0000:00:02.0");
    assert!(
        errors[0]
            .1
            .ends_with("dri/renderD128 isn't the device 226:128 that sysfs names"),
        "{errors:?}"
    );
    assert_eq!(errors[1].0, "0000:0b:00.0");
    assert!(
        errors[1].1.contains("dri/renderD129: No such file"),
        "{errors:?}"
    );
}

#[test]
fn a_driver_without_a_signal_is_never_opened() {
    let t = tree("no-signal");
    nouveau(&t, "0000:0e:00.0").expect("a GPU on nouveau");
    let roots = t.roots();
    let ranking = rank(
        &roots.sysfs,
        &mut DriverKinds::new(&roots.sysfs, &roots.dev),
    );
    assert_eq!(ranking.gpus.len(), 1);
    assert_eq!(ranking.gpus[0].kind, RankedKind::NoSignal);
}
