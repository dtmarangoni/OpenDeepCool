//! `odc-e2e`'s flags, its start-up step and its fake mode, against fake trees, in `cargo test`
//! (Phase 5 plan, Task 9). The real cooler is never touched here.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use odc_e2e::device::restore_switch;
use odc_e2e::{Area, Options, parse_args, run};
use odc_testkit::fake_tree::{FakeTree, LP360_HID_ID};
use odc_testkit::hardware::Capability;
use odc_testkit::sysfs::{LP360_PRODUCT, LP360_VENDOR};

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("e2e")
        .join(name)
}

fn tree(name: &str) -> FakeTree {
    FakeTree::create(dir(name).join("tree")).expect("a fake tree")
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

const TICK: Duration = Duration::from_millis(1);

#[test]
fn flags_are_read_and_anything_unknown_is_refused() {
    assert_eq!(parse_args(&args(&[])), Ok(Some(Options::default())));
    assert_eq!(parse_args(&args(&["--help"])), Ok(None));
    assert_eq!(
        parse_args(&args(&[
            "--require",
            "lp360,lp360-reconnect",
            "--only",
            "device"
        ])),
        Ok(Some(Options {
            require: vec![Capability::Lp360, Capability::Lp360Reconnect],
            only: Some(Area::Device),
        }))
    );
    for (bad, reason) in [
        (&["--require", "lp361"][..], "isn't a capability"),
        (&["--only", "sensors"][..], "isn't an area"),
        (&["--fast"][..], "isn't an option"),
        (&["--require"][..], "needs a list"),
    ] {
        let error = parse_args(&args(bad)).expect_err(bad[0]);
        assert!(error.contains(reason), "{bad:?}: {error}");
    }
}

#[test]
fn with_no_cooler_every_check_runs_against_fakes_and_passes() {
    let t = tree("absent");
    let summary = run(&Options::default(), &t.roots(), &dir("absent"), TICK);
    let table = summary.table();
    assert!(summary.passed(), "{table}");
    assert!(table.contains("lp360            fake"), "{table}");
    assert!(table.contains("lp360-reconnect  fake"), "{table}");
    for check in ["test_images", "reconnect", "reset"] {
        assert!(table.contains(&format!("{check:<16} passed")), "{table}");
    }
}

#[test]
fn a_required_capability_that_is_absent_fails_the_run() {
    let t = tree("required");
    let options = Options {
        require: vec![Capability::Lp360],
        only: None,
    };
    let summary = run(&options, &t.roots(), &dir("required"), TICK);
    assert!(!summary.passed());
    assert!(
        summary.table().contains("FAILED  required real"),
        "{}",
        summary.table()
    );
}

#[test]
fn an_lp360_without_access_fails_with_the_setup_fix_and_no_fakes() {
    let t = tree("unusable");
    let node = t.hidraw(3, LP360_HID_ID).expect("a node");
    fs::remove_file(&node).expect("the stand-in");
    fs::create_dir(&node).expect("a folder in its place: no account can write it");
    let summary = run(&Options::default(), &t.roots(), &dir("unusable"), TICK);
    let table = summary.table();
    assert!(!summary.passed(), "{table}");
    assert!(table.contains("test_images      FAILED"), "{table}");
    assert!(table.contains("odc-dev -- setup"), "{table}");
}

#[test]
fn the_start_up_step_switches_a_cooler_left_off_back_on() {
    let t = tree("switch-off");
    let usb = t
        .usb_device("1-6", LP360_VENDOR, LP360_PRODUCT, "0\n")
        .expect("a USB device");
    let note = restore_switch(&t.roots()).expect("the switch is readable");
    assert!(note.is_some_and(|n| n.contains("switched it on")));
    assert_eq!(
        fs::read_to_string(usb.join("authorized")).expect("the switch"),
        "1"
    );
}

#[test]
fn the_start_up_step_leaves_a_cooler_that_is_on_alone() {
    let t = tree("switch-on");
    let usb = t
        .usb_device("1-6", LP360_VENDOR, LP360_PRODUCT, "1\n")
        .expect("a USB device");
    assert_eq!(restore_switch(&t.roots()), Ok(None));
    assert_eq!(
        fs::read_to_string(usb.join("authorized")).expect("the switch"),
        "1\n",
        "nothing written"
    );
}
