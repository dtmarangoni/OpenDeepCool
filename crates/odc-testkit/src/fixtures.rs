//! Fake sysfs and procfs trees for the sensor rules (Phase 6 plan, Task 3): one fixture per
//! driver and hardware case, each with the files its kernel documentation names, linked the way
//! sysfs links them.
//! - `class/hwmon/hwmonN`, `class/drm/cardN` and `class/drm/renderDN` lead to their folder under
//!   the parent device, whose `device` link points back to the parent.
//! - A PCI device has its `class` file, a `driver` link, and an entry under `bus/pci/devices`.
//! - `class/powercap/<zone>` leads to the zone's folder under its control type.
//!
//! Every name, address and value is made up (AGENTS.md rule 10). Each fixture gives each reading
//! its own value, so a rule that reads the wrong file reads the wrong number (rule 5).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::fake_tree::FakeTree;

/// The made-up name of the board's sensor chip in [`board_fans`] (AGENTS.md rule 10).
pub const BOARD_CHIP: &str = "fakeboardchip";

/// PCI class codes as sysfs writes them (`Documentation/ABI/testing/sysfs-bus-pci`, `class`;
/// the kernel's `pci_ids.h`): a VGA controller, a 3D controller and a host bridge.
pub const PCI_CLASS_VGA: &str = "0x030000";
pub const PCI_CLASS_3D: &str = "0x030200";
pub const PCI_CLASS_HOST_BRIDGE: &str = "0x060000";

/// A file's name and contents.
fn kv(name: impl Into<String>, contents: impl ToString) -> (String, String) {
    (name.into(), contents.to_string())
}

/// The lowest free number for `<prefix>N` among the entries of `dir`, as the kernel hands out
/// device numbers.
fn lowest_free(dir: &Path, prefix: &str) -> io::Result<u32> {
    let mut used = Vec::new();
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let name = entry?.file_name();
                if let Some(n) = name
                    .to_str()
                    .and_then(|n| n.strip_prefix(prefix))
                    .and_then(|n| n.parse::<u32>().ok())
                {
                    used.push(n);
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    Ok((0..).find(|n| !used.contains(n)).unwrap_or(u32::MAX))
}

/// The last component of a folder path.
fn last(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A PCI device at `addr`, with its `class`, a `driver` link when it has a driver, and its
/// `bus/pci/devices` entry. Returns its folder, relative to the tree's root.
pub fn pci_device(
    tree: &FakeTree,
    addr: &str,
    class: &str,
    driver: Option<&str>,
) -> io::Result<String> {
    let dir = format!("sys/devices/pci0000:00/{addr}");
    tree.file(format!("{dir}/class"), format!("{class}\n"))?;
    tree.symlink(
        format!("sys/bus/pci/devices/{addr}"),
        format!("../../../devices/pci0000:00/{addr}"),
    )?;
    if let Some(driver) = driver {
        tree.file(
            format!("sys/bus/pci/drivers/{driver}/uevent"),
            format!("DRIVER={driver}\n"),
        )?;
        tree.symlink(
            format!("{dir}/driver"),
            format!("../../../bus/pci/drivers/{driver}"),
        )?;
    }
    Ok(dir)
}

/// An hwmon device named `name` under the device folder `parent` (relative to the tree's root),
/// with `files` (`Documentation/hwmon/sysfs-interface.rst`). `class/hwmon/hwmonN`, with the
/// lowest free N, leads to it, and its `device` link points to `parent`. Returns its full path.
pub fn hwmon(
    tree: &FakeTree,
    parent: &str,
    name: &str,
    files: &[(String, String)],
) -> io::Result<PathBuf> {
    let n = lowest_free(&tree.root().join("sys/class/hwmon"), "hwmon")?;
    let dir = format!("{parent}/hwmon/hwmon{n}");
    tree.file(format!("{dir}/name"), format!("{name}\n"))?;
    for (file, contents) in files {
        tree.file(format!("{dir}/{file}"), format!("{contents}\n"))?;
    }
    tree.symlink(
        format!("{dir}/device"),
        format!("../../../{}", last(parent)),
    )?;
    let under_sys = dir.strip_prefix("sys/").unwrap_or(&dir);
    tree.symlink(
        format!("sys/class/hwmon/hwmon{n}"),
        format!("../../{under_sys}"),
    )?;
    Ok(tree.root().join(dir))
}

/// A DRM card for the GPU folder `gpu` (relative to the tree's root): `class/drm/cardN`, with
/// the lowest free N, leads to it, and its `device` link points to the GPU. Returns its full
/// path.
pub fn drm_card(tree: &FakeTree, gpu: &str) -> io::Result<PathBuf> {
    let n = lowest_free(&tree.root().join("sys/class/drm"), "card")?;
    let dir = format!("{gpu}/drm/card{n}");
    tree.symlink(format!("{dir}/device"), format!("../../../{}", last(gpu)))?;
    let under_sys = dir.strip_prefix("sys/").unwrap_or(&dir);
    tree.symlink(
        format!("sys/class/drm/card{n}"),
        format!("../../{under_sys}"),
    )?;
    Ok(tree.root().join(dir))
}

/// The render node of the GPU at `pci`: `drm/renderD<minor>` in the GPU's folder, whose `dev`
/// file holds its device numbers (major 226, the DRM devices' in the kernel's
/// `Documentation/admin-guide/devices.txt`), its `class/drm` entry, and the `dev/dri` stand-in,
/// a regular file, which it returns. A real node is a character device, so a helper that checks
/// the node it opens refuses the stand-in.
pub fn render_node(tree: &FakeTree, pci: &str, minor: u32) -> io::Result<PathBuf> {
    let dir = format!("sys/devices/pci0000:00/{pci}/drm/renderD{minor}");
    tree.file(format!("{dir}/dev"), format!("226:{minor}\n"))?;
    tree.symlink(format!("{dir}/device"), format!("../../../{pci}"))?;
    tree.symlink(
        format!("sys/class/drm/renderD{minor}"),
        format!("../../devices/pci0000:00/{pci}/drm/renderD{minor}"),
    )?;
    tree.file(format!("dev/dri/renderD{minor}"), "")
}

/// The Intel CPU temperature driver (`Documentation/hwmon/coretemp.rst`), a platform device:
/// the package's channel, labelled `Package id 0`, and two cores' channels, in millidegrees C.
/// Returns the hwmon folder.
pub fn coretemp(tree: &FakeTree, package_millic: i64) -> io::Result<PathBuf> {
    hwmon(
        tree,
        "sys/devices/platform/coretemp.0",
        "coretemp",
        &[
            kv("temp1_label", "Package id 0"),
            kv("temp1_input", package_millic),
            kv("temp2_label", "Core 0"),
            kv("temp2_input", package_millic - 4_000),
            kv("temp3_label", "Core 1"),
            kv("temp3_input", package_millic - 6_000),
        ],
    )
}

/// The PCI function the AMD CPU temperature driver binds to, a host bridge.
fn k10temp_device(tree: &FakeTree) -> io::Result<String> {
    pci_device(tree, "0000:00:18.3", PCI_CLASS_HOST_BRIDGE, Some("k10temp"))
}

/// `/proc/cpuinfo` for two logical CPUs of AMD's CPU family `family` (proc_cpuinfo(5)), which
/// the k10temp rule reads.
fn proc_cpuinfo(tree: &FakeTree, family: u32) -> io::Result<PathBuf> {
    let cpu = |n: u32| {
        format!(
            "processor\t: {n}\nvendor_id\t: AuthenticAMD\ncpu family\t: {family}\nmodel\t\t: 1\n\n"
        )
    };
    tree.file("proc/cpuinfo", format!("{}{}", cpu(0), cpu(1)))
}

/// The AMD CPU temperature driver (`Documentation/hwmon/k10temp.rst`) on CPU family 19h:
/// `Tctl`, `Tdie` when given, and one CCD's channel, in millidegrees C. Returns the hwmon folder.
pub fn k10temp(tree: &FakeTree, tctl_millic: i64, tdie_millic: Option<i64>) -> io::Result<PathBuf> {
    let parent = k10temp_device(tree)?;
    proc_cpuinfo(tree, 0x19)?;
    let mut files = vec![kv("temp1_label", "Tctl"), kv("temp1_input", tctl_millic)];
    if let Some(tdie) = tdie_millic {
        files.extend([kv("temp2_label", "Tdie"), kv("temp2_input", tdie)]);
    }
    files.extend([
        kv("temp3_label", "Tccd1"),
        kv("temp3_input", tctl_millic - 3_000),
    ]);
    hwmon(tree, &parent, "k10temp", &files)
}

/// k10temp on CPU family 15h, older than 17h, where `Tctl` isn't a temperature (the k10temp
/// documentation). Returns the hwmon folder.
pub fn k10temp_pre17h(tree: &FakeTree) -> io::Result<PathBuf> {
    let parent = k10temp_device(tree)?;
    proc_cpuinfo(tree, 0x15)?;
    hwmon(
        tree,
        &parent,
        "k10temp",
        &[kv("temp1_label", "Tctl"), kv("temp1_input", 45_000)],
    )
}

/// The amdgpu files of one GPU (`Documentation/gpu/amdgpu/thermal.rst`): `gpu_busy_percent` on
/// the PCI device, and an hwmon device with the given temperatures `(label, millidegrees)`, the
/// power file `(name, microwatts)` and, on a card, its own fan.
fn amdgpu(
    tree: &FakeTree,
    pci: &str,
    temperatures: &[(&str, i64)],
    busy_percent: u32,
    power: (&str, u64),
    fan_rpm: Option<u64>,
) -> io::Result<PathBuf> {
    let gpu = pci_device(tree, pci, PCI_CLASS_VGA, Some("amdgpu"))?;
    tree.file(
        format!("{gpu}/gpu_busy_percent"),
        format!("{busy_percent}\n"),
    )?;
    let mut files = Vec::new();
    for (i, (label, millic)) in temperatures.iter().enumerate() {
        files.extend([
            kv(format!("temp{}_label", i + 1), label),
            kv(format!("temp{}_input", i + 1), millic),
        ]);
    }
    files.push(kv(power.0, power.1));
    if let Some(rpm) = fan_rpm {
        files.push(kv("fan1_input", rpm));
    }
    hwmon(tree, &gpu, "amdgpu", &files)?;
    drm_card(tree, &gpu)?;
    Ok(tree.root().join(gpu))
}

/// An AMD graphics card: `edge`, `junction` (edge + 36 °C) and `mem` (edge + 25 °C)
/// temperatures, `power1_average`, the card's own fan, and the busy percentage. Returns the PCI
/// device's folder.
pub fn amdgpu_dgpu(
    tree: &FakeTree,
    pci: &str,
    edge_millic: i64,
    busy_percent: u32,
    power_average_uw: u64,
) -> io::Result<PathBuf> {
    amdgpu(
        tree,
        pci,
        &[
            ("edge", edge_millic),
            ("junction", edge_millic + 36_000),
            ("mem", edge_millic + 25_000),
        ],
        busy_percent,
        ("power1_average", power_average_uw),
        Some(1_200),
    )
}

/// AMD integrated graphics: only the `edge` temperature, `power1_average`, no fan of its own.
/// Returns the PCI device's folder.
pub fn amdgpu_apu(
    tree: &FakeTree,
    pci: &str,
    edge_millic: i64,
    busy_percent: u32,
    power_average_uw: u64,
) -> io::Result<PathBuf> {
    amdgpu(
        tree,
        pci,
        &[("edge", edge_millic)],
        busy_percent,
        ("power1_average", power_average_uw),
        None,
    )
}

/// An AMD graphics card that has `power1_input`, instantaneous power, and no `power1_average`.
/// Returns the PCI device's folder.
pub fn amdgpu_power_input_only(
    tree: &FakeTree,
    pci: &str,
    edge_millic: i64,
    busy_percent: u32,
    power_input_uw: u64,
) -> io::Result<PathBuf> {
    amdgpu(
        tree,
        pci,
        &[
            ("edge", edge_millic),
            ("junction", edge_millic + 36_000),
            ("mem", edge_millic + 25_000),
        ],
        busy_percent,
        ("power1_input", power_input_uw),
        Some(1_200),
    )
}

/// Intel graphics on the i915 driver, on a platform with its hwmon
/// (`Documentation/ABI/testing/sysfs-driver-intel-i915-hwmon`): `temp1_input` in millidegrees C
/// and `energy1_input` in microjoules. Returns the PCI device's folder.
pub fn i915(tree: &FakeTree, pci: &str, temp_millic: i64, energy_uj: u64) -> io::Result<PathBuf> {
    let gpu = pci_device(tree, pci, PCI_CLASS_VGA, Some("i915"))?;
    hwmon(
        tree,
        &gpu,
        "i915",
        &[
            kv("temp1_input", temp_millic),
            kv("energy1_input", energy_uj),
        ],
    )?;
    drm_card(tree, &gpu)?;
    Ok(tree.root().join(gpu))
}

/// Intel graphics on the xe driver (`Documentation/ABI/testing/sysfs-driver-intel-xe-hwmon`):
/// `temp2_input`, the package temperature in millidegrees C, and `energy1_input`, the card's
/// energy in microjoules. Returns the PCI device's folder.
pub fn xe(tree: &FakeTree, pci: &str, temp2_millic: i64, energy1_uj: u64) -> io::Result<PathBuf> {
    let gpu = pci_device(tree, pci, PCI_CLASS_VGA, Some("xe"))?;
    hwmon(
        tree,
        &gpu,
        "xe",
        &[
            kv("temp2_input", temp2_millic),
            kv("energy1_input", energy1_uj),
        ],
    )?;
    drm_card(tree, &gpu)?;
    Ok(tree.root().join(gpu))
}

/// A GPU on NVIDIA's proprietary driver, which has no hwmon interface: only the PCI device, a 3D
/// controller, to show that the display base class alone counts. Returns its folder.
pub fn nvidia_proprietary(tree: &FakeTree, pci: &str) -> io::Result<PathBuf> {
    let gpu = pci_device(tree, pci, PCI_CLASS_3D, Some("nvidia"))?;
    Ok(tree.root().join(gpu))
}

/// A GPU on nouveau, whose hwmon readings no kernel document describes. It still has a
/// temperature and a fan, which no rule may read. Returns the PCI device's folder.
pub fn nouveau(tree: &FakeTree, pci: &str) -> io::Result<PathBuf> {
    let gpu = pci_device(tree, pci, PCI_CLASS_VGA, Some("nouveau"))?;
    hwmon(
        tree,
        &gpu,
        "nouveau",
        &[kv("temp1_input", 52_000), kv("fan1_input", 1_500)],
    )?;
    drm_card(tree, &gpu)?;
    Ok(tree.root().join(gpu))
}

/// A powercap control type: its folder, which has no `name`, and its `class/powercap` entry
/// (`Documentation/power/powercap/powercap.rst`).
fn control_type(tree: &FakeTree, name: &str) -> io::Result<()> {
    tree.file(
        format!("sys/devices/virtual/powercap/{name}/enabled"),
        "1\n",
    )?;
    tree.symlink(
        format!("sys/class/powercap/{name}"),
        format!("../../devices/virtual/powercap/{name}"),
    )?;
    Ok(())
}

/// One powercap zone under `control`. A sub-zone, whose id has two numbers (`intel-rapl:0:0`),
/// sits in its parent zone's folder.
fn zone(
    tree: &FakeTree,
    control: &str,
    id: &str,
    name: &str,
    energy_uj: u64,
    max_energy_range_uj: u64,
) -> io::Result<()> {
    let path = match id.rsplit_once(':') {
        Some((parent, _)) if parent.contains(':') => format!("{control}/{parent}/{id}"),
        _ => format!("{control}/{id}"),
    };
    let dir = format!("sys/devices/virtual/powercap/{path}");
    tree.file(format!("{dir}/name"), format!("{name}\n"))?;
    tree.file(format!("{dir}/energy_uj"), format!("{energy_uj}\n"))?;
    tree.file(
        format!("{dir}/max_energy_range_uj"),
        format!("{max_energy_range_uj}\n"),
    )?;
    tree.file(format!("{dir}/enabled"), "1\n")?;
    tree.symlink(
        format!("sys/class/powercap/{id}"),
        format!("../../devices/virtual/powercap/{path}"),
    )?;
    Ok(())
}

/// RAPL zones (`Documentation/power/powercap/powercap.rst`): the control type `intel-rapl`, and
/// each zone `(id, name, energy_uj, max_energy_range_uj)`, for example
/// `("intel-rapl:0", "package-0", …)` or the sub-zone `("intel-rapl:0:0", "core", …)`.
pub fn rapl(tree: &FakeTree, zones: &[(&str, &str, u64, u64)]) -> io::Result<()> {
    control_type(tree, "intel-rapl")?;
    for &(id, name, energy_uj, max) in zones {
        zone(tree, "intel-rapl", id, name, energy_uj, max)?;
    }
    Ok(())
}

/// The MMIO RAPL control type with one zone, `intel-rapl-mmio:0`, also named `package-0`, which
/// the CPU power rule must not count.
pub fn rapl_mmio(tree: &FakeTree, energy_uj: u64, max_energy_range_uj: u64) -> io::Result<()> {
    control_type(tree, "intel-rapl-mmio")?;
    zone(
        tree,
        "intel-rapl-mmio",
        "intel-rapl-mmio:0",
        "package-0",
        energy_uj,
        max_energy_range_uj,
    )
}

/// `/proc/stat` with `text` (proc_stat(5)), whose first line is the aggregate `cpu` line.
pub fn proc_stat(tree: &FakeTree, text: &str) -> io::Result<PathBuf> {
    tree.file("proc/stat", text)
}

/// `/proc/meminfo` (proc_meminfo(5)): `MemTotal` and `MemAvailable` in kB, among other lines,
/// aligned as the kernel writes them.
pub fn proc_meminfo(tree: &FakeTree, total_kb: u64, available_kb: u64) -> io::Result<PathBuf> {
    let line = |key: &str, kb: u64| format!("{:<16}{kb:>8} kB\n", format!("{key}:"));
    let text = [
        line("MemTotal", total_kb),
        line("MemFree", available_kb / 2),
        line("MemAvailable", available_kb),
        line("Buffers", 2_048),
        line("Cached", 4_096_000),
    ]
    .concat();
    tree.file("proc/meminfo", text)
}

/// The board's sensor chip, a platform device named `chip` (usually [`BOARD_CHIP`]), with fan
/// inputs `(N, rpm, label)` (`Documentation/hwmon/sysfs-interface.rst`, `fanN_input` and
/// `fanN_label`). Returns the hwmon folder.
pub fn board_fans(
    tree: &FakeTree,
    chip: &str,
    inputs: &[(u32, u64, Option<&str>)],
) -> io::Result<PathBuf> {
    let mut files = Vec::new();
    for &(n, rpm, label) in inputs {
        files.push(kv(format!("fan{n}_input"), rpm));
        if let Some(label) = label {
            files.push(kv(format!("fan{n}_label"), label));
        }
    }
    hwmon(
        tree,
        &format!("sys/devices/platform/{chip}.656"),
        chip,
        &files,
    )
}
