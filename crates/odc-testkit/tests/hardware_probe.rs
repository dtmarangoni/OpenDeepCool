//! The hardware probe against fake trees (Phase 5 plan, Task 9): absent, usable, and present but
//! unusable with the setup fix. "Unusable" uses a folder where the node should be: opening a
//! folder for writing fails for every account, root included, which CI runs as.

use std::fs;
use std::path::PathBuf;

use odc_testkit::fake_tree::{FakeTree, LP360_HID_ID};
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

#[test]
fn capability_names_are_what_require_takes() {
    assert_eq!(Capability::from_name("lp360"), Some(Capability::Lp360));
    assert_eq!(
        Capability::from_name("lp360-reconnect"),
        Some(Capability::Lp360Reconnect)
    );
    assert_eq!(Capability::from_name("lp36"), None);
}
