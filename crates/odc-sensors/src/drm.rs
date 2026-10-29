//! Gpu usage from the chosen GPU's `gpu_busy_percent` (`Documentation/gpu/amdgpu/thermal.rst`;
//! `docs/spec/sensors.md`, Driver rules): the driver's own percentage, as read.

use std::path::PathBuf;
use std::time::Instant;

use crate::model::{Exact, Reading, Unavailable};
use crate::registry::{Candidate, Source};
use crate::sysfs::read_integer;

/// The busy percentage of the GPU whose PCI device folder is `device`.
pub(crate) struct BusyPercentCandidate {
    device: PathBuf,
}

impl BusyPercentCandidate {
    pub(crate) fn new(device: PathBuf) -> Self {
        BusyPercentCandidate { device }
    }
}

impl Candidate for BusyPercentCandidate {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        let Ok(device) = self.device.canonicalize() else {
            return Err(Unavailable::NoDevice);
        };
        let path = device.join("gpu_busy_percent");
        if !path.exists() {
            return Err(Unavailable::NoDevice);
        }
        read_integer::<i64>(&path)?;
        Ok(Box::new(BusyPercent { path }))
    }
}

struct BusyPercent {
    path: PathBuf,
}

impl Source for BusyPercent {
    fn read(&mut self, _now: Instant) -> Result<Reading, Unavailable> {
        read_integer(&self.path).map(|percent| Reading::Value(Exact::whole(percent)))
    }

    fn describe(&self) -> String {
        format!("gpu_busy_percent, {}", self.path.display())
    }
}
