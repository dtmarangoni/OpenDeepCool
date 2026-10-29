//! The metric model (Phase 6 plan, Task 2): DeepCreative's names as the expected frames use them,
//! one-line reasons, exact values, snapshots, and the fake readings Phase 7 tests with.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use odc_sensors::fake::FakeReadings;
use odc_sensors::{Exact, Fan, Metric, Reading, Readings, Snapshot, Unavailable};
use odc_testkit::expected::load_mode;

/// The number of every `mode-NNN.txt` in `docs/spec/frames/`.
fn frame_modes() -> Vec<u16> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/spec/frames");
    let mut modes: Vec<u16> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            name.strip_prefix("mode-")?
                .strip_suffix(".txt")?
                .parse()
                .ok()
        })
        .collect();
    modes.sort_unstable();
    assert!(!modes.is_empty(), "no expected frames in {}", dir.display());
    modes
}

#[test]
fn the_seven_metrics_go_by_deepcreatives_names_exactly_the_names_the_frames_use() {
    let names = Metric::ALL.map(Metric::spec_name);
    assert_eq!(
        names,
        [
            "CPU Temperature",
            "CPU Usage",
            "CPU Power",
            "Gpu temperature",
            "Gpu usage",
            "Gpu power",
            "Memory Load",
        ]
    );
    for metric in Metric::ALL {
        assert_eq!(Metric::from_spec_name(metric.spec_name()), Some(metric));
    }
    assert_eq!(Metric::from_spec_name("CPU usage"), None);

    let mut seen = BTreeSet::new();
    for mode in frame_modes() {
        for case in load_mode(mode).cases {
            for (name, _) in &case.readings {
                let metric = Metric::from_spec_name(name).unwrap_or_else(|| {
                    panic!("mode {mode}, {}: {name:?} isn't a metric", case.heading)
                });
                seen.insert(metric);
            }
        }
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), Metric::ALL);
}

/// One of each reason. The match stops compiling when a reason is added, so the list is kept
/// whole.
fn every_reason() -> Vec<Unavailable> {
    let reasons = vec![
        Unavailable::NoRule {
            driver: "fakegpu".into(),
        },
        Unavailable::NoDevice,
        Unavailable::PermissionDenied {
            path: "/sys/class/powercap/fake:0/energy_uj".into(),
        },
        Unavailable::ReadFailed {
            path: "/sys/class/hwmon/hwmon9/temp1_input".into(),
        },
        Unavailable::NotYet,
        Unavailable::IntegratedGpuPower,
        Unavailable::UnknownGpuKind,
        Unavailable::DriverUpdated,
        Unavailable::LibraryFailed {
            library: "NVML".into(),
            error: "libnvidia-ml.so.1: cannot open shared object file\nNo such file".into(),
        },
    ];
    for reason in &reasons {
        match reason {
            Unavailable::NoRule { .. }
            | Unavailable::NoDevice
            | Unavailable::PermissionDenied { .. }
            | Unavailable::ReadFailed { .. }
            | Unavailable::NotYet
            | Unavailable::IntegratedGpuPower
            | Unavailable::UnknownGpuKind
            | Unavailable::DriverUpdated
            | Unavailable::LibraryFailed { .. } => {}
        }
    }
    reasons
}

#[test]
fn every_reason_reads_as_its_own_non_empty_line() {
    let mut texts = BTreeSet::new();
    for reason in every_reason() {
        let text = reason.to_string();
        assert!(!text.trim().is_empty(), "{reason:?}");
        assert!(!text.chars().any(char::is_control), "{reason:?}: {text:?}");
        assert!(texts.insert(text.clone()), "{reason:?} shares {text:?}");
    }
    let odd = Unavailable::LibraryFailed {
        library: "NV\nML".into(),
        error: String::new(),
    };
    assert!(!odd.to_string().chars().any(char::is_control));
}

#[test]
fn whole_number_sources_give_whole_values_and_fractions_compare_by_value() {
    // 57 000 millidegrees and 2 000 000 microwatts.
    let temperature = Exact::new(57_000, 1_000).expect("a fraction");
    assert_eq!(temperature, Exact::whole(57));
    assert_eq!((temperature.num(), temperature.den()), (57_000, 1_000));
    assert_eq!(Exact::new(2_000_000, 1_000_000), Some(Exact::whole(2)));
    assert_eq!(Exact::new(1, 0), None);
    // 23 busy jiffies out of 40 are exactly 57.5 %.
    let usage = Exact::new(100 * 23, 40).expect("a fraction");
    assert!(Exact::whole(57) < usage && usage < Exact::whole(58));
    assert_eq!(usage, Exact::new(115, 2).expect("a fraction"));
    let low = Exact::new(i64::MIN, u64::MAX).expect("a fraction");
    let high = Exact::new(i64::MAX, u64::MAX).expect("a fraction");
    assert!(low < Exact::whole(0) && Exact::whole(0) < high);
}

/// Each metric's own value, distinct and non-zero (AGENTS.md rule 5).
fn own_value(metric: Metric) -> Reading {
    let place = Metric::ALL
        .iter()
        .position(|m| *m == metric)
        .expect("a metric");
    Reading::Value(Exact::whole(11 * (place as i64 + 1)))
}

#[test]
fn a_snapshot_gives_each_metric_its_own_reading() {
    let fans = Fan::Chosen(Reading::Value(Exact::whole(930)));
    let pump = Fan::Chosen(Reading::Unavailable(Unavailable::NoDevice));
    let snapshot = Snapshot::new(own_value, fans.clone(), pump.clone());
    for metric in Metric::ALL {
        assert_eq!(snapshot.get(metric), &own_value(metric), "{metric:?}");
    }
    assert_eq!((snapshot.cooler_fans, snapshot.pump), (fans, pump));
}

#[test]
fn the_fake_plays_its_script_repeats_the_last_snapshot_and_counts_restarts() {
    let first = Snapshot::new(own_value, Fan::NotChosen, Fan::NotChosen);
    let second = Snapshot::new(
        |_| Reading::Unavailable(Unavailable::NotYet),
        Fan::NotChosen,
        Fan::NotChosen,
    );
    let mut fake = FakeReadings::new([first.clone(), second.clone()]);
    assert_eq!(fake.snapshot(), first);
    assert_eq!(fake.snapshot(), second);
    assert_eq!(fake.snapshot(), second);
    assert_eq!(fake.restarts(), 0);
    fake.restart_intervals();
    fake.restart_intervals();
    assert_eq!(fake.restarts(), 2);
}
