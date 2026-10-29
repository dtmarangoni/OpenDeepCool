//! Gpu temperature, usage and power for amdgpu, Intel and NVIDIA, over fake trees and a fake NVML
//! (Phase 6 plan, Task 8; `docs/spec/sensors.md`, Driver rules): every reading from the chosen
//! GPU's own files, each metric from its own file, and NVML only for NVIDIA.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use odc_sensors::fake::{FakeClock, FakeNvml};
use odc_sensors::{
    Clock, Exact, Gpu, Metric, PciAddr, Ranked, RankedKind, Ranking, Reading, Readings, Registry,
    Roots, Unavailable, gpus,
};
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{
    PCI_CLASS_VGA, amdgpu_apu, amdgpu_dgpu, amdgpu_power_input_only, hwmon, i915, nouveau,
    nvidia_proprietary, pci_device, xe,
};

const CARD: &str = "0000:0a:00.0";
const IGPU: &str = "0000:00:02.0";

fn tree(name: &str) -> FakeTree {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("gpu_metrics")
        .join(name);
    FakeTree::create(root).expect("a fake tree")
}

fn roots(tree: &FakeTree) -> Roots {
    Roots {
        sys: tree.root().join("sys"),
        proc: tree.root().join("proc"),
    }
}

fn pci(text: &str) -> PciAddr {
    text.parse().expect("a PCI address")
}

fn ranked(at: &str, driver: &str, kind: RankedKind) -> Ranked {
    Ranked {
        pci: pci(at),
        driver: driver.into(),
        kind,
    }
}

fn dedicated(at: &str, driver: &str) -> Ranked {
    ranked(
        at,
        driver,
        RankedKind::Dedicated {
            signal: "the driver's flag".into(),
        },
    )
}

/// A registry over the tree, with `nvml` for NVIDIA's library, reading the GPU at `at`.
fn registry(
    tree: &FakeTree,
    clock: &FakeClock,
    nvml: FakeNvml,
    ranking: &Ranking,
    at: &str,
) -> Registry<FakeClock> {
    let mut registry = Registry::from_rules_with_nvml(clock.clone(), roots(tree), Box::new(nvml));
    let list = gpus(&roots(tree).sys, Some(ranking));
    let gpu: &Gpu = list.iter().find(|g| g.pci == pci(at)).expect("the GPU");
    registry.choose_gpu(Some(gpu));
    registry
}

fn no_nvml() -> FakeNvml {
    FakeNvml::new(Err(Unavailable::NoDevice), &[])
}

/// The three GPU readings of one snapshot.
fn gpu_readings(registry: &mut Registry<FakeClock>) -> [Reading; 3] {
    let snapshot = registry.snapshot();
    [Metric::GpuTemperature, Metric::GpuUsage, Metric::GpuPower].map(|m| snapshot.get(m).clone())
}

fn value(num: i64, den: u64) -> Reading {
    Reading::Value(Exact::new(num, den).expect("a fraction"))
}

fn unavailable(reason: Unavailable) -> Reading {
    Reading::Unavailable(reason)
}

fn no_rule(driver: &str) -> Reading {
    unavailable(Unavailable::NoRule {
        driver: driver.into(),
    })
}

#[test]
fn an_amd_card_reads_edge_not_junction_or_mem_its_busy_share_and_its_average_power() {
    let t = tree("amdgpu");
    // edge 41 °C, junction 77 °C, mem 66 °C: only 41 is the GPU's temperature.
    amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("card");
    let ranking = Ranking {
        gpus: vec![dedicated(CARD, "amdgpu")],
    };
    let mut r = registry(&t, &FakeClock::new(), no_nvml(), &ranking, CARD);
    assert_eq!(
        gpu_readings(&mut r),
        [value(41, 1), value(37, 1), value(182, 1)]
    );
}

#[test]
fn amd_power_is_shown_only_for_a_gpu_ranked_dedicated() {
    let t = tree("amdgpu-apu");
    amdgpu_apu(&t, "0000:0c:00.0", 48_000, 12, 9_000_000).expect("apu");
    let integrated = Ranking {
        gpus: vec![ranked(
            "0000:0c:00.0",
            "amdgpu",
            RankedKind::Integrated {
                signal: "the driver's flag".into(),
            },
        )],
    };
    let mut r = registry(
        &t,
        &FakeClock::new(),
        no_nvml(),
        &integrated,
        "0000:0c:00.0",
    );
    assert_eq!(
        gpu_readings(&mut r),
        [
            value(48, 1),
            value(12, 1),
            unavailable(Unavailable::IntegratedGpuPower)
        ]
    );
    // Unranked, its kind is unknown: it might be integrated, so no power either.
    let mut r = registry(
        &t,
        &FakeClock::new(),
        no_nvml(),
        &Ranking::default(),
        "0000:0c:00.0",
    );
    assert_eq!(
        gpu_readings(&mut r)[2],
        unavailable(Unavailable::UnknownGpuKind)
    );
}

#[test]
fn an_amd_card_without_power1_average_reads_power1_input() {
    let t = tree("amdgpu-input");
    amdgpu_power_input_only(&t, CARD, 41_000, 37, 150_000_000).expect("card");
    let ranking = Ranking {
        gpus: vec![dedicated(CARD, "amdgpu")],
    };
    let mut r = registry(&t, &FakeClock::new(), no_nvml(), &ranking, CARD);
    assert_eq!(gpu_readings(&mut r)[2], value(150, 1));
}

/// Takes a snapshot, sets the energy counter, and takes another a second later.
fn energy_readings(
    t: &FakeTree,
    clock: &FakeClock,
    r: &mut Registry<FakeClock>,
    counter: &str,
    after: u64,
) -> ([Reading; 3], [Reading; 3]) {
    let first = gpu_readings(r);
    t.file(counter, format!("{after}\n")).expect("a counter");
    clock.advance(Duration::from_secs(1));
    (first, gpu_readings(r))
}

#[test]
fn intel_graphics_read_their_temperature_and_energy_and_report_no_usage() {
    let t = tree("i915");
    i915(&t, IGPU, 44_000, 9_100_000).expect("i915");
    let ranking = Ranking::default();
    let clock = FakeClock::new();
    let mut r = registry(&t, &clock, no_nvml(), &ranking, IGPU);
    let counter = "sys/devices/pci0000:00/0000:00:02.0/hwmon/hwmon0/energy1_input";
    let (first, second) = energy_readings(&t, &clock, &mut r, counter, 12_100_000);
    assert_eq!(
        first,
        [
            value(44, 1),
            no_rule("i915"),
            unavailable(Unavailable::NotYet)
        ]
    );
    assert_eq!(second[2], value(3, 1));

    let t = tree("xe");
    xe(&t, CARD, 47_000, 7_300_000).expect("xe");
    let clock = FakeClock::new();
    let mut r = registry(&t, &clock, no_nvml(), &ranking, CARD);
    let counter = "sys/devices/pci0000:00/0000:0a:00.0/hwmon/hwmon0/energy1_input";
    let (first, second) = energy_readings(&t, &clock, &mut r, counter, 9_800_000);
    assert_eq!(
        first,
        [
            value(47, 1),
            no_rule("xe"),
            unavailable(Unavailable::NotYet)
        ]
    );
    assert_eq!(second[2], value(2_500_000, 1_000_000));
    // After a wake-up, the energy interval starts again.
    r.restart_intervals();
    clock.advance(Duration::from_secs(1));
    assert_eq!(gpu_readings(&mut r)[2], unavailable(Unavailable::NotYet));
}

#[test]
fn nouveau_has_no_reading_for_any_gpu_metric() {
    let t = tree("nouveau");
    nouveau(&t, CARD).expect("nouveau");
    let mut r = registry(&t, &FakeClock::new(), no_nvml(), &Ranking::default(), CARD);
    assert_eq!(
        gpu_readings(&mut r),
        [no_rule("nouveau"), no_rule("nouveau"), no_rule("nouveau")]
    );
}

#[test]
fn nvidia_reads_through_nvml_by_the_chosen_gpus_bus_id() {
    let t = tree("nvidia");
    nvidia_proprietary(&t, CARD).expect("nvidia");
    let ranking = Ranking {
        gpus: vec![dedicated(CARD, "nvidia")],
    };
    let matching = FakeNvml::new(Ok(()), &[(CARD, 63, 88, 215_500)]);
    let mut r = registry(&t, &FakeClock::new(), matching, &ranking, CARD);
    assert_eq!(
        gpu_readings(&mut r),
        [value(63, 1), value(88, 1), value(215_500, 1_000)]
    );

    let other = FakeNvml::new(Ok(()), &[("0000:0b:00.0", 63, 88, 215_500)]);
    let mut r = registry(&t, &FakeClock::new(), other, &ranking, CARD);
    let none = unavailable(Unavailable::NoDevice);
    assert_eq!(gpu_readings(&mut r), [none.clone(), none.clone(), none]);
}

#[test]
fn nvml_never_starts_when_the_chosen_gpu_isnt_nvidias() {
    let t = tree("nvml-count");
    amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("card");
    nvidia_proprietary(&t, "0000:0d:00.0").expect("nvidia");
    let ranking = Ranking {
        gpus: vec![
            dedicated(CARD, "amdgpu"),
            dedicated("0000:0d:00.0", "nvidia"),
        ],
    };
    let nvml = FakeNvml::new(Ok(()), &[("0000:0d:00.0", 63, 88, 215_500)]);
    let starts = nvml.starts();
    let clock = FakeClock::new();
    let mut r = registry(&t, &clock, nvml, &ranking, CARD);
    for _ in 0..200 {
        r.snapshot();
        clock.advance(Duration::from_secs(1));
    }
    assert_eq!(starts.load(Ordering::SeqCst), 0, "NVML started for amdgpu");

    let list = gpus(&roots(&t).sys, Some(&ranking));
    let nvidia = list.iter().find(|g| g.driver == "nvidia").expect("nvidia");
    r.choose_gpu(Some(nvidia));
    assert_eq!(gpu_readings(&mut r)[0], value(63, 1));
    assert!(starts.load(Ordering::SeqCst) > 0);
}

#[test]
fn nvmls_failures_reach_every_gpu_metric_with_their_own_reason() {
    let t = tree("nvml-reasons");
    nvidia_proprietary(&t, CARD).expect("nvidia");
    let ranking = Ranking {
        gpus: vec![dedicated(CARD, "nvidia")],
    };
    let library = |error: &str| Unavailable::LibraryFailed {
        library: "NVML".into(),
        error: error.into(),
    };
    let reasons = [
        library(
            "a libloading error occurred: libnvidia-ml.so.1: cannot open shared object file: No such file or directory",
        ),
        library(
            "a libloading error occurred: libnvidia-ml.so.1: cannot open shared object file: Operation not permitted",
        ),
        Unavailable::NoDevice,
        Unavailable::DriverUpdated,
        Unavailable::PermissionDenied {
            path: "libnvidia-ml.so.1".into(),
        },
        library("an internal driver error occurred"),
    ];
    for reason in reasons {
        let nvml = FakeNvml::new(Err(reason.clone()), &[(CARD, 63, 88, 215_500)]);
        let mut r = registry(&t, &FakeClock::new(), nvml, &ranking, CARD);
        let expected = unavailable(reason.clone());
        assert_eq!(
            gpu_readings(&mut r),
            [expected.clone(), expected.clone(), expected],
            "{reason:?}"
        );
    }
}

#[test]
fn only_the_chosen_gpus_own_files_are_read() {
    let t = tree("two-gpus");
    amdgpu_dgpu(&t, CARD, 41_000, 37, 182_000_000).expect("card");
    i915(&t, IGPU, 44_000, 9_100_000).expect("igpu");
    let ranking = Ranking {
        gpus: vec![dedicated(CARD, "amdgpu")],
    };
    let mut r = registry(&t, &FakeClock::new(), no_nvml(), &ranking, CARD);
    assert_eq!(gpu_readings(&mut r)[0], value(41, 1));
    let list = gpus(&roots(&t).sys, Some(&ranking));
    let igpu = list.iter().find(|g| g.driver == "i915").expect("i915");
    r.choose_gpu(Some(igpu));
    assert_eq!(gpu_readings(&mut r)[0], value(44, 1));
    r.choose_gpu(None);
    assert_eq!(gpu_readings(&mut r)[0], unavailable(Unavailable::NoDevice));
}

/// Takes a snapshot every second for `seconds` + 1 seconds, creating the card's hwmon at
/// `appears` seconds, and returns Gpu temperature's readings.
fn gpu_hwmon_appearing_at(name: &str, appears: u64, seconds: u64) -> Vec<Reading> {
    let t = tree(name);
    let gpu = pci_device(&t, CARD, PCI_CLASS_VGA, Some("amdgpu")).expect("the card");
    let ranking = Ranking {
        gpus: vec![dedicated(CARD, "amdgpu")],
    };
    let clock = FakeClock::new();
    let t0 = clock.now();
    let mut r = registry(&t, &clock, no_nvml(), &ranking, CARD);
    (0..=seconds)
        .map(|_| {
            if clock.now() == t0 + Duration::from_secs(appears) {
                hwmon(
                    &t,
                    &gpu,
                    "amdgpu",
                    &[
                        ("temp1_label".into(), "edge".into()),
                        ("temp1_input".into(), "41000".into()),
                    ],
                )
                .expect("the card's hwmon");
            }
            let reading = r.snapshot().get(Metric::GpuTemperature).clone();
            clock.advance(Duration::from_secs(1));
            reading
        })
        .collect()
}

#[test]
fn the_chosen_gpus_hwmon_appearing_late_is_bound_at_the_next_5_s_or_60_s_scan() {
    for (name, appears, bound, seconds) in [("late-30", 31, 35, 40), ("late-180", 181, 240, 250)] {
        for (t, reading) in gpu_hwmon_appearing_at(name, appears, seconds)
            .iter()
            .enumerate()
        {
            let expected = if t as u64 >= bound {
                value(41, 1)
            } else {
                unavailable(Unavailable::NoDevice)
            };
            assert_eq!(*reading, expected, "{name}: at {t} s");
        }
    }
}
