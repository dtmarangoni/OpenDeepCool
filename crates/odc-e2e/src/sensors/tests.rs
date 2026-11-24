use super::*;

#[test]
fn json_numbers_read_in_thousandths_without_floating_point() {
    assert_eq!(number_milli("865.000"), Some(865_000));
    assert_eq!(number_milli("41.5"), Some(41_500));
    assert_eq!(number_milli("-0.250"), Some(-250));
    assert_eq!(number_milli("23"), Some(23_000));
    assert_eq!(number_milli("1.23456"), Some(1_234));
    for bad in ["", ".5", "1e3", "x"] {
        assert_eq!(number_milli(bad), None, "{bad:?}");
    }
}

#[test]
fn pci_chips_are_named_as_libsensors_names_them() {
    let card: PciAddr = "0000:0a:00.0".parse().expect("an address");
    assert_eq!(pci_chip("amdgpu", card), "amdgpu-pci-0a00");
    let igpu: PciAddr = "0000:00:02.0".parse().expect("an address");
    assert_eq!(pci_chip("i915", igpu), "i915-pci-0010");
}

#[test]
fn sensors_json_gives_the_values_the_rules_read() {
    let text = r#"{
  "coretemp-isa-0000": {"Adapter": "ISA adapter", "Package id 0": {"temp1_input": 57.000}, "Core 0": {"temp2_input": 53.000}},
  "amdgpu-pci-0a00": {"edge": {"temp1_input": 41.000}, "junction": {"temp2_input": 77.000}, "PPT": {"power1_average": 182.000}}
}"#;
    let json = json::parse(text).expect("JSON");
    assert_eq!(cpu_package(&json), Ok(57_000));
    assert_eq!(
        gpu_temperature(&json, "amdgpu-pci-0a00", "amdgpu"),
        Ok(41_000)
    );
    assert_eq!(
        chip_value(&json, "amdgpu-pci-0a00", None, |s| s == "power1_average"),
        Some(182_000)
    );
}

/// The owner's limit (2026-11-17): the checks never push the CPU towards 60 °C.
#[test]
fn the_cpu_load_is_one_thread_for_each_16_cpus_and_at_least_one() {
    for (cpus, busy) in [(1, 1), (4, 1), (15, 1), (16, 1), (20, 1), (32, 2), (64, 4)] {
        assert_eq!(load_threads(cpus), busy, "{cpus} CPUs");
    }
}

/// The load stops itself at the limit, within a look or two, and not before it.
#[test]
fn the_cpu_load_stops_itself_at_the_temperature_limit_and_only_there() {
    let dir = std::env::temp_dir().join(format!("odc-e2e-load-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let pc = FakePc::new(&dir).expect("a fake PC");
    let roots = || odc_sensors::Roots {
        sys: pc.tree.roots().sysfs,
        proc: pc.tree.roots().proc,
    };
    // Under the limit: the load runs on.
    fs::write(&pc.package_input, format!("{}\n", LOAD_STOP_TEMP - 1_000)).expect("a reading");
    let load = CpuLoad::start(roots());
    thread::sleep(Duration::from_millis(600));
    assert_eq!(load.capped(), None, "a degree under the limit");
    // At it: the load says so, and its threads end.
    fs::write(&pc.package_input, format!("{LOAD_STOP_TEMP}\n")).expect("a reading");
    let waited = Instant::now();
    while load.capped().is_none() && waited.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(load.capped(), Some(LOAD_STOP_TEMP));
    assert!(
        waited.elapsed() < Duration::from_secs(1),
        "stopped within a few looks: {:?}",
        waited.elapsed()
    );
    assert!(load.stop.load(Ordering::Relaxed), "the threads are told");
    drop(load);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_check_says_when_the_load_stopped_at_the_limit() {
    assert_eq!(capped("x".into(), None), "x");
    assert!(capped("x".into(), Some(50_000)).contains("stopped at 50"));
}

#[test]
fn the_thresholds_are_the_plans() {
    let value = |milli: i64| Reading::Value(odc_sensors::Exact::new(milli, 1_000).expect("exact"));
    // One thread on 20 CPUs: a share of 5 %.
    let share = load_share(1, 20);
    assert_eq!(share, 5_000);
    assert!(cpu_usage(&value(5_100), Ok(6_000), share).is_ok());
    assert!(
        cpu_usage(&value(4_000), Ok(4_000), share).is_ok(),
        "80 % of the share"
    );
    assert!(
        cpu_usage(&value(3_900), Ok(3_900), share).is_err(),
        "below 80 % of the share"
    );
    assert!(
        cpu_usage(&value(5_100), Ok(11_000), share).is_err(),
        "5.9 points from top"
    );
    assert!(
        cpu_usage(&value(100_000), Ok(5_000), share).is_err(),
        "stuck at 100 % under a light load"
    );
    // One CPU, all of it busy.
    assert_eq!(load_share(1, 1), 100_000);
    assert!(cpu_usage(&value(97_000), Ok(99_000), 100_000).is_ok());
    assert!(
        cpu_usage(&value(79_000), Ok(79_000), 100_000).is_err(),
        "below 80 %"
    );
    assert!(cpu_power(&value(6_000), &value(88_000)).is_ok());
    assert!(
        cpu_power(&value(88_000), &value(6_000)).is_err(),
        "not above idle"
    );
    assert!(cpu_power(&value(500), &value(88_000)).is_err(), "below 1 W");
    assert!(
        cpu_temp_load(&value(30_000), &value(33_000), Ok(33_000)).is_ok(),
        "3 °C up"
    );
    assert!(
        cpu_temp_load(&value(30_000), &value(32_900), Ok(32_900)).is_err(),
        "less than 3 °C up"
    );
    assert!(
        cpu_temp_load(&value(30_000), &value(30_000), Ok(30_000)).is_err(),
        "a reading stuck at its idle value"
    );
    assert!(
        cpu_temp_load(&value(80_000), &value(60_000), Ok(60_000)).is_err(),
        "lower than at idle"
    );
    assert!(
        cpu_temp_load(&value(30_000), &value(70_000), Ok(72_000)).is_ok(),
        "2 °C apart"
    );
    assert!(
        cpu_temp_load(&value(30_000), &value(70_000), Ok(72_100)).is_err(),
        "more than 2 °C from sensors"
    );
    assert!(
        cpu_temp_load(&value(30_000), &value(70_000), Err("no sensors".into())).is_err(),
        "no reference"
    );
    assert!(
        cpu_temp_load(
            &Reading::Unavailable(odc_sensors::Unavailable::NotYet),
            &value(70_000),
            Ok(70_000)
        )
        .is_err(),
        "no idle reading"
    );
    // The thread's CPU clock counts what the thread computes and not what it waits for.
    let slept = thread_cpu();
    thread::sleep(Duration::from_millis(50));
    assert!(
        thread_cpu() - slept < Duration::from_millis(10),
        "a sleep costs no CPU"
    );
    let burned = thread_cpu();
    let began = Instant::now();
    while began.elapsed() < Duration::from_millis(30) {
        hint::spin_loop();
    }
    assert!(
        thread_cpu() - burned >= Duration::from_millis(20),
        "a busy 30 ms costs CPU"
    );
    assert!(sensors_cost(1.0).is_ok());
    assert!(
        sensors_cost(16.0).is_ok(),
        "a CPU ten times as slow as the development PC's"
    );
    assert!(sensors_cost(100.0).is_ok(), "exactly the ceiling");
    assert!(sensors_cost(100.01).is_err(), "above it");
    assert!(
        sensors_cost(240.0)
            .expect_err("a loop that no longer waits")
            .contains("240.00 ms"),
        "the note names the cost"
    );
    assert!(memory_load(&value(62_400), 62_500).is_ok());
    assert!(memory_load(&value(61_900), 62_500).is_err());
    assert!(gpu_power(&value(30_000), 25_000).is_ok(), "5 W");
    assert!(gpu_power(&value(200_000), 182_000).is_ok(), "10 %");
    assert!(gpu_power(&value(210_000), 182_000).is_err());
    assert!(gpu_usage(&[50_000, 95_000], &[30_000]).is_ok());
    assert!(gpu_usage(&[85_000], &[]).is_err(), "below 90 %");
    assert!(gpu_usage(&[95_000], &[60_000]).is_err(), "didn't drop");
}
