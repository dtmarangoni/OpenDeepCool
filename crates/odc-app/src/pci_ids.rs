//! The system's PCI ID list (`hwdata`'s `pci.ids`), for the GPUs' product names
//! (`docs/design/settings-app.md`, Configuration page: "The GPUs").
//!
//! Its format, from the file's own header: `vendor  vendor_name` lines, `\tdevice  device_name`
//! lines under their vendor (one tab), `\t\tsubvendor subdevice  subsystem_name` lines (two
//! tabs), `#` comments, and then the device classes, from the first `C xx` line.

use std::collections::HashMap;

/// Where Arch's `hwdata` installs the list.
pub const PATH: &str = "/usr/share/hwdata/pci.ids";

/// The device name of each of `ids` (vendor, device) that `text` lists; an ID it doesn't list,
/// or 0, which sysfs gives when it has none, has no entry.
pub fn names(text: &str, ids: &[(u16, u16)]) -> HashMap<(u16, u16), String> {
    let mut found = HashMap::new();
    let mut vendor = None;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if line.starts_with("C ") {
            break;
        }
        if line.starts_with("\t\t") {
            continue;
        }
        if let Some(device_line) = line.strip_prefix('\t') {
            let (Some(vendor), Some((device, name))) = (vendor, id_and_name(device_line)) else {
                continue;
            };
            if ids.contains(&(vendor, device)) && vendor != 0 && device != 0 {
                found
                    .entry((vendor, device))
                    .or_insert_with(|| name.to_owned());
            }
            continue;
        }
        vendor = id_and_name(line).map(|(id, _)| id);
    }
    found
}

/// The product a device name names: its last bracketed part when it ends with one, as in
/// "Chip 12 [Graphics R870]", or the whole name.
pub fn product(name: &str) -> &str {
    name.strip_suffix(']')
        .and_then(|rest| rest.rfind('[').map(|start| rest[start + 1..].trim()))
        .filter(|inside| !inside.is_empty())
        .unwrap_or(name)
}

/// `XXXX  name`: four hex digits, two spaces, the name.
fn id_and_name(line: &str) -> Option<(u16, &str)> {
    let (id, name) = line.split_once("  ")?;
    if id.len() != 4 {
        return None;
    }
    let id = u16::from_str_radix(id, 16).ok()?;
    let name = name.trim();
    (!name.is_empty()).then_some((id, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up list in the file's format: fictional vendors, devices and names.
    const FIXTURE: &str = "\
#
#\tList of PCI IDs (a made-up one)
#
# Syntax:
# vendor  vendor_name
#\tdevice  device_name\t\t\t\t<-- single tab
#\t\tsubvendor subdevice  subsystem_name\t<-- two tabs
0a0a  Contoso Ltd.
\t0001  Contoso Chip 12 [Graphics R870]
\t\t0a0a 0101  Contoso Graphics R870 Pro
\t0002  Contoso Bridge
# a comment between devices
\t00ff  Contoso Chip 14 [Graphics R990/R995]
0b0b  Fabrikam, Inc.
\t0001  Fabrikam Display Engine
\tzzzz  not a device line
C 03  Display controller
\t00  VGA compatible controller
";

    #[test]
    fn each_id_finds_its_own_vendor_s_device_line() {
        let ids = [(0x0a0a, 0x0001), (0x0a0a, 0x00ff), (0x0b0b, 0x0001)];
        let found = names(FIXTURE, &ids);
        assert_eq!(found.len(), 3);
        assert_eq!(found[&(0x0a0a, 0x0001)], "Contoso Chip 12 [Graphics R870]");
        assert_eq!(
            found[&(0x0a0a, 0x00ff)],
            "Contoso Chip 14 [Graphics R990/R995]"
        );
        // The same device ID under another vendor is another device.
        assert_eq!(found[&(0x0b0b, 0x0001)], "Fabrikam Display Engine");
    }

    // Unknown IDs, a subsystem line's IDs, the class section, a zero ID and a malformed line give
    // nothing.
    #[test]
    fn unknown_ids_and_other_lines_give_no_name() {
        let ids = [
            (0x0a0a, 0x0003),
            (0x0c0c, 0x0001),
            (0x0a0a, 0x0101),
            (0x0003, 0x0000),
            (0x0000, 0x0001),
            (0x0b0b, 0xffff),
        ];
        assert!(names(FIXTURE, &ids).is_empty());
        assert!(names("", &[(0x0a0a, 0x0001)]).is_empty());
        // A device line before any vendor line belongs to none.
        assert!(names("\t0001  Orphan\n", &[(0x0000, 0x0001)]).is_empty());
        // After a vendor line that doesn't parse, its devices belong to no vendor.
        let broken = "0a0a  Contoso Ltd.\nzz  Broken\n\t0001  Not Contoso's\n";
        assert!(names(broken, &[(0x0a0a, 0x0001)]).is_empty());
    }

    #[test]
    fn upper_case_hex_and_crlf_lines_read_alike() {
        let text = "0A0A  Contoso Ltd.\r\n\t00FF  Contoso Chip 14\r\n";
        assert_eq!(
            names(text, &[(0x0a0a, 0x00ff)])[&(0x0a0a, 0x00ff)],
            "Contoso Chip 14"
        );
    }

    #[test]
    fn a_product_is_the_last_bracketed_part_or_the_whole_name() {
        assert_eq!(product("Contoso Chip 12 [Graphics R870]"), "Graphics R870");
        assert_eq!(
            product("Contoso Chip 14 [Graphics R990/R995]"),
            "Graphics R990/R995"
        );
        assert_eq!(product("Chip [A] [Graphics B]"), "Graphics B");
        assert_eq!(
            product("Fabrikam Display Engine"),
            "Fabrikam Display Engine"
        );
        assert_eq!(product("Chip []"), "Chip []");
        assert_eq!(product("Chip [open"), "Chip [open");
        assert_eq!(product("Chip ]"), "Chip ]");
    }
}
