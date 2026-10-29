//! The GPU metrics of a GPU on NVIDIA's own driver, through NVIDIA's library, NVML, which is the
//! one piece of code inside the display service that isn't OpenDeepCool's. It's started only when
//! the chosen GPU's driver is `nvidia`, never on other PCs (owner, 2026-10-27; solution review
//! P6-06), and its failures name their cause (P6-12; `docs/spec/sensors.md`, Driver rules).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::error::NvmlError;

use crate::model::{Exact, Reading, Unavailable};
use crate::registry::{Candidate, Source};

/// The library NVML is loaded from, as `nvml-wrapper` loads it on Linux.
const LIBRARY: &str = "libnvidia-ml.so.1";

/// One of the three readings NVML gives the display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NvmlReading {
    /// The GPU sensor's temperature, whole °C.
    Temperature,
    /// The GPU's utilisation rate, whole percent.
    Usage,
    /// The power used, milliwatts.
    Power,
}

/// What the display service asks NVML. [`RealNvml`] is NVIDIA's library;
/// [`crate::fake::FakeNvml`] stands in for it in tests.
pub trait NvmlSource: Send {
    /// Starts NVML. Called when a GPU metric looks for its source, so a driver that loads late
    /// is found, and does nothing once started.
    fn start(&mut self) -> Result<(), Unavailable>;

    /// One reading of the GPU at `bus_id` (`domain:bus:device.function`).
    fn read(&mut self, bus_id: &str, reading: NvmlReading) -> Result<u32, Unavailable>;
}

/// NVIDIA's library through `nvml-wrapper`: loaded at the first start, never before.
#[derive(Default)]
pub struct RealNvml {
    nvml: Option<Nvml>,
}

impl RealNvml {
    pub fn new() -> Self {
        RealNvml { nvml: None }
    }
}

impl NvmlSource for RealNvml {
    fn start(&mut self) -> Result<(), Unavailable> {
        if self.nvml.is_none() {
            self.nvml = Some(Nvml::init().map_err(|e| start_reason(&e))?);
        }
        Ok(())
    }

    fn read(&mut self, bus_id: &str, reading: NvmlReading) -> Result<u32, Unavailable> {
        let nvml = self
            .nvml
            .as_ref()
            .ok_or_else(|| library_failed("NVML wasn't started".into()))?;
        let device = nvml
            .device_by_pci_bus_id(bus_id)
            .map_err(|e| read_reason(&e))?;
        match reading {
            NvmlReading::Temperature => device.temperature(TemperatureSensor::Gpu),
            NvmlReading::Usage => device.utilization_rates().map(|rates| rates.gpu),
            NvmlReading::Power => device.power_usage(),
        }
        .map_err(|e| read_reason(&e))
    }
}

fn library_failed(error: String) -> Unavailable {
    Unavailable::LibraryFailed {
        library: "NVML".into(),
        error,
    }
}

/// Why NVML couldn't start: the driver isn't loaded yet, so there's no device; the driver was
/// updated since boot; no permission; or the library can't be loaded or started, with the
/// loader's or NVML's own message, which tells a missing library from a refused one.
fn start_reason(error: &NvmlError) -> Unavailable {
    match error {
        NvmlError::DriverNotLoaded => Unavailable::NoDevice,
        NvmlError::LibRmVersionMismatch => Unavailable::DriverUpdated,
        NvmlError::NoPermission => Unavailable::PermissionDenied {
            path: PathBuf::from(LIBRARY),
        },
        other => library_failed(other.to_string()),
    }
}

/// Why a reading failed: the GPU isn't there, it doesn't give that reading, or as for a start.
fn read_reason(error: &NvmlError) -> Unavailable {
    match error {
        NvmlError::NotFound | NvmlError::GpuLost | NvmlError::DriverNotLoaded => {
            Unavailable::NoDevice
        }
        NvmlError::NotSupported => Unavailable::NoRule {
            driver: "nvidia".into(),
        },
        other => start_reason(other),
    }
}

/// One NVML reading of the chosen GPU, sharing the one NVML with the other two.
pub(crate) struct NvmlCandidate {
    reading: NvmlReading,
    bus_id: String,
    nvml: Arc<Mutex<Box<dyn NvmlSource>>>,
}

impl NvmlCandidate {
    pub(crate) fn new(
        reading: NvmlReading,
        bus_id: String,
        nvml: Arc<Mutex<Box<dyn NvmlSource>>>,
    ) -> Self {
        NvmlCandidate {
            reading,
            bus_id,
            nvml,
        }
    }
}

impl Candidate for NvmlCandidate {
    fn find(&self) -> Result<Box<dyn Source>, Unavailable> {
        let mut nvml = self.nvml.lock().unwrap_or_else(PoisonError::into_inner);
        nvml.start()?;
        nvml.read(&self.bus_id, self.reading)?;
        Ok(Box::new(NvmlValue {
            reading: self.reading,
            bus_id: self.bus_id.clone(),
            nvml: Arc::clone(&self.nvml),
        }))
    }
}

struct NvmlValue {
    reading: NvmlReading,
    bus_id: String,
    nvml: Arc<Mutex<Box<dyn NvmlSource>>>,
}

impl Source for NvmlValue {
    fn read(&mut self, _now: Instant) -> Result<Reading, Unavailable> {
        let mut nvml = self.nvml.lock().unwrap_or_else(PoisonError::into_inner);
        let n = i64::from(nvml.read(&self.bus_id, self.reading)?);
        let value = match self.reading {
            NvmlReading::Temperature | NvmlReading::Usage => Exact::whole(n),
            NvmlReading::Power => Exact::new(n, 1_000).unwrap_or(Exact::whole(0)),
        };
        Ok(Reading::Value(value))
    }

    fn describe(&self) -> String {
        format!("NVML {:?} of {}", self.reading, self.bus_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nvml_failures_name_their_cause() {
        assert_eq!(
            start_reason(&NvmlError::DriverNotLoaded),
            Unavailable::NoDevice
        );
        assert_eq!(
            start_reason(&NvmlError::LibRmVersionMismatch),
            Unavailable::DriverUpdated
        );
        assert_eq!(
            start_reason(&NvmlError::NoPermission),
            Unavailable::PermissionDenied {
                path: PathBuf::from(LIBRARY)
            }
        );
        let symbol = NvmlError::FailedToLoadSymbol("nvmlInit_v2: undefined symbol".into());
        assert_eq!(start_reason(&symbol), library_failed(symbol.to_string()));
        assert_eq!(
            start_reason(&NvmlError::Unknown),
            library_failed(NvmlError::Unknown.to_string())
        );
        for gone in [NvmlError::NotFound, NvmlError::GpuLost] {
            assert_eq!(read_reason(&gone), Unavailable::NoDevice);
        }
        assert_eq!(
            read_reason(&NvmlError::NotSupported),
            Unavailable::NoRule {
                driver: "nvidia".into()
            }
        );
        assert_eq!(
            read_reason(&NvmlError::LibRmVersionMismatch),
            Unavailable::DriverUpdated
        );
    }
}
