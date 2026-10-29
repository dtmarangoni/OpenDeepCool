//! CPU Usage and Memory Load from procfs, over fake trees (Phase 6 plan, Task 5;
//! `docs/spec/sensors.md`, Driver rules): exact busy shares between two samples, iowait counted
//! as idle, and the interval started again after a wake-up.

use std::path::PathBuf;
use std::time::Duration;

use odc_sensors::fake::FakeClock;
use odc_sensors::{Exact, Metric, Reading, Readings, Registry, Roots, Unavailable};
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{proc_meminfo, proc_stat};
use proptest::prelude::*;

fn tree(name: &str) -> FakeTree {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("procfs")
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

/// A `/proc/stat` whose aggregate line has `fields` (user, nice, system, idle, iowait, irq,
/// softirq, steal, guest, guest_nice), followed by a per-CPU line and other lines that nothing
/// reads.
fn stat(fields: [u64; 10]) -> String {
    let line: Vec<String> = fields.iter().map(u64::to_string).collect();
    format!(
        "cpu  {}\ncpu0 1 1 1 1 1 1 1 1 1 1\nintr 12345 0 0\nctxt 999\n",
        line.join(" ")
    )
}

/// CPU Usage for each `/proc/stat` in turn, a second apart.
fn usage(name: &str, samples: &[[u64; 10]]) -> Vec<Reading> {
    let t = tree(name);
    let clock = FakeClock::new();
    proc_stat(&t, &stat(samples[0])).expect("stat");
    let mut registry = registry(&t, &clock);
    samples
        .iter()
        .map(|sample| {
            proc_stat(&t, &stat(*sample)).expect("stat");
            let reading = registry.snapshot().get(Metric::CpuUsage).clone();
            clock.advance(Duration::from_secs(1));
            reading
        })
        .collect()
}

fn percent(num: i64, den: u64) -> Reading {
    Reading::Value(Exact::new(num, den).expect("a fraction"))
}

const NOT_YET: Reading = Reading::Unavailable(Unavailable::NotYet);

const START: [u64; 10] = [1000, 10, 300, 8000, 200, 20, 30, 5, 400, 0];

/// `START` plus `delta`, field by field.
fn plus(delta: [u64; 10]) -> [u64; 10] {
    let mut sample = START;
    for (field, d) in sample.iter_mut().zip(delta) {
        *field += d;
    }
    sample
}

#[test]
fn the_busy_share_of_the_interval_is_exact_and_iowait_counts_as_idle() {
    // Fields: user, nice, system, idle, iowait, irq, softirq, steal, guest, guest_nice.
    let cases = [
        ("idle", [0, 0, 0, 100, 0, 0, 0, 0, 0, 0], percent(0, 1)),
        ("half", [50, 0, 0, 50, 0, 0, 0, 0, 0, 0], percent(50, 1)),
        ("full", [70, 0, 30, 0, 0, 0, 0, 0, 0, 0], percent(100, 1)),
        ("iowait", [40, 0, 0, 40, 20, 0, 0, 0, 0, 0], percent(40, 1)),
        // Guest time is already inside user: adding it again would give more than 100 %.
        ("guest", [60, 0, 0, 40, 0, 0, 0, 0, 60, 0], percent(60, 1)),
        (
            "irq and steal",
            [10, 5, 5, 60, 0, 10, 5, 5, 0, 0],
            percent(40, 1),
        ),
    ];
    for (name, delta, expected) in cases {
        assert_eq!(
            usage(name, &[START, plus(delta)]),
            [NOT_YET, expected],
            "{name}"
        );
    }
}

#[test]
fn twenty_three_busy_jiffies_of_forty_are_exactly_57_5_percent() {
    let readings = usage("57.5", &[START, plus([23, 0, 0, 17, 0, 0, 0, 0, 0, 0])]);
    assert_eq!(readings[1], percent(115, 2));
    let Reading::Value(exact) = readings[1] else {
        panic!("a value");
    };
    // Rounded to the nearest whole number in integers, halves up: 58.
    let den = i64::try_from(exact.den()).expect("a small denominator");
    assert_eq!((2 * exact.num() + den) / (2 * den), 58);
    // The same share in floating point falls just short of the half and would round to 57.
    assert_eq!(23.0_f64 / 40.0 * 100.0, 57.49999999999999);
}

#[test]
fn an_interval_without_jiffies_keeps_the_last_value() {
    let busy = plus([30, 0, 0, 70, 0, 0, 0, 0, 0, 0]);
    let readings = usage("no-jiffies", &[START, busy, busy]);
    assert_eq!(readings, [NOT_YET, percent(30, 1), percent(30, 1)]);
}

#[test]
fn after_restart_intervals_the_next_reading_is_not_yet_and_then_uses_only_the_new_interval() {
    let t = tree("restart");
    let clock = FakeClock::new();
    let a = START;
    let b = plus([50, 0, 0, 50, 0, 0, 0, 0, 0, 0]);
    // During the sleep the counters jump; the reading after the wake-up must not span it.
    let c = plus([5_000, 0, 0, 100, 0, 0, 0, 0, 0, 0]);
    let d = plus([5_010, 0, 0, 190, 0, 0, 0, 0, 0, 0]);
    proc_stat(&t, &stat(a)).expect("stat");
    let mut registry = registry(&t, &clock);
    let take = |registry: &mut Registry<FakeClock>, sample: [u64; 10]| {
        proc_stat(&t, &stat(sample)).expect("stat");
        clock.advance(Duration::from_secs(1));
        registry.snapshot().get(Metric::CpuUsage).clone()
    };
    assert_eq!(take(&mut registry, a), NOT_YET);
    assert_eq!(take(&mut registry, b), percent(50, 1));
    registry.restart_intervals();
    assert_eq!(take(&mut registry, c), NOT_YET);
    // From c to d: 10 busy of 100.
    assert_eq!(take(&mut registry, d), percent(10, 1));
}

#[test]
fn memory_load_is_the_share_of_memory_not_available() {
    let t = tree("meminfo");
    proc_meminfo(&t, 31_000_000, 7_750_000).expect("meminfo");
    let mut registry = registry(&t, &FakeClock::new());
    assert_eq!(registry.snapshot().get(Metric::MemoryLoad), &percent(75, 1));
}

#[test]
fn meminfo_without_memavailable_is_a_failed_read() {
    let t = tree("meminfo-old");
    t.file(
        "proc/meminfo",
        "MemTotal:       31000000 kB\nMemFree:         1000000 kB\n",
    )
    .expect("meminfo");
    let mut registry = registry(&t, &FakeClock::new());
    assert!(matches!(
        registry.snapshot().get(Metric::MemoryLoad),
        Reading::Unavailable(Unavailable::ReadFailed { .. })
    ));
}

proptest! {
    #[test]
    fn the_busy_share_stays_between_0_and_100_percent_for_rising_counters(
        delta in prop::array::uniform10(0u64..1_000_000),
    ) {
        let readings = usage("property", &[START, plus(delta)]);
        match &readings[1] {
            Reading::Value(exact) => {
                prop_assert!(Exact::whole(0) <= *exact && *exact <= Exact::whole(100), "{exact:?}");
            }
            // No jiffies in the interval and no earlier value.
            other => prop_assert_eq!(other, &NOT_YET),
        }
    }
}
