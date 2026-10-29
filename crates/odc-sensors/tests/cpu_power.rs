//! CPU Power from the RAPL package energy counters, over fake trees (Phase 6 plan, Task 6;
//! `docs/spec/sensors.md`, CPU power): every package zone's energy over the interval, whole
//! microjoules over whole microseconds, with the counters' wrap-around and wake-ups.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use odc_sensors::fake::FakeClock;
use odc_sensors::{Exact, Metric, Reading, Readings, Registry, Roots, Unavailable};
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{rapl, rapl_mmio};

const MAX: u64 = 262_143_328_850;
const ZONE0: &str = "sys/devices/virtual/powercap/intel-rapl/intel-rapl:0/energy_uj";
const ZONE1: &str = "sys/devices/virtual/powercap/intel-rapl/intel-rapl:1/energy_uj";
const CORE: &str = "sys/devices/virtual/powercap/intel-rapl/intel-rapl:0/intel-rapl:0:0/energy_uj";
const MMIO: &str = "sys/devices/virtual/powercap/intel-rapl-mmio/intel-rapl-mmio:0/energy_uj";

fn tree(name: &str) -> FakeTree {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("cpu_power")
        .join(name);
    FakeTree::create(root).expect("a fake tree")
}

fn registry(tree: &FakeTree, clock: &FakeClock) -> Registry<FakeClock> {
    let roots = Roots {
        sys: tree.root().join("sys"),
        proc: tree.root().join("proc"),
    };
    Registry::from_rules(clock.clone(), roots)
}

/// Sets each counter, then takes a snapshot one second after the last.
fn sample(
    tree: &FakeTree,
    registry: &mut Registry<FakeClock>,
    clock: &FakeClock,
    counters: &[(&str, u64)],
) -> Reading {
    for (path, energy) in counters {
        tree.file(path, format!("{energy}\n")).expect("a counter");
    }
    clock.advance(Duration::from_secs(1));
    registry.snapshot().get(Metric::CpuPower).clone()
}

fn watts(num: i64, den: u64) -> Reading {
    Reading::Value(Exact::new(num, den).expect("a fraction"))
}

const NOT_YET: Reading = Reading::Unavailable(Unavailable::NotYet);

#[test]
fn two_joules_in_one_second_are_exactly_two_watts() {
    let t = tree("one");
    rapl(&t, &[("intel-rapl:0", "package-0", 7_000_000, MAX)]).expect("rapl");
    let clock = FakeClock::new();
    let mut r = registry(&t, &clock);
    assert_eq!(sample(&t, &mut r, &clock, &[(ZONE0, 7_000_000)]), NOT_YET);
    let reading = sample(&t, &mut r, &clock, &[(ZONE0, 9_000_000)]);
    assert_eq!(reading, watts(2, 1));
    assert_eq!(reading, watts(2_000_000, 1_000_000));
}

#[test]
fn a_counter_that_wrapped_gives_the_energy_across_the_wrap() {
    let t = tree("wrap");
    rapl(&t, &[("intel-rapl:0", "package-0", 9_500_000, 10_000_000)]).expect("rapl");
    let clock = FakeClock::new();
    let mut r = registry(&t, &clock);
    assert_eq!(sample(&t, &mut r, &clock, &[(ZONE0, 9_500_000)]), NOT_YET);
    // 500 000 µJ up to the wrap at 10 000 000, then 1 500 000 after it.
    assert_eq!(
        sample(&t, &mut r, &clock, &[(ZONE0, 1_500_000)]),
        watts(2, 1)
    );
}

#[test]
fn two_packages_add_up_and_sub_zones_and_mmio_twins_dont_count() {
    let t = tree("two");
    rapl(
        &t,
        &[
            ("intel-rapl:0", "package-0", 1_000_000, MAX),
            ("intel-rapl:0:0", "core", 1_000_000, MAX),
            ("intel-rapl:1", "package-1", 2_000_000, MAX),
        ],
    )
    .expect("rapl");
    rapl_mmio(&t, 3_000_000, MAX).expect("mmio");
    let clock = FakeClock::new();
    let mut r = registry(&t, &clock);
    sample(&t, &mut r, &clock, &[]);
    let reading = sample(
        &t,
        &mut r,
        &clock,
        &[
            (ZONE0, 2_500_000),
            (CORE, 9_000_000),
            (ZONE1, 4_500_000),
            (MMIO, 30_000_000),
        ],
    );
    // 1.5 J and 2.5 J from the packages; the core sub-zone's and the MMIO zone's don't count.
    assert_eq!(reading, watts(4, 1));
    let bindings = r.bindings();
    let binding = format!("{:?}", bindings[Metric::CpuPower as usize].1);
    assert!(binding.contains("intel-rapl:0/energy_uj"), "{binding}");
    assert!(binding.contains("intel-rapl:1/energy_uj"), "{binding}");
    assert!(
        !binding.contains("mmio") && !binding.contains("intel-rapl:0:0"),
        "{binding}"
    );
}

#[test]
fn after_a_wake_up_a_counter_that_dropped_gives_not_yet_never_a_power_across_the_sleep() {
    let t = tree("sleep");
    rapl(&t, &[("intel-rapl:0", "package-0", 50_000_000, MAX)]).expect("rapl");
    let clock = FakeClock::new();
    let mut r = registry(&t, &clock);
    sample(&t, &mut r, &clock, &[(ZONE0, 50_000_000)]);
    assert_eq!(
        sample(&t, &mut r, &clock, &[(ZONE0, 52_000_000)]),
        watts(2, 1)
    );
    // The PC sleeps; its counter reads 0 when it wakes, and Phase 7 restarts the intervals.
    clock.advance(Duration::from_secs(3_600));
    r.restart_intervals();
    assert_eq!(sample(&t, &mut r, &clock, &[(ZONE0, 0)]), NOT_YET);
    assert_eq!(
        sample(&t, &mut r, &clock, &[(ZONE0, 3_000_000)]),
        watts(3, 1)
    );
}

#[test]
fn without_rapl_zones_cpu_power_has_no_device() {
    let t = tree("none");
    rapl_mmio(&t, 3_000_000, MAX).expect("mmio only");
    let mut r = registry(&t, &FakeClock::new());
    assert_eq!(
        r.snapshot().get(Metric::CpuPower),
        &Reading::Unavailable(Unavailable::NoDevice)
    );
}

/// The effective user ID, from `/proc/self/status` (proc(5)).
fn effective_uid() -> u32 {
    let status = fs::read_to_string("/proc/self/status").expect("/proc/self/status");
    status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .and_then(|ids| ids.split_whitespace().nth(1))
        .and_then(|id| id.parse().ok())
        .expect("a Uid: line")
}

#[test]
fn a_counter_this_account_may_not_read_is_permission_denied() {
    if effective_uid() == 0 {
        // File modes don't stop root, as in CI's container; the mapping of a refused read is
        // unit-tested in `sysfs.rs`.
        eprintln!("skipped as root: file modes don't stop root");
        return;
    }
    let t = tree("denied");
    rapl(&t, &[("intel-rapl:0", "package-0", 7_000_000, MAX)]).expect("rapl");
    t.mode(ZONE0, 0o000).expect("a mode");
    let mut r = registry(&t, &FakeClock::new());
    let reading = r.snapshot().get(Metric::CpuPower).clone();
    t.mode(ZONE0, 0o644).expect("a mode");
    let Reading::Unavailable(Unavailable::PermissionDenied { path }) = reading else {
        panic!("permission denied: {reading:?}");
    };
    assert!(
        path.ends_with("intel-rapl:0/energy_uj"),
        "{}",
        path.display()
    );
}
