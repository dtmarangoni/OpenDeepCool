//! The sensor fixtures (Phase 6 plan, Task 3): each lists the files its kernel document names,
//! linked as sysfs links them, and the fake tree's own methods stay inside the tree.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{
    BOARD_CHIP, PCI_CLASS_3D, PCI_CLASS_HOST_BRIDGE, PCI_CLASS_VGA, amdgpu_apu, amdgpu_dgpu,
    amdgpu_power_input_only, board_fans, coretemp, i915, k10temp, k10temp_pre17h, nouveau,
    nvidia_proprietary, proc_meminfo, proc_stat, rapl, rapl_mmio, xe,
};

fn tree(name: &str) -> FakeTree {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("fixtures")
        .join(name);
    FakeTree::create(root).expect("a fake tree")
}

/// Every path in the tree under `sys/` and `proc/`, relative to the root, a link shown with its
/// target as `path -> target`, sorted.
fn listing(tree: &FakeTree) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let shown = path
                .strip_prefix(root)
                .expect("inside the root")
                .display()
                .to_string();
            let meta = fs::symlink_metadata(&path).expect("an entry");
            if meta.file_type().is_symlink() {
                let target = fs::read_link(&path).expect("a link");
                out.push(format!("{shown} -> {}", target.display()));
            } else if meta.is_dir() {
                walk(root, &path, out);
            } else {
                out.push(shown);
            }
        }
    }
    let mut out = Vec::new();
    for top in ["sys", "proc"] {
        walk(tree.root(), &tree.root().join(top), &mut out);
    }
    out.sort();
    out
}

fn assert_lists(tree: &FakeTree, expected: &[&str]) {
    let listing = listing(tree);
    for path in expected {
        assert!(
            listing.iter().any(|p| p == path),
            "{path} is missing from:\n{}",
            listing.join("\n")
        );
    }
}

fn read(tree: &FakeTree, path: &str) -> String {
    fs::read_to_string(tree.root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Where `path`'s links lead, relative to the root.
fn resolved(tree: &FakeTree, path: &str) -> String {
    let root = tree.root().canonicalize().expect("the root");
    let full = tree
        .root()
        .join(path)
        .canonicalize()
        .unwrap_or_else(|e| panic!("{path}: {e}"));
    full.strip_prefix(root)
        .expect("inside the root")
        .display()
        .to_string()
}

#[test]
fn coretemp_is_a_platform_device_with_its_package_and_cores() {
    let t = tree("coretemp");
    coretemp(&t, 57_000).expect("coretemp");
    assert_lists(
        &t,
        &[
            "sys/class/hwmon/hwmon0 -> ../../devices/platform/coretemp.0/hwmon/hwmon0",
            "sys/devices/platform/coretemp.0/hwmon/hwmon0/device -> ../../../coretemp.0",
            "sys/devices/platform/coretemp.0/hwmon/hwmon0/name",
            "sys/devices/platform/coretemp.0/hwmon/hwmon0/temp1_label",
            "sys/devices/platform/coretemp.0/hwmon/hwmon0/temp1_input",
            "sys/devices/platform/coretemp.0/hwmon/hwmon0/temp2_input",
        ],
    );
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/name"), "coretemp\n");
    assert_eq!(
        read(&t, "sys/class/hwmon/hwmon0/temp1_label"),
        "Package id 0\n"
    );
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/temp1_input"), "57000\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/temp2_input"), "53000\n");
    assert_eq!(
        resolved(&t, "sys/class/hwmon/hwmon0/device"),
        "sys/devices/platform/coretemp.0"
    );
    assert!(
        !t.root()
            .join("sys/devices/platform/coretemp.0/class")
            .exists()
    );
}

#[test]
fn k10temp_hangs_off_a_host_bridge_and_says_its_cpu_family() {
    let t = tree("k10temp");
    k10temp(&t, 61_000, Some(51_000)).expect("k10temp");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/name"), "k10temp\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/temp1_label"), "Tctl\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/temp1_input"), "61000\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/temp2_label"), "Tdie\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/temp2_input"), "51000\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/temp3_label"), "Tccd1\n");
    assert_eq!(
        read(&t, "sys/class/hwmon/hwmon0/device/class"),
        format!("{PCI_CLASS_HOST_BRIDGE}\n")
    );
    assert_eq!(
        resolved(&t, "sys/class/hwmon/hwmon0/device/driver"),
        "sys/bus/pci/drivers/k10temp"
    );
    assert!(read(&t, "proc/cpuinfo").contains("cpu family\t: 25\n"));

    let only_tctl = tree("k10temp-tctl");
    k10temp(&only_tctl, 61_000, None).expect("k10temp");
    assert!(
        !only_tctl
            .root()
            .join("sys/class/hwmon/hwmon0/temp2_label")
            .exists()
    );

    let old = tree("k10temp-pre17h");
    k10temp_pre17h(&old).expect("k10temp");
    assert!(read(&old, "proc/cpuinfo").contains("cpu family\t: 21\n"));
}

#[test]
fn an_amd_card_has_three_distinct_temperatures_its_power_its_fan_and_its_busy_share() {
    let t = tree("amdgpu");
    amdgpu_dgpu(&t, "0000:0a:00.0", 41_000, 37, 182_000_000).expect("amdgpu");
    let gpu = "sys/devices/pci0000:00/0000:0a:00.0";
    assert_lists(
        &t,
        &[
            "sys/bus/pci/devices/0000:0a:00.0 -> ../../../devices/pci0000:00/0000:0a:00.0",
            "sys/class/drm/card0 -> ../../devices/pci0000:00/0000:0a:00.0/drm/card0",
            "sys/class/hwmon/hwmon0 -> ../../devices/pci0000:00/0000:0a:00.0/hwmon/hwmon0",
            "sys/devices/pci0000:00/0000:0a:00.0/class",
            "sys/devices/pci0000:00/0000:0a:00.0/driver -> ../../../bus/pci/drivers/amdgpu",
            "sys/devices/pci0000:00/0000:0a:00.0/drm/card0/device -> ../../../0000:0a:00.0",
            "sys/devices/pci0000:00/0000:0a:00.0/gpu_busy_percent",
            "sys/devices/pci0000:00/0000:0a:00.0/hwmon/hwmon0/device -> ../../../0000:0a:00.0",
            "sys/devices/pci0000:00/0000:0a:00.0/hwmon/hwmon0/fan1_input",
            "sys/devices/pci0000:00/0000:0a:00.0/hwmon/hwmon0/power1_average",
        ],
    );
    let hwmon = "sys/class/hwmon/hwmon0";
    let temps: Vec<(String, String)> = (1..=3)
        .map(|n| {
            (
                read(&t, &format!("{hwmon}/temp{n}_label")),
                read(&t, &format!("{hwmon}/temp{n}_input")),
            )
        })
        .collect();
    assert_eq!(
        temps,
        [
            ("edge\n".to_owned(), "41000\n".to_owned()),
            ("junction\n".to_owned(), "77000\n".to_owned()),
            ("mem\n".to_owned(), "66000\n".to_owned()),
        ]
    );
    assert_eq!(read(&t, &format!("{hwmon}/power1_average")), "182000000\n");
    assert_eq!(read(&t, &format!("{gpu}/gpu_busy_percent")), "37\n");
    assert_eq!(
        read(&t, &format!("{gpu}/class")),
        format!("{PCI_CLASS_VGA}\n")
    );
    for link in [
        "sys/bus/pci/devices/0000:0a:00.0",
        "sys/class/drm/card0/device",
        "sys/class/hwmon/hwmon0/device",
    ] {
        assert_eq!(resolved(&t, link), gpu, "{link}");
    }
}

#[test]
fn amd_integrated_graphics_and_a_card_with_only_power1_input_differ_where_they_should() {
    let apu = tree("amdgpu-apu");
    amdgpu_apu(&apu, "0000:0c:00.0", 48_000, 12, 9_000_000).expect("apu");
    let hwmon = apu.root().join("sys/class/hwmon/hwmon0");
    assert!(hwmon.join("temp1_label").exists() && !hwmon.join("temp2_label").exists());
    assert!(hwmon.join("power1_average").exists() && !hwmon.join("fan1_input").exists());

    let input = tree("amdgpu-power-input");
    amdgpu_power_input_only(&input, "0000:0a:00.0", 41_000, 37, 150_000_000).expect("card");
    let hwmon = input.root().join("sys/class/hwmon/hwmon0");
    assert!(hwmon.join("power1_input").exists() && !hwmon.join("power1_average").exists());
    assert_eq!(
        read(&input, "sys/class/hwmon/hwmon0/power1_input"),
        "150000000\n"
    );
}

#[test]
fn intel_graphics_have_their_hwmon_files_and_no_busy_share() {
    let t = tree("intel");
    i915(&t, "0000:00:02.0", 44_000, 9_100_000).expect("i915");
    xe(&t, "0000:0b:00.0", 47_000, 7_300_000).expect("xe");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/name"), "i915\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/temp1_input"), "44000\n");
    assert_eq!(
        read(&t, "sys/class/hwmon/hwmon0/energy1_input"),
        "9100000\n"
    );
    assert_eq!(read(&t, "sys/class/hwmon/hwmon1/name"), "xe\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon1/temp2_input"), "47000\n");
    assert_eq!(
        read(&t, "sys/class/hwmon/hwmon1/energy1_input"),
        "7300000\n"
    );
    assert_eq!(
        resolved(&t, "sys/class/drm/card1/device"),
        "sys/devices/pci0000:00/0000:0b:00.0"
    );
    for gpu in ["0000:00:02.0", "0000:0b:00.0"] {
        let dir = t.root().join("sys/devices/pci0000:00").join(gpu);
        assert!(!dir.join("gpu_busy_percent").exists(), "{gpu}");
    }
}

#[test]
fn nvidia_has_no_hwmon_and_nouveau_has_readings_no_rule_may_use() {
    let t = tree("nvidia-nouveau");
    nvidia_proprietary(&t, "0000:0d:00.0").expect("nvidia");
    nouveau(&t, "0000:0e:00.0").expect("nouveau");
    let nvidia = "sys/devices/pci0000:00/0000:0d:00.0";
    assert_eq!(
        read(&t, &format!("{nvidia}/class")),
        format!("{PCI_CLASS_3D}\n")
    );
    assert_eq!(
        resolved(&t, &format!("{nvidia}/driver")),
        "sys/bus/pci/drivers/nvidia"
    );
    assert!(!t.root().join(nvidia).join("hwmon").exists());
    assert!(!t.root().join(nvidia).join("drm").exists());
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/name"), "nouveau\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/fan1_input"), "1500\n");
    assert_eq!(
        resolved(&t, "sys/class/drm/card0/device"),
        "sys/devices/pci0000:00/0000:0e:00.0"
    );
}

#[test]
fn rapl_zones_sit_under_their_control_type_and_the_control_type_has_no_name() {
    let t = tree("rapl");
    rapl(
        &t,
        &[
            ("intel-rapl:0", "package-0", 7_000_000, 262_143_328_850),
            ("intel-rapl:0:0", "core", 5_000_000, 262_143_328_850),
            ("intel-rapl:1", "package-1", 6_000_000, 262_143_328_850),
        ],
    )
    .expect("rapl");
    rapl_mmio(&t, 9_000_000, 262_143_328_850).expect("mmio");
    assert_lists(
        &t,
        &[
            "sys/class/powercap/intel-rapl -> ../../devices/virtual/powercap/intel-rapl",
            "sys/class/powercap/intel-rapl:0 -> ../../devices/virtual/powercap/intel-rapl/intel-rapl:0",
            "sys/class/powercap/intel-rapl:0:0 -> ../../devices/virtual/powercap/intel-rapl/intel-rapl:0/intel-rapl:0:0",
            "sys/class/powercap/intel-rapl:1 -> ../../devices/virtual/powercap/intel-rapl/intel-rapl:1",
            "sys/class/powercap/intel-rapl-mmio:0 -> ../../devices/virtual/powercap/intel-rapl-mmio/intel-rapl-mmio:0",
            "sys/devices/virtual/powercap/intel-rapl/enabled",
            "sys/devices/virtual/powercap/intel-rapl/intel-rapl:0/energy_uj",
            "sys/devices/virtual/powercap/intel-rapl/intel-rapl:0/max_energy_range_uj",
        ],
    );
    assert!(!t.root().join("sys/class/powercap/intel-rapl/name").exists());
    assert_eq!(
        read(&t, "sys/class/powercap/intel-rapl:0/name"),
        "package-0\n"
    );
    assert_eq!(
        read(&t, "sys/class/powercap/intel-rapl:0/energy_uj"),
        "7000000\n"
    );
    assert_eq!(read(&t, "sys/class/powercap/intel-rapl:0:0/name"), "core\n");
    assert_eq!(
        read(&t, "sys/class/powercap/intel-rapl-mmio:0/name"),
        "package-0\n"
    );
}

#[test]
fn proc_files_carry_what_they_were_given() {
    let t = tree("proc");
    let stat = "cpu  100 2 30 4000 50 6 7 8 0 0\ncpu0 50 1 15 2000 25 3 3 4 0 0\n";
    proc_stat(&t, stat).expect("stat");
    proc_meminfo(&t, 31_000_000, 7_750_000).expect("meminfo");
    assert_eq!(read(&t, "proc/stat"), stat);
    let meminfo = read(&t, "proc/meminfo");
    assert!(
        meminfo.starts_with("MemTotal:       31000000 kB\n"),
        "{meminfo}"
    );
    assert!(
        meminfo.contains("\nMemAvailable:    7750000 kB\n"),
        "{meminfo}"
    );
}

#[test]
fn the_board_chip_is_a_platform_device_with_its_fans_and_labels() {
    let t = tree("board");
    board_fans(
        &t,
        BOARD_CHIP,
        &[(1, 930, None), (2, 3_400, Some("Pump")), (3, 0, None)],
    )
    .expect("board");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/name"), "fakeboardchip\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/fan1_input"), "930\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/fan2_label"), "Pump\n");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/fan3_input"), "0\n");
    assert!(!t.root().join("sys/class/hwmon/hwmon0/fan1_label").exists());
    assert_eq!(
        resolved(&t, "sys/class/hwmon/hwmon0/device"),
        "sys/devices/platform/fakeboardchip.656"
    );
}

#[test]
fn hwmon_numbers_are_the_lowest_free_so_a_test_can_change_them() {
    let t = tree("numbers");
    coretemp(&t, 57_000).expect("coretemp");
    board_fans(&t, BOARD_CHIP, &[(1, 930, None)]).expect("board");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon1/name"), "fakeboardchip\n");
    t.remove("sys/class/hwmon/hwmon0").expect("the link");
    t.remove("sys/devices/platform/coretemp.0")
        .expect("the device");
    board_fans(&t, "otherfakechip", &[(1, 700, None)]).expect("another chip");
    assert_eq!(read(&t, "sys/class/hwmon/hwmon0/name"), "otherfakechip\n");
}

#[test]
fn the_fake_tree_writes_links_and_modes_only_inside_itself() {
    let t = tree("methods");
    let file = t.file("sys/x/y/value", "42\n").expect("a file");
    assert_eq!(fs::read_to_string(&file).expect("its contents"), "42\n");
    t.symlink("sys/link", "x/y").expect("a link");
    assert_eq!(read(&t, "sys/link/value"), "42\n");
    t.mode("sys/x/y/value", 0o400).expect("a mode");
    let mode = fs::metadata(&file).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o400);
    t.remove("sys/link").expect("the link");
    assert!(file.exists(), "removing a link keeps what it points to");
    t.remove("sys/x").expect("the folder");
    assert!(!file.exists());
    for outside in ["/tmp/odc-outside", "../outside", "sys/../../outside"] {
        assert!(t.file(outside, "x").is_err(), "{outside}");
        assert!(t.symlink(outside, "x").is_err(), "{outside}");
        assert!(t.remove(outside).is_err(), "{outside}");
    }
}
