//! The LP360's hidraw adapter: finds the cooler by its USB ID under any node name, and writes
//! reports to its node (`docs/spec/protocol.md`, Device; the kernel's hidraw documentation: the
//! first byte of a `write()` is the report number).

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};

use odc_protocol::Report;

use crate::lp360::{Connector, OpenError, Transport};

/// The bus, vendor and product the kernel writes in `HID_ID=%04X:%08X:%08X`: USB, `3633:000c`.
const BUS_USB: u32 = 0x0003;
const VENDOR: u32 = 0x3633;
const PRODUCT: u32 = 0x000c;

/// Finds the LP360 by scanning `<sysfs>/class/hidraw/*/device/uevent` and opens `<dev>/<name>`.
/// Both roots are parameters, so the tests run the real code against fake trees.
#[derive(Clone, Debug)]
pub struct HidrawConnector {
    sysfs: PathBuf,
    dev: PathBuf,
}

impl HidrawConnector {
    pub fn new(sysfs: impl Into<PathBuf>, dev: impl Into<PathBuf>) -> Self {
        HidrawConnector {
            sysfs: sysfs.into(),
            dev: dev.into(),
        }
    }

    /// The system's own `/sys` and `/dev`.
    pub fn system() -> Self {
        HidrawConnector::new("/sys", "/dev")
    }

    /// The number of every hidraw node whose device is an LP360. Entries come and go during a
    /// scan, so one whose uevent can't be read is skipped.
    fn matching_nodes(&self) -> Result<Vec<u32>, OpenError> {
        let class = self.sysfs.join("class/hidraw");
        let entries = match fs::read_dir(&class) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(OpenError { path: class, error }),
        };
        let mut numbers = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(number) = name
                .to_str()
                .and_then(|n| n.strip_prefix("hidraw"))
                .and_then(|n| n.parse::<u32>().ok())
            else {
                continue;
            };
            let Ok(uevent) = fs::read_to_string(entry.path().join("device/uevent")) else {
                continue;
            };
            if hid_id(&uevent) == Some((BUS_USB, VENDOR, PRODUCT)) {
                numbers.push(number);
            }
        }
        Ok(numbers)
    }
}

/// The bus, vendor and product of a uevent's `HID_ID=` line, read as hex numbers, so letter case
/// can't matter.
fn hid_id(uevent: &str) -> Option<(u32, u32, u32)> {
    let value = uevent.lines().find_map(|l| l.strip_prefix("HID_ID="))?;
    let mut parts = value.split(':').map(|p| u32::from_str_radix(p, 16));
    let id = (
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    );
    parts.next().is_none().then_some(id)
}

/// Opens a node write-only. std opens every file with `O_CLOEXEC`; a test checks it.
fn open_node(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).open(path)
}

impl Connector for HidrawConnector {
    type Transport = HidrawTransport;

    /// The lowest-numbered LP360, compared as numbers so `hidraw9` comes before `hidraw10`
    /// (solution review P5-10). Several LP360s on one PC are out of scope.
    fn connect(&mut self) -> Result<Option<HidrawTransport>, OpenError> {
        let Some(number) = self.matching_nodes()?.into_iter().min() else {
            return Ok(None);
        };
        let path = self.dev.join(format!("hidraw{number}"));
        match open_node(&path) {
            Ok(file) => Ok(Some(HidrawTransport(file))),
            Err(error) => Err(OpenError { path, error }),
        }
    }
}

/// An open LP360 node.
#[derive(Debug)]
pub struct HidrawTransport(File);

impl Transport for HidrawTransport {
    /// One `write` of the 64 bytes. A short write is an error, and so is every error the kernel
    /// returns, `ENODEV` once the cooler is gone; `Lp360` then drops the connection.
    fn write_report(&mut self, report: &Report) -> io::Result<()> {
        let bytes = report.as_bytes();
        let written = self.0.write(bytes)?;
        if written != bytes.len() {
            return Err(io::Error::new(
                ErrorKind::WriteZero,
                format!("a short write: {written} of {} bytes", bytes.len()),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;

    #[test]
    fn hid_id_reads_the_kernel_format_in_either_case() {
        let uevent = "DRIVER=hid-generic\nHID_ID=0003:00003633:0000000C\nHID_NAME=x\n";
        assert_eq!(hid_id(uevent), Some((0x3, 0x3633, 0xc)));
        assert_eq!(
            hid_id("HID_ID=0003:00003633:0000000c"),
            Some((0x3, 0x3633, 0xc))
        );
        assert_eq!(hid_id("HID_NAME=x\n"), None);
        assert_eq!(hid_id("HID_ID=0003:00003633"), None);
        assert_eq!(hid_id("HID_ID=0003:00003633:0000000C:1"), None);
        assert_eq!(hid_id("HID_ID=0003:zz:0000000C"), None);
    }

    /// The node is opened write-only with close-on-exec: an open file's fdinfo flags, in octal,
    /// include `O_CLOEXEC` when it's set (proc(5)). The values are asm-generic/fcntl.h's.
    #[test]
    fn a_node_is_opened_write_only_with_close_on_exec() {
        let file = open_node(Path::new("/dev/null")).expect("/dev/null opens");
        let fdinfo = fs::read_to_string(format!("/proc/self/fdinfo/{}", file.as_raw_fd()))
            .expect("this process's fdinfo");
        let flags = fdinfo
            .lines()
            .find_map(|l| l.strip_prefix("flags:"))
            .map(|f| u32::from_str_radix(f.trim(), 8).expect("octal flags"))
            .expect("a flags line");
        const O_CLOEXEC: u32 = 1 << 19;
        const O_ACCMODE: u32 = 3;
        const O_WRONLY: u32 = 1;
        assert_eq!(flags & O_CLOEXEC, O_CLOEXEC, "flags {flags:o}");
        assert_eq!(flags & O_ACCMODE, O_WRONLY, "flags {flags:o}");
    }
}
