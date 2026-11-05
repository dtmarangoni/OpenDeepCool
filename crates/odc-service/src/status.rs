//! What the service knows about itself, for its D-Bus side and journal (Phase 7 plan, Tasks 6
//! and 7): the link to the cooler, which modes can be shown, each metric's binding and reading,
//! the GPUs and fan inputs found, and the dev copy's release and override. The engine, device and
//! sensor threads each write their part; the D-Bus side only reads (one `Arc<RwLock<Status>>`).
//! The settings aren't here: the D-Bus side keeps the saved ones itself.

use std::path::Path;

use odc_device::Link;
use odc_sensors::{
    Binding, Exact, FanInput, Gpu, Metric, RANK_FILE, Reading, Snapshot, delete_stale, gpus,
    review_ranking,
};

use crate::dbus::{kind_of, reason_of};
use crate::engine::{Availability, Engine};
use crate::log::Priority;
use crate::settings::{Settings, SettingsStore};

/// How the journal line with the first snapshot's readings begins.
pub const FIRST_READINGS: &str = "first readings: ";

/// The journal line the service writes once per start, with the snapshot its first frame is drawn
/// from (owner, 2026-11-04; Phase 7 plan, Task 9): each metric by its spec name, then its exact
/// value as `num/den` or its reason's kind, as `OverrideReadings` takes them (`not-yet`).
pub fn first_readings(snapshot: &Snapshot) -> String {
    let readings: Vec<String> = Metric::ALL
        .iter()
        .map(|&metric| {
            let reading = match snapshot.get(metric) {
                Reading::Value(exact) => format!("{}/{}", exact.num(), exact.den()),
                Reading::Unavailable(reason) => kind_of(reason).to_owned(),
            };
            format!("{} {reading}", metric.spec_name())
        })
        .collect();
    format!("{FIRST_READINGS}{}", readings.join(", "))
}

/// The readings of a [`first_readings`] line, every metric's in the order of [`Metric::ALL`];
/// `None` when the line isn't one.
pub fn parse_first_readings(line: &str) -> Option<Vec<(Metric, Reading)>> {
    let readings: Vec<(Metric, Reading)> = line
        .strip_prefix(FIRST_READINGS)?
        .split(", ")
        .map(|entry| {
            let (name, reading) = entry.rsplit_once(' ')?;
            let metric = Metric::from_spec_name(name)?;
            let reading = match reading.split_once('/') {
                Some((num, den)) => {
                    Reading::Value(Exact::new(num.parse().ok()?, den.parse().ok()?)?)
                }
                None => Reading::Unavailable(reason_of(reading)?),
            };
            Some((metric, reading))
        })
        .collect::<Option<_>>()?;
    let metrics: Vec<Metric> = readings.iter().map(|(metric, _)| *metric).collect();
    (metrics == Metric::ALL).then_some(readings)
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    /// The link after the last write; `None` before the first.
    pub link: Option<Link>,
    /// A test holds the cooler (the dev copy only).
    pub released: bool,
    pub availability: Availability,
    pub bindings: Vec<(Metric, Binding)>,
    pub gpus: Vec<Gpu>,
    pub fan_inputs: Vec<FanInput>,
    /// The readings drawn; `None` before the engine's first tick.
    pub snapshot: Option<Snapshot>,
    /// The last readings override the engine took: its number and its tick.
    pub adopted: Option<(u64, u64)>,
}

impl Status {
    /// The engine's part, after a tick; whether any of it changed.
    pub fn from_engine(&mut self, engine: &Engine) -> bool {
        let availability = engine.availability();
        let mut changed = false;
        if self.availability != availability {
            self.availability = availability;
            changed = true;
        }
        if self.snapshot.as_ref() != Some(engine.snapshot()) {
            self.snapshot = Some(engine.snapshot().clone());
            changed = true;
        }
        if self.adopted != engine.adopted() {
            self.adopted = engine.adopted();
            changed = true;
        }
        changed
    }
}

/// The GPUs at start-up, with the GPU-rank helper's kinds (Phase 6 Task 7), and what the start
/// did to the ranking and the GPU choice.
pub struct GpusAtStart {
    pub gpus: Vec<Gpu>,
    /// The journal's lines, each with its priority.
    pub lines: Vec<(Priority, String)>,
}

/// Reads the ranking in `state_dir`, deleting a broken one. When GPUs were added or removed since
/// it was written, or the chosen GPU is gone, the user's choice is cleared, so the GPU is chosen
/// automatically, and saved; only then is a stale ranking deleted, so the helper ranks again at
/// the next start (owner, 2026-11-04). A cleared choice that can't be saved still holds for this
/// run, and the ranking stays, so the next start tries again.
pub fn gpus_at_start(
    sys: &Path,
    state_dir: &Path,
    store: &SettingsStore,
    settings: &mut Settings,
) -> GpusAtStart {
    let path = state_dir.join(RANK_FILE);
    let review = review_ranking(sys, &path);
    let gpus = gpus(sys, review.ranking.as_ref());
    let mut lines: Vec<(Priority, String)> = review
        .deleted
        .into_iter()
        .map(|why| (Priority::Notice, why))
        .collect();
    let gone = settings
        .gpu_choice
        .as_ref()
        .is_some_and(|pci| !gpus.iter().any(|gpu| gpu.pci.to_string() == *pci));
    let mut saved = true;
    if (review.stale || gone)
        && let Some(pci) = settings.gpu_choice.take()
    {
        let cleared = format!(
            "the GPUs changed: the choice of {pci} is cleared, and the GPU chosen automatically"
        );
        match store.store(settings) {
            Ok(()) => lines.push((Priority::Notice, cleared)),
            Err(e) => {
                saved = false;
                lines.push((
                    Priority::Error,
                    format!(
                        "{cleared} for this run, but the settings can't be saved: {e}; the next \
                         start tries again"
                    ),
                ));
            }
        }
    }
    if review.stale && saved {
        lines.push((Priority::Notice, delete_stale(&path)));
    }
    GpusAtStart { gpus, lines }
}

#[cfg(test)]
mod tests {
    use odc_sensors::{Fan, Unavailable};

    use super::*;

    #[test]
    fn the_first_readings_line_reads_back_exactly() {
        let snapshot = Snapshot::new(
            |metric| match metric {
                Metric::CpuUsage => Reading::Unavailable(Unavailable::NotYet),
                Metric::GpuPower => Reading::Unavailable(Unavailable::IntegratedGpuPower),
                other => {
                    Reading::Value(Exact::new(45_000 + other as i64, 1000).expect("a denominator"))
                }
            },
            Fan::NotChosen,
            Fan::NotChosen,
        );
        let line = first_readings(&snapshot);
        assert!(line.starts_with("first readings: CPU Temperature 45000/1000, CPU Usage not-yet"));
        let read = parse_first_readings(&line).expect("it reads back");
        for (metric, reading) in read {
            match (reading, snapshot.get(metric)) {
                (Reading::Value(a), Reading::Value(b)) => {
                    assert_eq!(
                        (a.num(), a.den()),
                        (b.num(), b.den()),
                        "{metric:?} as given"
                    );
                }
                (Reading::Unavailable(a), Reading::Unavailable(b)) => {
                    assert_eq!(kind_of(&a), kind_of(b));
                }
                other => panic!("{metric:?}: {other:?}"),
            }
        }
        for broken in [
            "first readings: CPU Temperature 1/1",
            "readings: CPU Temperature 1/1",
            &line.replace("45000/1000", "45000/0"),
            &line.replace("not-yet", "soon"),
        ] {
            assert_eq!(parse_first_readings(broken), None, "{broken}");
        }
    }
}
