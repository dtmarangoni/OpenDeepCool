//! Finding devices in sysfs the way the product does: by their IDs or their PCI class and driver,
//! never by index or port (AGENTS.md rule 12). Shared by the dev tool and the end-to-end program.

use std::fs;
use std::path::{Path, PathBuf};

/// The LP360's USB IDs, as sysfs writes them in `idVendor` and `idProduct`.
pub const LP360_VENDOR: &str = "3633";
pub const LP360_PRODUCT: &str = "000c";

/// The USB device folder under `<sysfs>/bus/usb/devices` whose `idVendor` and `idProduct` match,
/// the lowest name first if there are several. Interfaces and folders without IDs are passed
/// over.
pub fn usb_device(sysfs: &Path, vendor: &str, product: &str) -> Option<PathBuf> {
    let mut matches: Vec<PathBuf> = fs::read_dir(sysfs.join("bus/usb/devices"))
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|dir| {
            let id = |name| fs::read_to_string(dir.join(name)).map(|s| s.trim().to_lowercase());
            id("idVendor").is_ok_and(|v| v == vendor) && id("idProduct").is_ok_and(|p| p == product)
        })
        .collect();
    matches.sort();
    matches.into_iter().next()
}

/// Every PCI display controller with a driver under `<sysfs>/bus/pci/devices`, as (address,
/// driver), by address: the GPUs, as the product lists them (`docs/spec/sensors.md`, GPU choice).
/// A display controller is PCI base class 0x03 (the kernel's `pci_ids.h`,
/// `PCI_BASE_CLASS_DISPLAY`).
pub fn display_devices(sysfs: &Path) -> Vec<(String, String)> {
    let Ok(entries) = fs::read_dir(sysfs.join("bus/pci/devices")) else {
        return Vec::new();
    };
    let mut devices: Vec<(String, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let dir = entry.path();
            let class = fs::read_to_string(dir.join("class")).ok()?;
            let class = u32::from_str_radix(class.trim().trim_start_matches("0x"), 16).ok()?;
            if class >> 16 != 0x03 {
                return None;
            }
            let driver = fs::read_link(dir.join("driver")).ok()?;
            Some((
                entry.file_name().into_string().ok()?,
                driver.file_name()?.to_str()?.to_owned(),
            ))
        })
        .collect();
    devices.sort();
    devices
}
