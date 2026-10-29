//! Finding devices in sysfs the way the product does: by their IDs, never by index or port
//! (AGENTS.md rule 12). Shared by the dev tool and the end-to-end program.

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
