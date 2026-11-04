//! The hardware probe every phase's end-to-end checks share (owner, 2026-10-24: the dev program
//! runs on other computers too). A device present and usable is tested for real, an absent one
//! through fakes, and one present but unusable fails with its fix, never falling back to fakes
//! (PRD, Testing Strategy, layer 4).

use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use odc_device::HidrawConnector;
use odc_sensors::{NvmlSource, RealNvml, display_devices, fan_inputs, rapl_package_zones};

use crate::sysfs::{LP360_PRODUCT, LP360_VENDOR, usb_device};

/// What a check needs from the PC. Phases 6–11 add theirs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Capability {
    /// The LP360's hidraw node, writable by this account.
    Lp360,
    /// The LP360's USB `authorized` switch, writable by this account (the dev reconnect rule).
    Lp360Reconnect,
    /// A CPU temperature driver, coretemp or k10temp (Phase 6, Task 11).
    CpuTemp,
    /// The RAPL package energy counters, readable by this account through the group.
    CpuPower,
    /// `/proc/stat` with its aggregate `cpu` line.
    ProcStat,
    /// `/proc/meminfo` with `MemTotal` and `MemAvailable`.
    Meminfo,
    /// A GPU on amdgpu whose render node this account can open (Phase 6, Task 9).
    GpuAmdgpu,
    /// A GPU on i915 whose render node this account can open.
    GpuI915,
    /// A GPU on xe whose render node this account can open.
    GpuXe,
    /// A GPU on NVIDIA's driver, where NVIDIA's library, NVML, starts (Task 11): CUDA tells its
    /// kind and NVML its readings. Without NVML, its checks run through fakes.
    GpuNvidia,
    /// A GPU on a driver without a kind signal, such as nouveau; nothing on it is opened.
    GpuOther,
    /// A fan input outside a graphics card: the board's sensor driver is loaded.
    BoardFans,
    /// lm-sensors' `sensors`, whose `sensors -j` the readings are compared with.
    SensorsTool,
    /// `ffmpeg`, whose Vulkan filters load a GPU for the Gpu usage check.
    GpuLoadTool,
    /// The dev copy of the display service (Phase 7 plan, Task 9): active, started with `--dev`,
    /// owning its bus name, its recording readable by this account, and the LP360 usable.
    DevService,
}

/// The drivers with a kind signal (`docs/spec/sensors.md`, GPU choice).
const SIGNAL_DRIVERS: [&str; 4] = ["amdgpu", "i915", "xe", "nvidia"];

impl Capability {
    pub const ALL: [Capability; 15] = [
        Capability::Lp360,
        Capability::Lp360Reconnect,
        Capability::CpuTemp,
        Capability::CpuPower,
        Capability::ProcStat,
        Capability::Meminfo,
        Capability::GpuAmdgpu,
        Capability::GpuI915,
        Capability::GpuXe,
        Capability::GpuNvidia,
        Capability::GpuOther,
        Capability::BoardFans,
        Capability::SensorsTool,
        Capability::GpuLoadTool,
        Capability::DevService,
    ];

    /// The name `--require` takes.
    pub fn name(self) -> &'static str {
        match self {
            Capability::Lp360 => "lp360",
            Capability::Lp360Reconnect => "lp360-reconnect",
            Capability::CpuTemp => "cpu-temp",
            Capability::CpuPower => "cpu-power",
            Capability::ProcStat => "proc-stat",
            Capability::Meminfo => "meminfo",
            Capability::GpuAmdgpu => "gpu-amdgpu",
            Capability::GpuI915 => "gpu-i915",
            Capability::GpuXe => "gpu-xe",
            Capability::GpuNvidia => "gpu-nvidia",
            Capability::GpuOther => "gpu-other",
            Capability::BoardFans => "board-fans",
            Capability::SensorsTool => "sensors-tool",
            Capability::GpuLoadTool => "gpu-load-tool",
            Capability::DevService => "dev-service",
        }
    }

    /// The GPU driver a GPU capability stands for; `None` for the others and for
    /// [`Capability::GpuOther`], which stands for every driver without a kind signal.
    pub fn gpu_driver(self) -> Option<&'static str> {
        match self {
            Capability::GpuAmdgpu => Some("amdgpu"),
            Capability::GpuI915 => Some("i915"),
            Capability::GpuXe => Some("xe"),
            Capability::GpuNvidia => Some("nvidia"),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Capability> {
        Capability::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// What the probe found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Probe {
    /// Present, and this account can use it.
    Real,
    /// Not present: the checks run against fakes.
    Absent { reason: String },
    /// Present but not usable: the checks fail with the fix.
    Unusable { reason: String, fix: String },
}

/// The roots the probe looks under: `/sys`, `/dev` and `/proc`, and `/usr/bin` for the tools the
/// checks compare with, or a fake tree's.
#[derive(Clone, Debug)]
pub struct Roots {
    pub sysfs: PathBuf,
    pub dev: PathBuf,
    pub proc: PathBuf,
    pub bin: PathBuf,
}

impl Roots {
    pub fn system() -> Self {
        Roots {
            sysfs: PathBuf::from("/sys"),
            dev: PathBuf::from("/dev"),
            proc: PathBuf::from("/proc"),
            bin: PathBuf::from("/usr/bin"),
        }
    }

    /// The same roots for the product's sensor rules.
    pub fn sensors(&self) -> odc_sensors::Roots {
        odc_sensors::Roots {
            sys: self.sysfs.clone(),
            proc: self.proc.clone(),
        }
    }
}

const SETUP_FIX: &str = "run `cargo run -p odc-dev -- setup`, then log in again or use \
                         `newgrp opendeepcool`";

const RENDER_FIX: &str = "give this account read and write access to the GPU's render node \
                          (`/dev/dri/renderD*`, the `render` group)";

/// Finds a capability the way the product finds it: by USB ID, hwmon `name`, powercap zone name,
/// PCI class and driver, or an NVML start.
pub fn probe(capability: Capability, roots: &Roots) -> Probe {
    match capability {
        Capability::Lp360 => {
            let node = match HidrawConnector::new(&roots.sysfs, &roots.dev).find() {
                Ok(Some(node)) => node,
                Ok(None) => return absent("no LP360 (USB 3633:000c) is attached"),
                Err(e) => return unusable(&e.path, &e.error.to_string()),
            };
            writable(&node.dev)
        }
        Capability::Lp360Reconnect => match usb_device(&roots.sysfs, LP360_VENDOR, LP360_PRODUCT) {
            Some(usb) => writable(&usb.join("authorized")),
            None => absent("no LP360 USB device (3633:000c) is attached"),
        },
        Capability::CpuTemp => {
            let found = fs::read_dir(roots.sysfs.join("class/hwmon"))
                .into_iter()
                .flatten()
                .flatten()
                .any(|entry| {
                    fs::read_to_string(entry.path().join("name"))
                        .is_ok_and(|name| ["coretemp", "k10temp"].contains(&name.trim()))
                });
            if found {
                Probe::Real
            } else {
                absent("no CPU temperature driver (coretemp, k10temp)")
            }
        }
        Capability::CpuPower => {
            let zones = rapl_package_zones(&roots.sysfs);
            if zones.is_empty() {
                return absent("no RAPL package zone");
            }
            for zone in zones {
                let counter = zone.join("energy_uj");
                match fs::read_to_string(&counter) {
                    Ok(_) => {}
                    Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                        return unusable(&counter, "this account may not read it");
                    }
                    Err(e) => return unusable(&counter, &e.to_string()),
                }
            }
            Probe::Real
        }
        Capability::ProcStat => match fs::read_to_string(roots.proc.join("stat")) {
            Ok(text) if text.lines().any(|line| line.starts_with("cpu ")) => Probe::Real,
            _ => absent("no /proc/stat with its cpu line"),
        },
        Capability::Meminfo => match fs::read_to_string(roots.proc.join("meminfo")) {
            Ok(text)
                if ["MemTotal:", "MemAvailable:"]
                    .iter()
                    .all(|key| text.lines().any(|line| line.starts_with(key))) =>
            {
                Probe::Real
            }
            _ => absent("no /proc/meminfo with MemTotal and MemAvailable"),
        },
        Capability::GpuAmdgpu | Capability::GpuI915 | Capability::GpuXe => {
            let driver = capability.gpu_driver().unwrap_or_default();
            let gpus: Vec<String> = display_devices(&roots.sysfs)
                .into_iter()
                .filter(|(_, d)| d == driver)
                .map(|(pci, _)| pci.to_string())
                .collect();
            if gpus.is_empty() {
                return absent(&format!("no GPU on {driver}"));
            }
            gpus.iter()
                .map(|pci| render_node(roots, pci))
                .find(|probe| *probe != Probe::Real)
                .unwrap_or(Probe::Real)
        }
        Capability::GpuNvidia => {
            if !display_devices(&roots.sysfs)
                .iter()
                .any(|(_, d)| d == "nvidia")
            {
                return absent("no GPU on NVIDIA's driver (nvidia)");
            }
            match RealNvml::new().start() {
                Ok(()) => Probe::Real,
                Err(e) => absent(&format!("a GPU on nvidia, but NVML doesn't start: {e}")),
            }
        }
        Capability::GpuOther => {
            if display_devices(&roots.sysfs)
                .iter()
                .any(|(_, d)| !SIGNAL_DRIVERS.contains(&d.as_str()))
            {
                Probe::Real
            } else {
                absent("no GPU on a driver without a kind signal")
            }
        }
        Capability::BoardFans => {
            if fan_inputs(&roots.sysfs).is_empty() {
                absent(
                    "no fan input outside a graphics card: the board's sensor driver isn't loaded",
                )
            } else {
                Probe::Real
            }
        }
        Capability::SensorsTool => tool(
            roots,
            "sensors",
            "install lm-sensors: `sudo pacman -S lm_sensors`",
        ),
        Capability::GpuLoadTool => tool(roots, "ffmpeg", "install ffmpeg: `sudo pacman -S ffmpeg`"),
        Capability::DevService => dev_service(roots),
    }
}

/// The dev copy's unit, which `odc-dev setup` installs.
pub const DEV_UNIT: &str = "opendeepcool.service";

/// The dev copy's frame recording: `odc_service::RECORDING` in its state directory.
pub const DEV_RECORDING: &str = "/var/lib/opendeepcool/dev-frames.log";

const SERVICE_FIX: &str = "run `cargo run -p odc-dev -- setup`; `cargo run -p odc-dev -- check` \
                           says what's missing";

/// The dev service, as `odc-dev check` finds it: its unit's state and main process from
/// `<bin>/systemctl`, the process's arguments, the bus name's owner, and the recording. Where
/// there's no `systemctl`, as in a fake tree, or no such unit, it's absent.
fn dev_service(roots: &Roots) -> Probe {
    let Ok(out) = Command::new(roots.bin.join("systemctl"))
        .args(["show", "--property=LoadState,ActiveState,MainPID", DEV_UNIT])
        .output()
    else {
        return absent("no systemctl to ask about the dev service");
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let value = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
            .unwrap_or("")
            .to_owned()
    };
    if value("LoadState") != "loaded" {
        return absent(&format!("the dev service ({DEV_UNIT}) isn't installed"));
    }
    let unusable = |reason: String| Probe::Unusable {
        reason,
        fix: SERVICE_FIX.to_owned(),
    };
    let state = value("ActiveState");
    if state != "active" {
        return unusable(format!("{DEV_UNIT} is {state}"));
    }
    let pid = value("MainPID");
    let cmdline = roots.proc.join(&pid).join("cmdline");
    match fs::read(&cmdline) {
        Ok(args) if args.split(|b| *b == 0).any(|arg| arg == b"--dev") => {}
        Ok(_) => {
            return unusable(format!(
                "{DEV_UNIT} runs without --dev: it's not the dev copy"
            ));
        }
        Err(e) => return unusable(format!("{}: {e}", cmdline.display())),
    }
    if let Err(e) = bus_name_owner() {
        return unusable(format!("{}: {e}", odc_ipc::BUS_NAME));
    }
    let recording = Path::new(DEV_RECORDING);
    if let Err(e) = fs::File::open(recording) {
        return unusable_with(recording, &e.to_string(), SETUP_FIX);
    }
    // Without a usable cooler, the real service has nothing to show: its checks run in-process.
    probe(Capability::Lp360, roots)
}

/// The unique name that owns the service's bus name on the system bus.
fn bus_name_owner() -> Result<String, String> {
    zbus::block_on(async {
        let connection = zbus::Connection::system().await?;
        let dbus = zbus::fdo::DBusProxy::new(&connection).await?;
        let name = zbus::names::BusName::try_from(odc_ipc::BUS_NAME)?;
        let owner = dbus.get_name_owner(name).await?;
        Ok::<String, zbus::Error>(owner.to_string())
    })
    .map_err(|e| e.to_string())
}

/// A tool in `<bin>`, which the checks start.
fn tool(roots: &Roots, name: &str, fix: &str) -> Probe {
    let path = roots.bin.join(name);
    match fs::metadata(&path) {
        Ok(meta) if meta.is_file() && meta.permissions().mode() & 0o111 != 0 => Probe::Real,
        _ => Probe::Unusable {
            reason: format!("{} is missing", path.display()),
            fix: fix.to_owned(),
        },
    }
}

/// The GPU's render node, as the rank helper opens it: for reading and writing.
fn render_node(roots: &Roots, pci: &str) -> Probe {
    let drm = roots.sysfs.join("bus/pci/devices").join(pci).join("drm");
    let name = fs::read_dir(&drm).ok().and_then(|entries| {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with("renderD"))
            .collect();
        names.sort();
        names.into_iter().next()
    });
    let Some(name) = name else {
        return Probe::Unusable {
            reason: format!("{}: no render node", drm.display()),
            fix: RENDER_FIX.to_owned(),
        };
    };
    let node = roots.dev.join("dri").join(name);
    match OpenOptions::new().read(true).write(true).open(&node) {
        Ok(_) => Probe::Real,
        Err(e) => Probe::Unusable {
            reason: format!("{}: {e}", node.display()),
            fix: RENDER_FIX.to_owned(),
        },
    }
}

/// Opening for writing proves access and writes nothing.
fn writable(path: &Path) -> Probe {
    match OpenOptions::new().write(true).open(path) {
        Ok(_) => Probe::Real,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            unusable(path, "this account may not write to it")
        }
        Err(e) => unusable(path, &e.to_string()),
    }
}

fn absent(reason: &str) -> Probe {
    Probe::Absent {
        reason: reason.to_owned(),
    }
}

fn unusable(path: &Path, why: &str) -> Probe {
    unusable_with(path, why, SETUP_FIX)
}

fn unusable_with(path: &Path, why: &str, fix: &str) -> Probe {
    Probe::Unusable {
        reason: format!("{}: {why}", path.display()),
        fix: fix.to_owned(),
    }
}

/// How a capability ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Real,
    Fake { reason: String },
    Failed { reason: String },
}

/// Collects every capability's mode and every check's result, and prints them as one table at
/// the end of a run.
#[derive(Debug, Default)]
pub struct Summary {
    modes: Vec<(Capability, Mode)>,
    checks: Vec<(String, Result<String, String>)>,
}

impl Summary {
    /// Records how a capability ran; the first record wins.
    pub fn mode(&mut self, capability: Capability, mode: Mode) {
        if !self.modes.iter().any(|(c, _)| *c == capability) {
            self.modes.push((capability, mode));
        }
    }

    pub fn check(&mut self, name: &str, result: Result<String, String>) {
        self.checks.push((name.to_owned(), result));
    }

    pub fn passed(&self) -> bool {
        self.checks.iter().all(|(_, r)| r.is_ok())
            && self
                .modes
                .iter()
                .all(|(_, m)| !matches!(m, Mode::Failed { .. }))
    }

    /// The table, one line per check, then one per capability.
    pub fn table(&self) -> String {
        let mut out = String::from("check            result\n");
        for (name, result) in &self.checks {
            let line = match result {
                Ok(note) => format!("passed  {note}"),
                Err(reason) => format!("FAILED  {reason}"),
            };
            out.push_str(&format!("{name:<16} {line}\n"));
        }
        out.push_str("capability       mode\n");
        let mut modes = self.modes.clone();
        modes.sort_by_key(|(c, _)| *c);
        for (capability, mode) in modes {
            let line = match mode {
                Mode::Real => "real".to_owned(),
                Mode::Fake { reason } => format!("fake    {reason}"),
                Mode::Failed { reason } => format!("FAILED  {reason}"),
            };
            out.push_str(&format!("{:<16} {line}\n", capability.name()));
        }
        out
    }
}
