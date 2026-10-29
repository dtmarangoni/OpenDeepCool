//! The registry's binding and rescans, with a fake clock and scripted sources (Phase 6 plan,
//! Task 2; owner, 2026-10-27, solution review P6-04): a metric without a source is looked for
//! every 5 s for two minutes after start-up or a wake-up, then every 60 s, and at once after a
//! failed read. The same behaviour over fake sysfs trees comes with Tasks 4 and 8.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use odc_sensors::fake::FakeClock;
use odc_sensors::{
    Binding, Candidate, Clock, Exact, Metric, Reading, Readings, Registry, Source, Unavailable,
};

fn secs(s: u64) -> Duration {
    Duration::from_secs(s)
}

/// A scripted rule: its device is there from `present_from` on (never when `None`), and each
/// search is logged with its time. Clones share the script and the log.
#[derive(Clone)]
struct Scripted {
    clock: FakeClock,
    present_from: Arc<Mutex<Option<Instant>>>,
    /// What a search answers while the device isn't there.
    absent: Unavailable,
    value: i64,
    searches: Arc<Mutex<Vec<Instant>>>,
    fail_next_read: Arc<AtomicBool>,
    restarts: Arc<AtomicUsize>,
}

impl Scripted {
    fn new(clock: &FakeClock, present_from: Option<Instant>, value: i64) -> Self {
        Scripted {
            clock: clock.clone(),
            present_from: Arc::new(Mutex::new(present_from)),
            absent: Unavailable::NoDevice,
            value,
            searches: Arc::default(),
            fail_next_read: Arc::default(),
            restarts: Arc::default(),
        }
    }

    fn unreadable(clock: &FakeClock) -> Self {
        Scripted {
            absent: Unavailable::PermissionDenied {
                path: "/sys/fake/unreadable".into(),
            },
            ..Scripted::new(clock, None, 0)
        }
    }

    fn boxed(&self) -> Box<dyn Candidate> {
        Box::new(self.clone())
    }

    /// When each search happened, in whole seconds after `t0`.
    fn search_seconds(&self, t0: Instant) -> Vec<u64> {
        let searches = self.searches.lock().expect("the search log");
        searches
            .iter()
            .map(|t| t.duration_since(t0).as_secs())
            .collect()
    }

    fn remove_device(&self) {
        *self.present_from.lock().expect("the script") = None;
    }

    fn fail_next_read(&self) {
        self.fail_next_read.store(true, Ordering::SeqCst);
    }

    fn restarts(&self) -> usize {
        self.restarts.load(Ordering::SeqCst)
    }
}

impl Candidate for Scripted {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        let now = self.clock.now();
        self.searches.lock().expect("the search log").push(now);
        match *self.present_from.lock().expect("the script") {
            Some(from) if now >= from => Ok(Box::new(ScriptedSource {
                value: self.value,
                fail_next_read: Arc::clone(&self.fail_next_read),
                restarts: Arc::clone(&self.restarts),
            })),
            _ => Err(self.absent.clone()),
        }
    }
}

struct ScriptedSource {
    value: i64,
    fail_next_read: Arc<AtomicBool>,
    restarts: Arc<AtomicUsize>,
}

impl Source for ScriptedSource {
    fn read(&mut self, _now: Instant) -> Result<Reading, Unavailable> {
        if self.fail_next_read.swap(false, Ordering::SeqCst) {
            return Err(Unavailable::ReadFailed {
                path: "/sys/fake/source".into(),
            });
        }
        Ok(Reading::Value(Exact::whole(self.value)))
    }

    fn restart(&mut self) {
        self.restarts.fetch_add(1, Ordering::SeqCst);
    }

    fn describe(&self) -> String {
        format!("scripted source {}", self.value)
    }
}

/// A registry where only `metric` has candidates.
fn registry(clock: &FakeClock, metric: Metric, rules: &[&Scripted]) -> Registry<FakeClock> {
    Registry::new(clock.clone(), |m| {
        if m == metric {
            rules.iter().map(|r| r.boxed()).collect()
        } else {
            Vec::new()
        }
    })
}

/// Takes a snapshot every second for `seconds` + 1 seconds, the clock moving one second after
/// each, and returns `metric`'s readings.
fn run(
    registry: &mut Registry<FakeClock>,
    clock: &FakeClock,
    seconds: u64,
    metric: Metric,
) -> Vec<Reading> {
    (0..=seconds)
        .map(|_| {
            let reading = registry.snapshot().get(metric).clone();
            clock.advance(secs(1));
            reading
        })
        .collect()
}

fn value(n: i64) -> Reading {
    Reading::Value(Exact::whole(n))
}

const NO_DEVICE: Reading = Reading::Unavailable(Unavailable::NoDevice);

/// Every 5 s from 0 to 120 s, then the given 60 s steps.
fn schedule(then: &[u64]) -> Vec<u64> {
    (0..=120).step_by(5).chain(then.iter().copied()).collect()
}

#[test]
fn a_source_that_stays_missing_is_looked_for_every_5_s_for_two_minutes_then_every_60_s() {
    let clock = FakeClock::new();
    let t0 = clock.now();
    let missing = Scripted::new(&clock, None, 41);
    let mut registry = registry(&clock, Metric::GpuTemperature, &[&missing]);
    let readings = run(&mut registry, &clock, 300, Metric::GpuTemperature);
    assert!(readings.iter().all(|r| *r == NO_DEVICE));
    assert_eq!(missing.search_seconds(t0), schedule(&[180, 240, 300]));
}

#[test]
fn a_source_that_appears_after_30_s_is_bound_at_the_next_5_s_scan() {
    let clock = FakeClock::new();
    let t0 = clock.now();
    let late = Scripted::new(&clock, Some(t0 + secs(31)), 41);
    let mut registry = registry(&clock, Metric::GpuTemperature, &[&late]);
    let readings = run(&mut registry, &clock, 40, Metric::GpuTemperature);
    for (t, reading) in readings.iter().enumerate() {
        let expected = if t >= 35 { value(41) } else { NO_DEVICE };
        assert_eq!(*reading, expected, "at {t} s");
    }
    assert_eq!(late.search_seconds(t0), [0, 5, 10, 15, 20, 25, 30, 35]);
    assert_eq!(
        registry.bindings()[Metric::GpuTemperature as usize],
        (
            Metric::GpuTemperature,
            Binding::Bound("scripted source 41".into())
        )
    );
}

#[test]
fn a_source_that_appears_after_3_minutes_is_bound_at_the_next_60_s_scan() {
    let clock = FakeClock::new();
    let t0 = clock.now();
    let late = Scripted::new(&clock, Some(t0 + secs(181)), 41);
    let mut registry = registry(&clock, Metric::GpuTemperature, &[&late]);
    let readings = run(&mut registry, &clock, 250, Metric::GpuTemperature);
    for (t, reading) in readings.iter().enumerate() {
        let expected = if t >= 240 { value(41) } else { NO_DEVICE };
        assert_eq!(*reading, expected, "at {t} s");
    }
    assert_eq!(late.search_seconds(t0), schedule(&[180, 240]));
}

#[test]
fn restart_intervals_restarts_the_bound_sources_and_the_5_s_rescans() {
    let clock = FakeClock::new();
    let t0 = clock.now();
    let bound = Scripted::new(&clock, Some(t0), 23);
    let missing = Scripted::new(&clock, None, 41);
    let mut registry = Registry::new(clock.clone(), |metric| match metric {
        Metric::CpuUsage => vec![bound.boxed()],
        Metric::GpuTemperature => vec![missing.boxed()],
        _ => Vec::new(),
    });
    run(&mut registry, &clock, 199, Metric::GpuTemperature);
    assert_eq!(bound.restarts(), 0);
    registry.restart_intervals();
    assert_eq!(bound.restarts(), 1);
    run(&mut registry, &clock, 60, Metric::GpuTemperature);
    let after: Vec<u64> = (200..=260).step_by(5).collect();
    assert_eq!(
        missing.search_seconds(t0),
        schedule(&[180])
            .into_iter()
            .chain(after)
            .collect::<Vec<_>>()
    );
    assert_eq!(bound.search_seconds(t0), [0]);
}

#[test]
fn a_read_that_fails_looks_for_the_source_again_at_once() {
    let clock = FakeClock::new();
    let t0 = clock.now();
    let rule = Scripted::new(&clock, Some(t0), 57);
    let mut registry = registry(&clock, Metric::CpuTemperature, &[&rule]);
    let readings = run(&mut registry, &clock, 51, Metric::CpuTemperature);
    assert!(readings.iter().all(|r| *r == value(57)));

    // At 52 s the read fails, the rule finds its device again, and the new source reads.
    rule.fail_next_read();
    assert_eq!(
        run(&mut registry, &clock, 0, Metric::CpuTemperature),
        [value(57)]
    );
    assert_eq!(rule.search_seconds(t0), [0, 52]);

    // At 53 s the device is gone: the read fails, the search at once finds nothing, and the
    // metric waits for the schedule's next scan, at 55 s.
    rule.remove_device();
    rule.fail_next_read();
    let readings = run(&mut registry, &clock, 3, Metric::CpuTemperature);
    assert_eq!(readings, [NO_DEVICE, NO_DEVICE, NO_DEVICE, NO_DEVICE]);
    assert_eq!(rule.search_seconds(t0), [0, 52, 53, 55]);
}

#[test]
fn candidates_are_tried_in_order_and_each_metric_reads_its_own_source() {
    let clock = FakeClock::new();
    let t0 = clock.now();
    // Each metric has its own distinct value (AGENTS.md rules 5 and 11).
    let own: Vec<Scripted> = Metric::ALL
        .iter()
        .enumerate()
        .map(|(i, _)| Scripted::new(&clock, Some(t0), 11 * (i as i64 + 1)))
        .collect();
    let absent = Scripted::new(&clock, None, 99);
    let unreadable = Scripted::unreadable(&clock);
    let mut registry = Registry::new(clock.clone(), |metric| match metric {
        // The first rule's device isn't there, so the second binds.
        Metric::CpuTemperature => vec![absent.boxed(), own[0].boxed()],
        // A device that's there but unreadable says more than one that isn't there.
        Metric::GpuTemperature => vec![absent.boxed(), unreadable.boxed(), absent.boxed()],
        other => vec![own[other as usize].boxed()],
    });
    let snapshot = registry.snapshot();
    for (i, metric) in Metric::ALL.into_iter().enumerate() {
        let expected = match metric {
            Metric::GpuTemperature => Reading::Unavailable(Unavailable::PermissionDenied {
                path: "/sys/fake/unreadable".into(),
            }),
            _ => value(11 * (i as i64 + 1)),
        };
        assert_eq!(snapshot.get(metric), &expected, "{metric:?}");
    }
    let bindings = registry.bindings();
    assert_eq!(
        bindings[0],
        (
            Metric::CpuTemperature,
            Binding::Bound("scripted source 11".into())
        )
    );
    assert!(matches!(
        &bindings[Metric::GpuTemperature as usize],
        (
            Metric::GpuTemperature,
            Binding::Unbound(Unavailable::PermissionDenied { .. })
        )
    ));
}
