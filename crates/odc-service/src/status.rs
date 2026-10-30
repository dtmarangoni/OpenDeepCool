//! What the service knows about itself, for its D-Bus side and journal (Phase 7 plan, Tasks 6
//! and 7): the link to the cooler, which modes can be shown, each metric's binding and reading,
//! the GPUs and fan inputs found, and the dev copy's release and override. The engine, device and
//! sensor threads each write their part; the D-Bus side only reads (one `Arc<RwLock<Status>>`).
//! The settings aren't here: the D-Bus side keeps the saved ones itself.

use std::path::Path;

use odc_device::Link;
use odc_sensors::{Binding, FanInput, Gpu, Metric, RANK_FILE, Snapshot, gpus, review_ranking};

use crate::engine::{Availability, Engine};
use crate::settings::Settings;

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

/// The GPUs at start-up, with the GPU-rank helper's kinds (Phase 6 Task 7): the ranking is read,
/// and a broken one, or a stale one while no GPU is chosen, deleted so the helper ranks again at
/// the next start.
pub struct GpusAtStart {
    pub gpus: Vec<Gpu>,
    /// When the ranking file was deleted, why, for the journal.
    pub deleted: Option<String>,
}

pub fn gpus_at_start(sys: &Path, state_dir: &Path, settings: &Settings) -> GpusAtStart {
    let review = review_ranking(
        sys,
        &state_dir.join(RANK_FILE),
        settings.gpu_choice.is_some(),
    );
    GpusAtStart {
        gpus: gpus(sys, review.ranking.as_ref()),
        deleted: review.deleted,
    }
}
