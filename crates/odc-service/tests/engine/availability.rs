use super::*;

// P6-04: a metric with no device stays available for the first two minutes after start-up, while
// the registry still looks for late drivers; after that its modes leave AvailableModes, and come
// back once it's found.
#[test]
fn late_sources_keep_their_modes_for_two_minutes() {
    let no_gpu = with(value(42), Reading::Unavailable(Unavailable::NoDevice));
    let mut engine = Engine::new(Settings::default(), no_gpu.clone());
    for _ in 0..300 {
        engine.tick(None, None, false);
    }
    let early = engine.availability();
    for mode in [2, 4, 6] {
        assert!(early.modes.contains(&mode), "mode {mode}: {early:?}");
    }
    // Found 30 s after start: it never left.
    let found = Published {
        snapshot: with(value(42), value(67)),
        answers: 0,
    };
    let mut engine2 = Engine::new(Settings::default(), no_gpu.clone());
    for t in 0..LATE_SOURCE_TICKS + 100 {
        let p = (t == 300).then(|| found.clone());
        engine2.tick(None, p, false);
        assert!(engine2.availability().modes.contains(&2), "tick {t}");
    }
    // Still missing after two minutes: modes 2, 4 and 6 go, with the reason; found later, back.
    for _ in 300..LATE_SOURCE_TICKS {
        engine.tick(None, None, false);
    }
    let late = engine.availability();
    for mode in [2, 4, 6] {
        assert!(!late.modes.contains(&mode), "mode {mode}");
        assert!(late.reasons[&mode].contains("no device"), "{late:?}");
    }
    assert!(late.modes.contains(&1) && late.modes.contains(&3) && late.modes.contains(&5));
    engine.tick(None, Some(found), false);
    assert!(engine.availability().modes.contains(&4));
    // A wake-up starts the two minutes again.
    engine.tick(
        Some(Apply {
            wake: true,
            ..Apply::default()
        }),
        Some(Published {
            snapshot: no_gpu,
            answers: 0,
        }),
        false,
    );
    assert!(engine.availability().modes.contains(&2), "after a wake-up");
}

// A GPU metric losing its device mid-run: dashes, the mode stays, and its modes leave
// AvailableModes once the late-source window is over; a lasting reason counts at once.
#[test]
fn a_reading_lost_mid_run_draws_dashes_and_the_mode_stays() {
    let mode4 = Settings {
        mode: 4,
        ..Settings::default()
    };
    let mut engine = Engine::new(mode4, with(value(42), value(67)));
    for _ in 0..LATE_SOURCE_TICKS {
        engine.tick(None, None, false);
    }
    let lost = with(value(42), Reading::Unavailable(Unavailable::NoDevice));
    let out = engine.tick(
        None,
        Some(Published {
            snapshot: lost.clone(),
            answers: 0,
        }),
        false,
    );
    assert_eq!(engine.mode().number(), 4);
    let dashes = readings(&[("CPU Usage", Some(42)), ("Gpu usage", None)]);
    let want = Engine::new(
        Settings {
            mode: 4,
            ..Settings::default()
        },
        snapshot(&dashes),
    )
    .tick(None, None, false)
    .frame;
    assert_eq!(out.frame, want, "42% over dashes");
    // Every mode without a GPU metric: `with` gives the GPU's temperature and power the same
    // reading as its usage.
    let modes = engine.availability().modes;
    assert_eq!(
        modes,
        vec![1, 3, 5, 7, 10, 13, 14, 15, 16, 18, 20, 22, 23, 24, 25, 26]
    );
    // A reading the PC may not give at all counts at once, settled or not.
    let denied = with(
        Reading::Unavailable(Unavailable::PermissionDenied {
            path: "/sys/fake".into(),
        }),
        value(67),
    );
    let early = availability(&denied, false);
    assert_eq!(
        early.modes,
        vec![
            2, 3, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 19, 21, 22, 23, 24, 25, 26
        ]
    );
}

// P8-03: every mode is available while the readings let it be.
#[test]
fn every_mode_is_available_while_the_readings_allow_it() {
    let all = with(value(42), value(67));
    for settled in [false, true] {
        let a = availability(&all, settled);
        let every: Vec<u32> = Mode::all().map(|m| u32::from(m.number())).collect();
        assert_eq!(a.modes, every);
        assert!(a.reasons.is_empty(), "{:?}", a.reasons);
    }
}

// Phase 8 plan, Task 4: a CPU whose driver has no temperature rule (k10temp before family 17h,
// for example) takes mode 7 out of AvailableModes with its reason, settled or not; the other
// modes stay.
#[test]
fn mode_7_leaves_available_modes_without_a_cpu_temperature_rule() {
    let no_rule = Snapshot::new(|metric| match metric {
        Metric::CpuTemperature => Reading::Unavailable(Unavailable::NoRule {
            driver: "made-up-temp".to_owned(),
        }),
        _ => value(50),
    });
    for settled in [false, true] {
        let a = availability(&no_rule, settled);
        assert!(!a.modes.contains(&7), "{:?}", a.modes);
        assert_eq!(a.reasons[&7], "CPU Temperature: no reading for this driver");
        assert!(a.modes.starts_with(&[1, 2, 3, 4, 5, 6]));
    }
}

// Phase 8 plan, Task 5: mode 8 leaves AvailableModes when no GPU is found once the first two
// minutes are over, and at once when its driver has no temperature rule, each with its reason.
#[test]
fn mode_8_leaves_available_modes_without_a_gpu_or_a_gpu_temperature_rule() {
    let gpu = |reading: Reading| {
        Snapshot::new(|metric| match metric {
            Metric::GpuTemperature => reading.clone(),
            _ => value(50),
        })
    };
    let no_gpu = gpu(Reading::Unavailable(Unavailable::NoDevice));
    assert!(
        availability(&no_gpu, false).modes.contains(&8),
        "still looked for"
    );
    let settled = availability(&no_gpu, true);
    assert!(!settled.modes.contains(&8), "{:?}", settled.modes);
    assert_eq!(settled.reasons[&8], "Gpu temperature: no device found");
    let no_rule = gpu(Reading::Unavailable(Unavailable::NoRule {
        driver: "made-up-gpu".to_owned(),
    }));
    for settled in [false, true] {
        let a = availability(&no_rule, settled);
        assert!(!a.modes.contains(&8), "{:?}", a.modes);
        assert_eq!(a.reasons[&8], "Gpu temperature: no reading for this driver");
        assert!(a.modes.contains(&7), "the CPU's mode stays");
    }
}

// Phase 8 plan, Task 6: mode 9 leaves AvailableModes when either temperature has no rule, with
// the first one's reason, top first.
#[test]
fn mode_9_leaves_available_modes_when_either_temperature_has_no_rule() {
    let no_rule = |driver: &str| {
        Reading::Unavailable(Unavailable::NoRule {
            driver: driver.to_owned(),
        })
    };
    let temperatures = |cpu: Reading, gpu: Reading| {
        Snapshot::new(|metric| match metric {
            Metric::CpuTemperature => cpu.clone(),
            Metric::GpuTemperature => gpu.clone(),
            _ => value(50),
        })
    };
    let cases = [
        (no_rule("made-up-temp"), value(38), "CPU Temperature"),
        (value(57), no_rule("made-up-gpu"), "Gpu temperature"),
        (
            no_rule("made-up-temp"),
            no_rule("made-up-gpu"),
            "CPU Temperature",
        ),
    ];
    for (cpu, gpu, first) in cases {
        let a = availability(&temperatures(cpu, gpu), true);
        assert!(!a.modes.contains(&9), "{:?}", a.modes);
        assert_eq!(
            a.reasons[&9],
            format!("{first}: no reading for this driver")
        );
    }
    assert!(
        availability(&temperatures(value(57), value(38)), true)
            .modes
            .contains(&9)
    );
}

// Phase 8 plan, Task 7: without the udev rule that lets the service read the CPU's energy
// counter, mode 10 leaves AvailableModes at once, with its reason; it's never drawn as 0 W.
#[test]
fn mode_10_leaves_available_modes_when_the_cpu_power_counter_is_denied() {
    let denied = Snapshot::new(|metric| match metric {
        Metric::CpuPower => Reading::Unavailable(Unavailable::PermissionDenied {
            path: "/sys/fake/energy_uj".into(),
        }),
        _ => value(50),
    });
    for settled in [false, true] {
        let a = availability(&denied, settled);
        assert!(!a.modes.contains(&10), "{:?}", a.modes);
        assert_eq!(a.reasons[&10], "CPU Power: no permission to read it");
        assert!(a.modes.contains(&7), "the other CPU modes stay");
    }
}

// Phase 8 plan, Task 8 (owner, 2026-10-27; solution review P8-06): mode 11 leaves AvailableModes
// on integrated AMD graphics, on an AMD GPU of unknown kind (an unranked APU, as Phase 6's sensor
// fixtures read it), on a driver without a power rule, and once no GPU is found after the first
// two minutes, each with its reason; the CPU's power mode stays.
#[test]
fn mode_11_leaves_available_modes_when_the_gpu_power_cant_be_shown() {
    let gpu_power = |reading: Reading| {
        Snapshot::new(|metric| match metric {
            Metric::GpuPower => reading.clone(),
            _ => value(50),
        })
    };
    let lasting = [
        (
            Unavailable::IntegratedGpuPower,
            "an integrated GPU's power includes the CPU's",
        ),
        (
            Unavailable::UnknownGpuKind,
            "the GPU's kind is unknown, so its power might include the CPU's",
        ),
        (
            Unavailable::NoRule {
                driver: "made-up-gpu".to_owned(),
            },
            "no reading for this driver",
        ),
    ];
    for (reason, text) in lasting {
        for settled in [false, true] {
            let a = availability(&gpu_power(Reading::Unavailable(reason.clone())), settled);
            assert!(!a.modes.contains(&11), "{reason:?}: {:?}", a.modes);
            assert_eq!(a.reasons[&11], format!("Gpu power: {text}"));
            assert!(a.modes.contains(&10), "the CPU's power stays");
        }
    }
    let no_gpu = gpu_power(Reading::Unavailable(Unavailable::NoDevice));
    assert!(
        availability(&no_gpu, false).modes.contains(&11),
        "still looked for"
    );
    let settled = availability(&no_gpu, true);
    assert!(!settled.modes.contains(&11));
    assert_eq!(settled.reasons[&11], "Gpu power: no device found");
}

// Phase 8 plan, Task 8: mode 12 leaves AvailableModes when either power can't be read for good,
// with the first one's reason, top first; the unknown-kind APU takes it out with mode 11.
#[test]
fn mode_12_leaves_available_modes_when_either_power_cant_be_shown() {
    let powers = |cpu: Reading, gpu: Reading| {
        Snapshot::new(|metric| match metric {
            Metric::CpuPower => cpu.clone(),
            Metric::GpuPower => gpu.clone(),
            _ => value(50),
        })
    };
    let denied = || {
        Reading::Unavailable(Unavailable::PermissionDenied {
            path: "/sys/fake/energy_uj".into(),
        })
    };
    let unknown = || Reading::Unavailable(Unavailable::UnknownGpuKind);
    let cases = [
        (denied(), value(250), "CPU Power: no permission to read it"),
        (
            value(65),
            unknown(),
            "Gpu power: the GPU's kind is unknown, so its power might include the CPU's",
        ),
        (denied(), unknown(), "CPU Power: no permission to read it"),
    ];
    for (cpu, gpu, reason) in cases {
        for settled in [false, true] {
            let a = availability(&powers(cpu.clone(), gpu.clone()), settled);
            assert!(!a.modes.contains(&12), "{:?}", a.modes);
            assert_eq!(a.reasons[&12], reason);
        }
    }
    let apu = availability(&powers(value(65), unknown()), true);
    assert!(!apu.modes.contains(&11) && !apu.modes.contains(&12));
    assert!(apu.modes.contains(&10), "the CPU's power stays");
    assert!(
        availability(&powers(value(65), value(250)), true)
            .modes
            .contains(&12)
    );
}

// The first second of a reading averaged over the interval draws dashes, then digits.
#[test]
fn not_yet_draws_dashes_then_digits() {
    let first = with(Reading::Unavailable(Unavailable::NotYet), value(67));
    let mut engine = Engine::new(Settings::default(), first);
    let dashes = engine.tick(None, None, false).frame;
    assert_eq!(dashes, Some(home(1, &readings(&[("CPU Usage", None)]))));
    let every: Vec<u32> = Mode::all().map(|m| u32::from(m.number())).collect();
    assert_eq!(engine.availability().modes, every, "NotYet passes");
    let digits = engine
        .tick(
            None,
            Some(Published {
                snapshot: with(value(42), value(67)),
                answers: 0,
            }),
            false,
        )
        .frame;
    assert_eq!(digits, Some(home(1, &usage(42))));
}
