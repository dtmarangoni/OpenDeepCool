//! Fake sysfs and `/dev` trees, for the tests and for the end-to-end program's fake mode. A
//! regular file stands in for each device node. Every name and ID is made up, except the LP360's
//! public USB ID (AGENTS.md rule 10).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::hardware::Roots;

/// The LP360's `HID_ID=` value, as the kernel writes it (`%04X:%08X:%08X`).
pub const LP360_HID_ID: &str = "0003:00003633:0000000C";

/// A fake `/sys` and `/dev` under one folder.
pub struct FakeTree {
    root: PathBuf,
}

impl FakeTree {
    /// An empty tree at `root`, replacing what was there.
    pub fn create(root: impl Into<PathBuf>) -> io::Result<FakeTree> {
        let root = root.into();
        match fs::remove_dir_all(&root) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        for dir in ["sys/class/hidraw", "sys/bus/usb/devices", "dev"] {
            fs::create_dir_all(root.join(dir))?;
        }
        Ok(FakeTree { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn roots(&self) -> Roots {
        Roots {
            sysfs: self.root.join("sys"),
            dev: self.root.join("dev"),
        }
    }

    /// A hidraw node whose device's uevent says `hid_id`, and its stand-in device file, which it
    /// returns.
    pub fn hidraw(&self, number: u32, hid_id: &str) -> io::Result<PathBuf> {
        let device = self
            .root
            .join(format!("sys/class/hidraw/hidraw{number}/device"));
        fs::create_dir_all(&device)?;
        fs::write(
            device.join("uevent"),
            format!("DRIVER=hid-generic\nHID_ID={hid_id}\nHID_NAME=Made-up Device\n"),
        )?;
        let node = self.root.join(format!("dev/hidraw{number}"));
        fs::write(&node, b"")?;
        Ok(node)
    }

    /// A USB device folder with its IDs and its `authorized` switch, which it returns.
    pub fn usb_device(
        &self,
        name: &str,
        vendor: &str,
        product: &str,
        authorized: &str,
    ) -> io::Result<PathBuf> {
        let dir = self.root.join("sys/bus/usb/devices").join(name);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("idVendor"), format!("{vendor}\n"))?;
        fs::write(dir.join("idProduct"), format!("{product}\n"))?;
        fs::write(dir.join("authorized"), authorized)?;
        Ok(dir)
    }
}
