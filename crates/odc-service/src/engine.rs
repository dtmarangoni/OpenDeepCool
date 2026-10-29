//! The engine (Phase 7 plan, Task 6): the tick logic, with no I/O and no clock. Each tick it
//! adopts the newest settings and the newest snapshot, decides between a reset and an update of
//! the picture (`docs/spec/display.md`, Mode switches and Movement, precisely), draws one frame,
//! turns it last, and works out which modes can be shown. The threads around it, and the clock,
//! are in [`crate::runtime`].

use std::collections::BTreeMap;

use odc_protocol::{Frame, Rotation};
use odc_render::{Marquee, Mode, Picture, Unit, Values, draws, mode_info, picture};
use odc_sensors::{Snapshot, Unavailable};

use crate::settings::Settings;

/// Ticks in the two minutes after start-up or a wake-up during which a metric without a device
/// may still find one, so `NoDevice` doesn't count as lasting yet (owner, 2026-10-27; solution
/// review P6-04): 1 200 ticks of 100 ms.
pub const LATE_SOURCE_TICKS: u64 = 1200;

/// What the D-Bus side posts for the engine: the newest complete settings, Refresh, and a
/// wake-up from standby. A newer post replaces an older one not yet taken, so only the newest
/// settings matter (owner, 2026-10-27; solution review P7-15); the flags add up.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Apply {
    pub settings: Option<Settings>,
    pub refresh: bool,
    pub wake: bool,
}

impl Apply {
    /// `newer` posted after `self`, before the engine took `self`.
    pub fn then(self, newer: Apply) -> Apply {
        Apply {
            settings: newer.settings.or(self.settings),
            refresh: self.refresh || newer.refresh,
            wake: self.wake || newer.wake,
        }
    }
}

/// A snapshot the sensor thread published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    pub snapshot: Snapshot,
    /// The highest snapshot request the sensor thread had taken before it read this one.
    pub answers: u64,
}

/// What the engine asks of the sensor thread after a tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Requests {
    /// A snapshot at once, numbered so the answer can be told apart: Refresh, or another GPU.
    pub snapshot: Option<u64>,
    /// The GPU and fan inputs to read, when they changed.
    pub choices: Option<Choices>,
    /// Start the readings averaged over the interval again: a wake-up.
    pub restart: bool,
}

/// The sensor choices the settings hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choices {
    pub gpu: Option<String>,
    pub cooler_fan_input: Option<crate::settings::SavedInput>,
    pub pump_input: Option<crate::settings::SavedInput>,
}

impl Choices {
    pub fn of(settings: &Settings) -> Choices {
        Choices {
            gpu: settings.gpu_choice.clone(),
            cooler_fan_input: settings.cooler_fan_input.clone(),
            pump_input: settings.pump_input.clone(),
        }
    }
}

/// Which modes can be shown, and why the others can't.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Availability {
    /// The modes this build draws whose every metric can be read.
    pub modes: Vec<u32>,
    /// Every other mode, with a one-line reason.
    pub reasons: BTreeMap<u32, String>,
}

/// One tick's result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tick {
    pub number: u64,
    /// The frame, turned; `None` for a missed deadline.
    pub frame: Option<Frame>,
    pub requests: Requests,
}

/// The engine's state: the settings in force, the readings drawn, and the movement.
pub struct Engine {
    settings: Settings,
    snapshot: Snapshot,
    marquee: Marquee,
    /// The next tick's number.
    tick: u64,
    /// The tick of start-up or of the last wake-up.
    since: u64,
    /// A snapshot request whose answer a reset waits for: Refresh, or another GPU.
    waiting: Option<u64>,
    requests: u64,
}

/// The mode the engine draws for the saved one: the saved mode when this build draws it, mode 1
/// otherwise, which only a downgrade can bring about.
fn shown_mode(settings: &Settings) -> Mode {
    Mode::new(settings.mode)
        .filter(|&mode| draws(mode))
        .or_else(|| Mode::new(1))
        .expect("mode 1 exists")
}

fn picture_of(settings: &Settings, snapshot: &Snapshot) -> Picture {
    let unit: Unit = settings.temperature_unit.into();
    picture(shown_mode(settings), &Values::of(snapshot), unit).expect("a drawn mode")
}

fn rotation(settings: &Settings) -> Rotation {
    Rotation::from_setting(settings.rotation).expect("settings hold rotation 0–3")
}

impl Engine {
    /// The engine at start-up, on the saved settings and the first snapshot: its first frame is
    /// the saved mode's picture at home (`docs/spec/settings.md`, Start-up).
    pub fn new(settings: Settings, snapshot: Snapshot) -> Engine {
        Engine {
            marquee: Marquee::new(picture_of(&settings, &snapshot)),
            settings,
            snapshot,
            tick: 0,
            since: 0,
            waiting: None,
            requests: 0,
        }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// The mode drawn.
    pub fn mode(&self) -> Mode {
        shown_mode(&self.settings)
    }

    /// One tick: adopt what arrived, reset or update the picture, then draw unless the deadline
    /// was missed. A missed tick draws nothing and doesn't move the strip (S22).
    pub fn tick(
        &mut self,
        apply: Option<Apply>,
        published: Option<Published>,
        missed: bool,
    ) -> Tick {
        let mut requests = Requests::default();
        let mut reset = false;
        if let Some(apply) = apply {
            if apply.wake {
                self.since = self.tick;
                requests.restart = true;
            }
            if let Some(new) = apply.settings {
                let old = std::mem::replace(&mut self.settings, new);
                let new = &self.settings;
                reset |= shown_mode(&old) != shown_mode(new)
                    || old.rotation != new.rotation
                    || (old.temperature_unit != new.temperature_unit
                        && shows_temperature(shown_mode(new)));
                if Choices::of(&old) != Choices::of(new) {
                    requests.choices = Some(Choices::of(new));
                    if old.gpu_choice != new.gpu_choice {
                        // Another GPU: its reset waits for a snapshot of the new GPU (Refresh's
                        // rule; Phase 7 plan, Task 6).
                        self.requests += 1;
                        self.waiting = Some(self.requests);
                        requests.snapshot = Some(self.requests);
                    }
                }
            }
            if apply.refresh {
                self.requests += 1;
                self.waiting = Some(self.requests);
                requests.snapshot = Some(self.requests);
            }
        }
        let mut changed = false;
        if let Some(published) = published {
            if self.waiting.is_some_and(|id| published.answers >= id) {
                self.waiting = None;
                reset = true;
            }
            changed = published.snapshot != self.snapshot;
            self.snapshot = published.snapshot;
        }
        let picture = picture_of(&self.settings, &self.snapshot);
        if reset {
            self.marquee.reset(picture);
        } else if changed {
            self.marquee.update(picture);
        }
        let number = self.tick;
        self.tick += 1;
        let frame = (!missed).then(|| self.marquee.tick().rotated(rotation(&self.settings)));
        Tick {
            number,
            frame,
            requests,
        }
    }

    /// Which modes can be shown now (Phase 7 plan, Task 6; `docs/spec/settings.md`, Availability).
    pub fn availability(&self) -> Availability {
        let settled = self.tick.saturating_sub(self.since) >= LATE_SOURCE_TICKS;
        availability(&self.snapshot, settled)
    }
}

/// Why a metric can't be shown for good, or `None` while it can, or may soon: a value, a read
/// that failed and is being retried, the first second of a reading averaged over the interval,
/// and, until `settled`, no device yet.
fn lasting(reason: &Unavailable, settled: bool) -> bool {
    match reason {
        Unavailable::NoRule { .. }
        | Unavailable::PermissionDenied { .. }
        | Unavailable::IntegratedGpuPower
        | Unavailable::UnknownGpuKind
        | Unavailable::DriverUpdated
        | Unavailable::LibraryFailed { .. } => true,
        Unavailable::NoDevice => settled,
        Unavailable::ReadFailed { .. } | Unavailable::NotYet => false,
    }
}

/// The modes a snapshot lets the service show: those this build draws whose every metric can be
/// read; the rest with their first reason (owner, 2026-10-27; solution review P8-03).
pub fn availability(snapshot: &Snapshot, settled: bool) -> Availability {
    let mut result = Availability::default();
    for mode in Mode::all() {
        let number = u32::from(mode.number());
        if !draws(mode) {
            result
                .reasons
                .insert(number, "comes in a later version".to_owned());
            continue;
        }
        let first = mode_info(mode)
            .metrics
            .iter()
            .find_map(|&metric| match snapshot.get(metric) {
                odc_sensors::Reading::Unavailable(reason) if lasting(reason, settled) => {
                    Some(format!("{}: {reason}", metric.spec_name()))
                }
                _ => None,
            });
        match first {
            None => result.modes.push(number),
            Some(reason) => {
                result.reasons.insert(number, reason);
            }
        }
    }
    result
}

/// Whether a mode shows a temperature: a unit change resets only such a mode
/// (`docs/spec/display.md`, Mode switches).
pub fn shows_temperature(mode: Mode) -> bool {
    mode_info(mode).metrics.iter().any(|m| {
        matches!(
            m,
            odc_sensors::Metric::CpuTemperature | odc_sensors::Metric::GpuTemperature
        )
    })
}
