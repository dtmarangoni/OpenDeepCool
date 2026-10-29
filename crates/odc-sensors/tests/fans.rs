//! The fan inputs the user picks for the cooler's fans and pump, over fake trees (Phase 6 plan,
//! Task 7; `docs/spec/sensors.md`, Fan and pump speed): every input but a graphics card's, found
//! again by chip, device and number on every read.

use std::path::PathBuf;

use odc_sensors::fake::FakeClock;
use odc_sensors::{
    Exact, Fan, Reading, Readings, Registry, Roots, Unavailable, fan_inputs, fan_rpm,
};
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{BOARD_CHIP, amdgpu_dgpu, board_fans, coretemp, nouveau};

fn tree(name: &str) -> FakeTree {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("fans")
        .join(name);
    FakeTree::create(root).expect("a fake tree")
}

fn sys(tree: &FakeTree) -> PathBuf {
    tree.root().join("sys")
}

fn rpm(n: i64) -> Reading {
    Reading::Value(Exact::whole(n))
}

/// A board chip with three inputs, one labelled, beside graphics cards with their own fans and a
/// CPU temperature driver without fans.
fn board_and_cards(t: &FakeTree) {
    amdgpu_dgpu(t, "0000:0a:00.0", 41_000, 37, 182_000_000).expect("amdgpu");
    nouveau(t, "0000:0e:00.0").expect("nouveau");
    coretemp(t, 57_000).expect("coretemp");
    board_fans(
        t,
        BOARD_CHIP,
        &[(1, 930, None), (2, 3_400, Some("Pump")), (3, 0, None)],
    )
    .expect("board");
}

#[test]
fn every_board_input_is_listed_and_no_graphics_card_fan_is() {
    let t = tree("list");
    board_and_cards(&t);
    let inputs = fan_inputs(&sys(&t));
    let shown: Vec<(String, String, u32, Option<String>)> = inputs
        .iter()
        .map(|f| {
            (
                f.chip.clone(),
                f.device.display().to_string(),
                f.input,
                f.label.clone(),
            )
        })
        .collect();
    let device = "devices/platform/fakeboardchip.656".to_owned();
    assert_eq!(
        shown,
        [
            (BOARD_CHIP.to_owned(), device.clone(), 1, None),
            (
                BOARD_CHIP.to_owned(),
                device.clone(),
                2,
                Some("Pump".to_owned())
            ),
            (BOARD_CHIP.to_owned(), device, 3, None),
        ]
    );
    let speeds: Vec<Reading> = inputs.iter().map(|f| fan_rpm(&sys(&t), f)).collect();
    assert_eq!(speeds, [rpm(930), rpm(3_400), rpm(0)]);
}

#[test]
fn an_input_is_found_again_when_its_chip_gets_another_hwmon_number() {
    let t = tree("renumbered");
    board_fans(&t, BOARD_CHIP, &[(1, 930, None), (2, 3_400, None)]).expect("board");
    let pump = fan_inputs(&sys(&t))
        .into_iter()
        .find(|f| f.input == 2)
        .expect("input 2");
    assert_eq!(fan_rpm(&sys(&t), &pump), rpm(3_400));

    // The chip comes back as hwmon1 after another device took hwmon0.
    t.remove("sys/class/hwmon/hwmon0").expect("the old link");
    t.remove("sys/devices/platform/fakeboardchip.656")
        .expect("the old device");
    coretemp(&t, 57_000).expect("coretemp");
    let again = board_fans(&t, BOARD_CHIP, &[(1, 940, None), (2, 3_350, None)]).expect("board");
    assert!(again.ends_with("hwmon1"), "{}", again.display());
    assert_eq!(fan_rpm(&sys(&t), &pump), rpm(3_350));
}

#[test]
fn a_chosen_input_that_is_gone_is_no_device_never_another_inputs_speed() {
    let t = tree("gone");
    board_fans(&t, BOARD_CHIP, &[(1, 930, None), (2, 3_400, None)]).expect("board");
    let first = board_fans(&t, "otherfakechip", &[(2, 1_100, None)]).expect("another chip");
    let inputs = fan_inputs(&sys(&t));
    let pump = inputs
        .iter()
        .find(|f| f.chip == BOARD_CHIP && f.input == 2)
        .expect("the board's input 2")
        .clone();
    // Each chip's input 2 reads its own speed (AGENTS.md rule 11).
    let other = inputs
        .iter()
        .find(|f| f.chip == "otherfakechip")
        .expect("the other chip's input");
    assert_eq!(fan_rpm(&sys(&t), other), rpm(1_100));
    assert_eq!(fan_rpm(&sys(&t), &pump), rpm(3_400));

    t.remove("sys/devices/platform/fakeboardchip.656/hwmon/hwmon0/fan2_input")
        .expect("the input");
    assert_eq!(
        fan_rpm(&sys(&t), &pump),
        Reading::Unavailable(Unavailable::NoDevice)
    );
    assert!(first.exists(), "the other chip is still there");
}

#[test]
fn the_snapshot_carries_the_chosen_fans_and_none_by_default() {
    let t = tree("snapshot");
    board_and_cards(&t);
    let inputs = fan_inputs(&sys(&t));
    let roots = Roots {
        sys: sys(&t),
        proc: t.root().join("proc"),
    };
    let mut registry = Registry::from_rules(FakeClock::new(), roots);
    let snapshot = registry.snapshot();
    assert_eq!(
        (snapshot.cooler_fans, snapshot.pump),
        (Fan::NotChosen, Fan::NotChosen)
    );
    registry.choose_fans(Some(inputs[0].clone()), Some(inputs[1].clone()));
    let snapshot = registry.snapshot();
    assert_eq!(
        (snapshot.cooler_fans, snapshot.pump),
        (Fan::Chosen(rpm(930)), Fan::Chosen(rpm(3_400)))
    );
}
