//! What the service knows about itself, for its D-Bus side and journal (Phase 7 plan, Task 6):
//! the link to the cooler, the settings in force, which modes can be shown, each metric's
//! binding, the GPUs and fan inputs found, and the fans' speeds. The engine, device and sensor
//! threads each write their part; the D-Bus side only reads (one `Arc<RwLock<Status>>`).

use std::path::Path;

use odc_device::Link;
use odc_sensors::{Binding, Fan, FanInput, Gpu, Metric, RANK_FILE, gpus, review_ranking};

use crate::engine::{Availability, Engine};
use crate::settings::Settings;

#[derive(Clone, Debug, Default)]
pub struct Status {
    /// The link after the last write; `None` before the first.
    pub link: Option<Link>,
    pub settings: Settings,
    pub availability: Availability,
    pub bindings: Vec<(Metric, Binding)>,
    pub gpus: Vec<Gpu>,
    pub fan_inputs: Vec<FanInput>,
    pub cooler_fans: Option<Fan>,
    pub pump: Option<Fan>,
}

impl Status {
    /// The engine's part, after a tick.
    pub fn from_engine(&mut self, engine: &Engine) {
        if self.settings != *engine.settings() {
            self.settings = engine.settings().clone();
        }
        self.availability = engine.availability();
        self.cooler_fans = Some(engine.snapshot().cooler_fans.clone());
        self.pump = Some(engine.snapshot().pump.clone());
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
