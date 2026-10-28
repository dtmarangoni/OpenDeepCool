//! `sysfs::usb_device` against a fake tree: the LP360 is found by its IDs among other devices,
//! in any letter case, and an interface or another product of the same vendor isn't it.

use std::fs;
use std::path::PathBuf;

use odc_testkit::sysfs::{LP360_PRODUCT, LP360_VENDOR, usb_device};

#[test]
fn the_lp360_is_found_by_its_ids_among_other_devices() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sysfs-usb");
    let _ = fs::remove_dir_all(&root);
    let devices = root.join("bus/usb/devices");
    let device = |name: &str, ids: Option<(&str, &str)>| {
        let dir = devices.join(name);
        fs::create_dir_all(&dir).expect("a fake device folder");
        if let Some((vendor, product)) = ids {
            fs::write(dir.join("idVendor"), format!("{vendor}\n")).expect("idVendor");
            fs::write(dir.join("idProduct"), format!("{product}\n")).expect("idProduct");
        }
    };
    assert_eq!(usb_device(&root, LP360_VENDOR, LP360_PRODUCT), None);
    device("usb1", Some(("1d6b", "0002")));
    device("1-2", Some(("3633", "000d")));
    device("1-6:1.0", None);
    assert_eq!(usb_device(&root, LP360_VENDOR, LP360_PRODUCT), None);
    device("1-6", Some(("3633", "000C")));
    assert_eq!(
        usb_device(&root, LP360_VENDOR, LP360_PRODUCT),
        Some(devices.join("1-6"))
    );
}
