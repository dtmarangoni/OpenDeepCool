//! The registry: it binds one source per metric from that metric's candidates, looks again for a
//! metric that has none, and reads them all for a snapshot (PRD, Architecture Notes: Sensor
//! architecture). The candidates are the driver rules of `docs/spec/sensors.md`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::fans::{self, FanInput};
use crate::model::{Clock, Fan, Metric, Reading, Readings, Snapshot, Unavailable};
use crate::rules::{self, Roots};

/// A metric without a source is looked for every 5 s for the first two minutes after start-up or
/// a wake-up, then every 60 s, so a driver that loads late is found without a restart (owner,
/// 2026-10-27; solution review P6-04; `docs/spec/sensors.md`, Sources that appear late).
const FAST: Duration = Duration::from_secs(5);
const FAST_UNTIL: Duration = Duration::from_secs(120);
const SLOW: Duration = Duration::from_secs(60);

/// One way of finding a metric's source: one of its driver rules.
pub trait Candidate: Send {
    /// The source, or why there's none: [`Unavailable::NoDevice`] when its device isn't there,
    /// another reason when the device is there but can't be read.
    fn find(&self) -> Result<Box<dyn Source>, Unavailable>;
}

/// A source the registry bound to a metric.
pub trait Source: Send {
    /// The reading at `now`. An error means the source failed, for example because its device
    /// vanished, and the registry looks for the metric's source again at once.
    fn read(&mut self, now: Instant) -> Result<Reading, Unavailable>;

    /// Forgets the last sample of a reading averaged over the interval; other sources ignore it.
    fn restart(&mut self) {}

    /// The source's name, for [`Registry::bindings`].
    fn describe(&self) -> String;
}

/// What a metric is bound to, for Phase 7's journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Binding {
    /// The source, by name.
    Bound(String),
    /// No source, and why.
    Unbound(Unavailable),
}

/// Binds and reads one source per metric: the display service's [`Readings`].
pub struct Registry<C: Clock> {
    clock: C,
    slots: [Slot; Metric::ALL.len()],
    /// Where the rescan schedule starts: start-up, or the last wake-up.
    since: Instant,
    next_scan: Instant,
    /// The sysfs root the fan inputs are read under, when the registry reads a real tree.
    sys: Option<PathBuf>,
    cooler_fans: Option<FanInput>,
    pump: Option<FanInput>,
}

struct Slot {
    candidates: Vec<Box<dyn Candidate>>,
    state: State,
}

enum State {
    Bound(Box<dyn Source>),
    Unbound(Unavailable),
}

impl<C: Clock> Registry<C> {
    /// A registry over each metric's `candidates`, tried in their order. The first snapshot looks
    /// for every source.
    pub fn new(clock: C, mut candidates: impl FnMut(Metric) -> Vec<Box<dyn Candidate>>) -> Self {
        let now = clock.now();
        Registry {
            slots: Metric::ALL.map(|metric| Slot {
                candidates: candidates(metric),
                state: State::Unbound(Unavailable::NoDevice),
            }),
            clock,
            since: now,
            next_scan: now,
            sys: None,
            cooler_fans: None,
            pump: None,
        }
    }

    /// A registry over the built-in driver rules, looking under `roots`.
    pub fn from_rules(clock: C, roots: Roots) -> Self {
        let mut registry = Registry::new(clock, |metric| rules::candidates(metric, &roots));
        registry.sys = Some(roots.sys);
        registry
    }

    /// The fan inputs the user chose for the cooler's fans and its pump; none is chosen by
    /// default (`docs/spec/sensors.md`, Fan and pump speed).
    pub fn choose_fans(&mut self, cooler_fans: Option<FanInput>, pump: Option<FanInput>) {
        self.cooler_fans = cooler_fans;
        self.pump = pump;
    }

    fn fan(&self, input: Option<&FanInput>) -> Fan {
        match (input, &self.sys) {
            (None, _) => Fan::NotChosen,
            (Some(input), Some(sys)) => Fan::Chosen(fans::fan_rpm(sys, input)),
            (Some(_), None) => Fan::Chosen(Reading::Unavailable(Unavailable::NoDevice)),
        }
    }

    /// Every metric's binding: its source, or why it has none.
    pub fn bindings(&self) -> Vec<(Metric, Binding)> {
        Metric::ALL
            .into_iter()
            .zip(&self.slots)
            .map(|(metric, slot)| {
                let binding = match &slot.state {
                    State::Bound(source) => Binding::Bound(source.describe()),
                    State::Unbound(reason) => Binding::Unbound(reason.clone()),
                };
                (metric, binding)
            })
            .collect()
    }
}

impl<C: Clock> Readings for Registry<C> {
    fn snapshot(&mut self) -> Snapshot {
        let now = self.clock.now();
        if now >= self.next_scan {
            for slot in &mut self.slots {
                if matches!(slot.state, State::Unbound(_)) {
                    slot.search();
                }
            }
            self.next_scan = next_scan(self.since, now);
        }
        let cooler_fans = self.fan(self.cooler_fans.as_ref());
        let pump = self.fan(self.pump.as_ref());
        let slots = &mut self.slots;
        Snapshot::new(|metric| slots[metric.index()].read(now), cooler_fans, pump)
    }

    fn restart_intervals(&mut self) {
        for slot in &mut self.slots {
            if let State::Bound(source) = &mut slot.state {
                source.restart();
            }
        }
        let now = self.clock.now();
        self.since = now;
        self.next_scan = now;
    }
}

impl Slot {
    /// Tries the candidates in order and binds the first source found. Without one, it keeps the
    /// first reason that isn't `NoDevice`: a device that's there but can't be read says more than
    /// one that isn't there.
    fn search(&mut self) {
        let mut reason = Unavailable::NoDevice;
        for candidate in &self.candidates {
            match candidate.find() {
                Ok(source) => {
                    self.state = State::Bound(source);
                    return;
                }
                Err(Unavailable::NoDevice) => {}
                Err(other) => {
                    if reason == Unavailable::NoDevice {
                        reason = other;
                    }
                }
            }
        }
        self.state = State::Unbound(reason);
    }

    /// Reads the bound source. When the read fails, it looks for a source again at once and
    /// reads the one it finds; when that fails too, the metric waits for the next scan.
    fn read(&mut self, now: Instant) -> Reading {
        let mut searched = false;
        loop {
            let result = match &mut self.state {
                State::Bound(source) => source.read(now),
                State::Unbound(reason) => return Reading::Unavailable(reason.clone()),
            };
            match result {
                Ok(reading) => return reading,
                Err(reason) if searched => {
                    self.state = State::Unbound(reason.clone());
                    return Reading::Unavailable(reason);
                }
                Err(_) => {
                    searched = true;
                    self.search();
                }
            }
        }
    }
}

/// The first time after `now` on the schedule that started at `since`: every 5 s up to two
/// minutes, then every 60 s.
fn next_scan(since: Instant, now: Instant) -> Instant {
    let elapsed = now.saturating_duration_since(since);
    let step = if elapsed < FAST_UNTIL { FAST } else { SLOW };
    let steps = elapsed.as_nanos() / step.as_nanos() + 1;
    let after = u64::try_from(step.as_nanos() * steps).unwrap_or(u64::MAX);
    since
        .checked_add(Duration::from_nanos(after))
        .unwrap_or(now + step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schedule_steps_5_s_until_two_minutes_then_60_s() {
        let since = Instant::now();
        let at = |s: u64| since + Duration::from_secs(s);
        for (now, next) in [
            (0, 5),
            (4, 5),
            (5, 10),
            (115, 120),
            (119, 120),
            (120, 180),
            (150, 180),
            (180, 240),
        ] {
            assert_eq!(next_scan(since, at(now)), at(next), "after {now} s");
        }
    }
}
