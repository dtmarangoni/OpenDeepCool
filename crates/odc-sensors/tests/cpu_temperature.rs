//! CPU Temperature from coretemp or k10temp, over fake sysfs trees (Phase 6 plan, Task 4;
//! `docs/spec/sensors.md`, Driver rules): the package's channel, found by the driver's name and
//! the channel's label, never by an index.

use std::path::PathBuf;
use std::time::Duration;

use odc_sensors::fake::FakeClock;
use odc_sensors::{Binding, Clock, Exact, Metric, Reading, Readings, Registry, Roots, Unavailable};
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{
    BOARD_CHIP, amdgpu_dgpu, board_fans, coretemp, hwmon, k10temp, k10temp_pre17h,
};

fn tree(name: &str) -> FakeTree {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("cpu_temperature")
        .join(name);
    FakeTree::create(root).expect("a fake tree")
}

fn roots(tree: &FakeTree) -> Roots {
    Roots {
        sys: tree.root().join("sys"),
        proc: tree.root().join("proc"),
    }
}

fn registry(tree: &FakeTree, clock: &FakeClock) -> Registry<FakeClock> {
    Registry::from_rules(clock.clone(), roots(tree))
}

/// CPU Temperature in one snapshot of a new registry.
fn cpu_temperature(tree: &FakeTree) -> Reading {
    let mut registry = registry(tree, &FakeClock::new());
    registry.snapshot().get(Metric::CpuTemperature).clone()
}

fn celsius(n: i64) -> Reading {
    Reading::Value(Exact::whole(n))
}

#[test]
fn coretemp_reads_the_package_57000_millidegrees_as_exactly_57() {
    let t = tree("coretemp");
    coretemp(&t, 57_000).expect("coretemp");
    let reading = cpu_temperature(&t);
    assert_eq!(reading, celsius(57));
    let Reading::Value(exact) = reading else {
        panic!("a value");
    };
    assert_eq!((exact.num(), exact.den()), (57_000, 1_000));
}

#[test]
fn k10temp_prefers_tdie_then_tctl_and_has_no_rule_before_family_17h() {
    let both = tree("k10temp-both");
    k10temp(&both, 61_000, Some(51_000)).expect("k10temp");
    assert_eq!(cpu_temperature(&both), celsius(51));

    let tctl = tree("k10temp-tctl");
    k10temp(&tctl, 61_000, None).expect("k10temp");
    assert_eq!(cpu_temperature(&tctl), celsius(61));

    let old = tree("k10temp-pre17h");
    k10temp_pre17h(&old).expect("k10temp");
    assert_eq!(
        cpu_temperature(&old),
        Reading::Unavailable(Unavailable::NoRule {
            driver: "k10temp".into()
        })
    );
}

#[test]
fn without_a_cpu_temperature_driver_the_metric_has_no_device() {
    let t = tree("none");
    board_fans(&t, BOARD_CHIP, &[(1, 930, None)]).expect("board");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 37, 182_000_000).expect("amdgpu");
    assert_eq!(
        cpu_temperature(&t),
        Reading::Unavailable(Unavailable::NoDevice)
    );
}

#[test]
fn a_board_chip_with_a_package_id_0_label_isnt_the_cpu() {
    let t = tree("foreign-label");
    hwmon(
        &t,
        "sys/devices/platform/fakeboardchip.656",
        BOARD_CHIP,
        &[
            ("temp1_label".into(), "Package id 0".into()),
            ("temp1_input".into(), "33000".into()),
        ],
    )
    .expect("a board chip");
    assert_eq!(
        cpu_temperature(&t),
        Reading::Unavailable(Unavailable::NoDevice)
    );
}

#[test]
fn an_hwmon_number_change_between_scans_still_finds_the_driver_by_name() {
    let t = tree("renumbered");
    board_fans(&t, "fakechipa", &[(1, 930, None)]).expect("a chip");
    board_fans(&t, "fakechipb", &[(1, 940, None)]).expect("a chip");
    let first = coretemp(&t, 57_000).expect("coretemp");
    assert!(first.ends_with("hwmon2"), "{}", first.display());
    let clock = FakeClock::new();
    let mut registry = registry(&t, &clock);
    assert_eq!(
        registry.snapshot().get(Metric::CpuTemperature),
        &celsius(57)
    );

    // The driver comes back as hwmon5, and other devices take numbers 2 to 4.
    t.remove("sys/class/hwmon/hwmon2").expect("the old link");
    t.remove("sys/devices/platform/coretemp.0")
        .expect("the old device");
    for chip in ["fakechipc", "fakechipd", "fakechipe"] {
        board_fans(&t, chip, &[(1, 950, None)]).expect("a chip");
    }
    let again = coretemp(&t, 58_000).expect("coretemp");
    assert!(again.ends_with("hwmon5"), "{}", again.display());

    clock.advance(Duration::from_secs(1));
    assert_eq!(
        registry.snapshot().get(Metric::CpuTemperature),
        &celsius(58)
    );
    let bindings = registry.bindings();
    let Binding::Bound(source) = &bindings[0].1 else {
        panic!("bound: {bindings:?}");
    };
    assert!(source.contains("hwmon5"), "{source}");
}

/// Takes a snapshot every second for `seconds` + 1 seconds, creating coretemp at `appears`
/// seconds, and returns CPU Temperature's readings.
fn appearing_at(name: &str, appears: u64, seconds: u64) -> Vec<Reading> {
    let t = tree(name);
    let clock = FakeClock::new();
    let t0 = clock.now();
    let mut registry = registry(&t, &clock);
    (0..=seconds)
        .map(|_| {
            if clock.now() == t0 + Duration::from_secs(appears) {
                coretemp(&t, 57_000).expect("coretemp");
            }
            let reading = registry.snapshot().get(Metric::CpuTemperature).clone();
            clock.advance(Duration::from_secs(1));
            reading
        })
        .collect()
}

#[test]
fn a_cpu_temperature_driver_that_appears_after_30_s_is_bound_at_the_next_5_s_scan() {
    for (t, reading) in appearing_at("late-30", 31, 40).iter().enumerate() {
        let expected = if t >= 35 {
            celsius(57)
        } else {
            Reading::Unavailable(Unavailable::NoDevice)
        };
        assert_eq!(*reading, expected, "at {t} s");
    }
}

#[test]
fn a_cpu_temperature_driver_that_appears_after_3_minutes_is_bound_at_the_next_60_s_scan() {
    for (t, reading) in appearing_at("late-180", 181, 250).iter().enumerate() {
        let expected = if t >= 240 {
            celsius(57)
        } else {
            Reading::Unavailable(Unavailable::NoDevice)
        };
        assert_eq!(*reading, expected, "at {t} s");
    }
}
