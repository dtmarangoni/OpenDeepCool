//! [`DriverKinds`]: each GPU's kind, asked of its driver (`docs/spec/sensors.md`, GPU choice).
//! The DRM drivers answer on the GPU's render node, opened only once it's known to be the device
//! sysfs names for that GPU. NVIDIA's driver answers through CUDA, loaded at the first GPU on it.
//! The answers' layouts come from the generated structures.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::ErrorKind;
use std::mem::offset_of;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

use odc_sensors::PciAddr;

use crate::bindings::{
    AMDGPU_IDS_FLAGS_FUSION, DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM, DRM_XE_QUERY_CONFIG_FLAGS,
    I915_MEMORY_CLASS_DEVICE, drm_i915_gem_memory_class_instance, drm_i915_memory_region_info,
    drm_i915_query_memory_regions, drm_xe_query_config,
};
use crate::ffi::{self, Cuda};
use crate::{Answer, KindSource, Signal};

/// The drivers of the GPUs under the sysfs root `sys`, through the nodes under `dev`.
pub struct DriverKinds {
    sys: PathBuf,
    dev: PathBuf,
    /// CUDA, loaded at the first GPU on NVIDIA's driver, or why it couldn't be.
    cuda: Option<Result<Cuda, String>>,
}

impl DriverKinds {
    pub fn new(sys: &Path, dev: &Path) -> Self {
        DriverKinds {
            sys: sys.to_owned(),
            dev: dev.to_owned(),
            cuda: None,
        }
    }

    /// The GPU's render node (`<dev>/dri/renderDN`), opened for reading and writing once it's
    /// the character device that `drm/renderDN/dev` in the GPU's sysfs folder names, so no
    /// request reaches another file.
    fn render_node(&self, pci: PciAddr) -> Result<File, String> {
        let drm = self
            .sys
            .join("bus/pci/devices")
            .join(pci.to_string())
            .join("drm");
        let no_node = || format!("{}: no render node", drm.display());
        let entries = match fs::read_dir(&drm) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Err(no_node()),
            Err(e) => return Err(format!("{}: {e}", drm.display())),
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with("renderD"))
            .collect();
        names.sort();
        let name = names.first().ok_or_else(no_node)?;
        let numbers = drm.join(name).join("dev");
        let (major, minor) = fs::read_to_string(&numbers)
            .ok()
            .as_deref()
            .and_then(device_numbers)
            .ok_or_else(|| format!("{}: not a device number", numbers.display()))?;
        let path = self.dev.join("dri").join(name);
        let not_it = || {
            format!(
                "{} isn't the device {major}:{minor} that sysfs names",
                path.display()
            )
        };
        let before = fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !is_device(&before, major, minor) {
            return Err(not_it());
        }
        let node = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // Checked again on the open file, in case the path changed in between.
        let after = node
            .metadata()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if !is_device(&after, major, minor) {
            return Err(not_it());
        }
        Ok(node)
    }
}

impl KindSource for DriverKinds {
    fn ask(&mut self, signal: Signal, pci: PciAddr) -> Result<Answer, String> {
        let answer = |integrated: bool| {
            if integrated {
                Answer::Integrated
            } else {
                Answer::Dedicated
            }
        };
        match signal {
            Signal::AmdgpuFusion => {
                let info = ffi::amdgpu_device_info(&self.render_node(pci)?)
                    .map_err(|e| format!("DRM_IOCTL_AMDGPU_INFO: {e}"))?;
                Ok(answer(
                    info.ids_flags & u64::from(AMDGPU_IDS_FLAGS_FUSION) != 0,
                ))
            }
            Signal::XeHasVram => {
                let config = ffi::xe_query_config(&self.render_node(pci)?)
                    .map_err(|e| format!("DRM_IOCTL_XE_DEVICE_QUERY: {e}"))?;
                let flags = xe_flags(&config)?;
                Ok(answer(
                    flags & u64::from(DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM) == 0,
                ))
            }
            Signal::I915DeviceMemory => {
                let regions = ffi::i915_memory_regions(&self.render_node(pci)?)
                    .map_err(|e| format!("DRM_IOCTL_I915_QUERY: {e}"))?;
                let classes = i915_memory_classes(&regions)?;
                Ok(answer(!classes.contains(&I915_MEMORY_CLASS_DEVICE)))
            }
            Signal::CudaIntegrated => {
                let cuda = self
                    .cuda
                    .get_or_insert_with(Cuda::load)
                    .as_ref()
                    .map_err(Clone::clone)?;
                for (device, bus_id) in cuda.devices()? {
                    if cuda_bus_id(&bus_id) == Some(pci) {
                        return cuda.integrated(device).map(answer);
                    }
                }
                Err(format!("CUDA lists no device at {pci}"))
            }
        }
    }
}

/// `major:minor`, as a device's `dev` file in sysfs writes it.
fn device_numbers(text: &str) -> Option<(u32, u32)> {
    let (major, minor) = text.trim().split_once(':')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// Whether `meta` is the character device `major:minor`.
fn is_device(meta: &Metadata, major: u32, minor: u32) -> bool {
    meta.file_type().is_char_device() && meta.rdev() == libc::makedev(major, minor)
}

/// A PCI address as CUDA writes it, in either case; sysfs writes lower case.
fn cuda_bus_id(text: &str) -> Option<PciAddr> {
    text.to_ascii_lowercase().parse().ok()
}

/// The flags parameter of a `drm_xe_query_config` the kernel wrote (`xe_drm.h`).
fn xe_flags(config: &[u8]) -> Result<u64, String> {
    let params =
        read::<4>(config, offset_of!(drm_xe_query_config, num_params)).map(u32::from_ne_bytes)?;
    let index = DRM_XE_QUERY_CONFIG_FLAGS as usize;
    if index >= params as usize {
        return Err(format!(
            "xe's configuration has {params} parameters, none for its flags"
        ));
    }
    let at = offset_of!(drm_xe_query_config, info) + index * size_of::<u64>();
    read::<8>(config, at).map(u64::from_ne_bytes)
}

/// The memory class of each region in a `drm_i915_query_memory_regions` the kernel wrote
/// (`i915_drm.h`).
fn i915_memory_classes(answer: &[u8]) -> Result<Vec<u32>, String> {
    let count = read::<4>(
        answer,
        offset_of!(drm_i915_query_memory_regions, num_regions),
    )
    .map(u32::from_ne_bytes)?;
    let class = offset_of!(drm_i915_memory_region_info, region)
        + offset_of!(drm_i915_gem_memory_class_instance, memory_class);
    (0..count as usize)
        .map(|i| {
            let at = offset_of!(drm_i915_query_memory_regions, regions)
                + i * size_of::<drm_i915_memory_region_info>()
                + class;
            read::<2>(answer, at).map(|bytes| u32::from(u16::from_ne_bytes(bytes)))
        })
        .collect()
}

/// `N` bytes of `answer` from `at`.
fn read<const N: usize>(answer: &[u8], at: usize) -> Result<[u8; N], String> {
    answer
        .get(at..at + N)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| format!("the kernel's answer ends before byte {}", at + N))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::I915_MEMORY_CLASS_SYSTEM;

    #[test]
    fn device_numbers_read_as_sysfs_writes_them() {
        assert_eq!(device_numbers("226:128\n"), Some((226, 128)));
        for bad in ["", "226", "226:", ":128", "226:x", "-1:128"] {
            assert_eq!(device_numbers(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn cuda_bus_ids_match_in_either_case() {
        let pci: PciAddr = "0000:0a:00.0".parse().expect("an address");
        assert_eq!(cuda_bus_id("0000:0A:00.0"), Some(pci));
        assert_eq!(cuda_bus_id("00000000:0a:00.0"), Some(pci));
        assert_eq!(cuda_bus_id("GPU-0"), None);
    }

    /// A configuration as the kernel writes it: the count, padding, then the parameters.
    fn config(params: &[u64]) -> Vec<u8> {
        let mut bytes = (params.len() as u32).to_ne_bytes().to_vec();
        bytes.extend([0; 4]);
        for param in params {
            bytes.extend(param.to_ne_bytes());
        }
        bytes
    }

    #[test]
    fn xe_flags_are_the_second_parameter() {
        let has_vram = u64::from(DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM);
        assert_eq!(
            xe_flags(&config(&[0xe20b, has_vram, 4096, 48, 2])),
            Ok(has_vram)
        );
        assert_eq!(xe_flags(&config(&[0xe20b, 0, 4096, 48, 2])), Ok(0));
        assert!(xe_flags(&config(&[0xe20b])).is_err(), "no flags parameter");
        let mut cut = config(&[0xe20b, has_vram]);
        cut.truncate(12);
        assert!(xe_flags(&cut).is_err(), "cut short");
    }

    /// Memory regions as the kernel writes them, with each region's class.
    fn regions(classes: &[u32]) -> Vec<u8> {
        let mut bytes = vec![0; size_of::<drm_i915_query_memory_regions>()];
        bytes[..4].copy_from_slice(&(classes.len() as u32).to_ne_bytes());
        for &class in classes {
            let mut region = vec![0; size_of::<drm_i915_memory_region_info>()];
            region[..2].copy_from_slice(&(class as u16).to_ne_bytes());
            bytes.extend(region);
        }
        bytes
    }

    #[test]
    fn i915_regions_give_their_memory_classes() {
        assert_eq!(
            i915_memory_classes(&regions(&[
                I915_MEMORY_CLASS_SYSTEM,
                I915_MEMORY_CLASS_DEVICE
            ])),
            Ok(vec![I915_MEMORY_CLASS_SYSTEM, I915_MEMORY_CLASS_DEVICE])
        );
        assert_eq!(
            i915_memory_classes(&regions(&[I915_MEMORY_CLASS_SYSTEM])),
            Ok(vec![I915_MEMORY_CLASS_SYSTEM])
        );
        let mut cut = regions(&[I915_MEMORY_CLASS_SYSTEM, I915_MEMORY_CLASS_DEVICE]);
        cut.truncate(cut.len() - size_of::<drm_i915_memory_region_info>());
        assert!(i915_memory_classes(&cut).is_err(), "a region missing");
    }
}
