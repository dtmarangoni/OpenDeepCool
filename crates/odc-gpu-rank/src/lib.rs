//! `odc-gpu-rank`: a short-lived helper that asks each GPU's driver once whether the GPU is
//! dedicated or integrated, writes the ranking the display service reads (`gpu-rank.json` in its
//! state directory) and exits (`docs/spec/sensors.md`, GPU choice; owner's design, 2026-10-24).
//!
//! It's the only OpenDeepCool program with `unsafe` code and the only one that opens a GPU's
//! device nodes. All of that code is in two modules: `bindings`, generated from the kernel's
//! headers by `odc-bindings`, and `ffi`, the requests and NVIDIA's library (Phase 6 plan, Task 9).

mod bindings;
mod drivers;
pub mod fake;
mod ffi;

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use odc_sensors::{PciAddr, RANK_FILE, Ranked, RankedKind, Ranking, display_devices};

pub use drivers::DriverKinds;

/// A driver's documented signal for a GPU's kind (`docs/spec/sensors.md`, GPU choice).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// amdgpu: `AMDGPU_IDS_FLAGS_FUSION` in the device info's `ids_flags` marks an APU.
    AmdgpuFusion,
    /// xe: `DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM` in the configuration's flags marks a card.
    XeHasVram,
    /// i915: a memory region of class `I915_MEMORY_CLASS_DEVICE`, device-local memory, marks a
    /// card.
    I915DeviceMemory,
    /// NVIDIA's driver: CUDA's `CU_DEVICE_ATTRIBUTE_INTEGRATED` marks integrated graphics.
    CudaIntegrated,
}

impl Signal {
    /// The signal of GPUs on `driver`; `None` for a driver without one, such as nouveau.
    pub fn for_driver(driver: &str) -> Option<Signal> {
        match driver {
            "amdgpu" => Some(Signal::AmdgpuFusion),
            "xe" => Some(Signal::XeHasVram),
            "i915" => Some(Signal::I915DeviceMemory),
            "nvidia" => Some(Signal::CudaIntegrated),
            _ => None,
        }
    }

    /// What the ranking file records as the signal that gave `answer`.
    pub fn describe(self, answer: Answer) -> String {
        let dedicated = answer == Answer::Dedicated;
        let flag = |name: &str, set: bool| format!("{name} {}", if set { "set" } else { "clear" });
        match self {
            Signal::AmdgpuFusion => flag("AMDGPU_IDS_FLAGS_FUSION", !dedicated),
            Signal::XeHasVram => flag("DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM", dedicated),
            Signal::I915DeviceMemory => format!(
                "{} I915_MEMORY_CLASS_DEVICE region",
                if dedicated { "an" } else { "no" }
            ),
            Signal::CudaIntegrated => {
                format!("CU_DEVICE_ATTRIBUTE_INTEGRATED {}", u8::from(!dedicated))
            }
        }
    }
}

/// What a driver's signal said about a GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Dedicated,
    Integrated,
}

/// Where the kinds come from: the drivers ([`DriverKinds`]), or [`fake::FakeKinds`] in tests.
pub trait KindSource {
    /// Asks the driver of the GPU at `pci` for `signal`; an error names why the query failed.
    fn ask(&mut self, signal: Signal, pci: PciAddr) -> Result<Answer, String>;
}

/// Every PCI display controller with a driver under `sys`, by address, with its kind. A driver
/// without a signal is never asked; a failed query leaves the GPU unknown, with the error (owner,
/// 2026-10-27; solution review P6-07).
pub fn rank(sys: &Path, kinds: &mut dyn KindSource) -> Ranking {
    let gpus = display_devices(sys)
        .into_iter()
        .map(|(pci, driver)| {
            let kind = match Signal::for_driver(&driver) {
                None => RankedKind::NoSignal,
                Some(signal) => match kinds.ask(signal, pci) {
                    Ok(Answer::Dedicated) => RankedKind::Dedicated {
                        signal: signal.describe(Answer::Dedicated),
                    },
                    Ok(Answer::Integrated) => RankedKind::Integrated {
                        signal: signal.describe(Answer::Integrated),
                    },
                    Err(error) => RankedKind::QueryFailed { error },
                },
            };
            Ranked { pci, driver, kind }
        })
        .collect();
    Ranking { gpus }
}

/// Writes `ranking` to `gpu-rank.json` in `state_dir` atomically: a new file next to it, synced,
/// then renamed over it, so the display service never reads half a file. Returns its path.
pub fn write_ranking(state_dir: &Path, ranking: &Ranking) -> io::Result<PathBuf> {
    let path = state_dir.join(RANK_FILE);
    let new = state_dir.join(format!(".{RANK_FILE}.new"));
    // Left by a run that stopped before its rename.
    if let Err(e) = fs::remove_file(&new)
        && e.kind() != ErrorKind::NotFound
    {
        return Err(e);
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(&new)?;
    file.write_all(ranking.render().as_bytes())?;
    file.sync_all()?;
    fs::rename(&new, &path)?;
    File::open(state_dir)?.sync_all()?;
    Ok(path)
}
