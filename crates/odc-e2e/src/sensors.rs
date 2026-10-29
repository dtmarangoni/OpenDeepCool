//! The sensor checks (Phase 6 plan, Task 11): every metric through the registry.
//! - A source the PC has runs real: the registry reads `/sys` and `/proc` as this account, a
//!   member of `opendeepcool`, and each reading is compared with a tool that reads the same
//!   source, in the same second: `sensors -j`, `free -b`, `top`.
//! - A source the PC hasn't runs the same checks over the Task 3 fixtures, whose values are
//!   chosen to pass them, against the fixtures' own values.
//! - One that's there but unusable fails with its fix.
//!
//! Each check also asks the registry which source it bound, so every metric names its source.
//! Nothing about any PC is written here.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::{fs, hint};

use odc_gpu_rank::{DriverKinds, rank};
use odc_sensors::fake::{FakeClock, FakeNvml};
use odc_sensors::json::{self, Json};
use odc_sensors::{
    Binding, Clock, Gpu, GpuKind, Metric, PciAddr, Ranked, RankedKind, Ranking, Reading, Readings,
    Registry, SystemClock, Unavailable, fan_inputs, fan_rpm, gpus,
};
use odc_testkit::fake_tree::FakeTree;
use odc_testkit::fixtures::{
    BOARD_CHIP, amdgpu_dgpu, board_fans, coretemp, i915, nouveau, nvidia_proprietary, proc_meminfo,
    proc_stat, rapl, xe,
};
use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};

/// The capabilities this area needs, the GPUs' included (Task 9 probes them too).
const SENSORS: [Capability; 12] = [
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
];

const GPUS: [Capability; 5] = [
    Capability::GpuAmdgpu,
    Capability::GpuI915,
    Capability::GpuXe,
    Capability::GpuNvidia,
    Capability::GpuOther,
];

/// The checks besides the GPUs', each with the capability whose mode decides how it runs.
const CHECKS: [(&str, Capability); 5] = [
    ("cpu_temp", Capability::CpuTemp),
    ("cpu_usage", Capability::ProcStat),
    ("cpu_power", Capability::CpuPower),
    ("memory_load", Capability::Meminfo),
    ("fans", Capability::BoardFans),
];

/// The Gpu usage check's load lasts this long, and the reading must drop within as long after.
const LOAD: Duration = Duration::from_secs(5);

pub fn run(roots: &Roots, out_dir: &Path, summary: &mut Summary) {
    let probes: Vec<(Capability, Probe)> = SENSORS.iter().map(|&c| (c, probe(c, roots))).collect();
    let found = |capability: Capability| -> &Probe {
        &probes
            .iter()
            .find(|(c, _)| *c == capability)
            .expect("probed")
            .1
    };
    let real = |capability: Capability| *found(capability) == Probe::Real;
    let real_gpu = GPUS.iter().any(|&c| real(c));
    let needs_sensors = real(Capability::CpuTemp)
        || real(Capability::BoardFans)
        || [
            Capability::GpuAmdgpu,
            Capability::GpuI915,
            Capability::GpuXe,
        ]
        .iter()
        .any(|&c| real(c));
    let needs_load = real(Capability::GpuAmdgpu) || real(Capability::GpuNvidia);
    for (capability, found) in &probes {
        let mode = match found {
            Probe::Real => Mode::Real,
            Probe::Absent { reason } => Mode::Fake {
                reason: reason.clone(),
            },
            Probe::Unusable { reason, fix } => {
                let needed = match capability {
                    Capability::SensorsTool => needs_sensors,
                    Capability::GpuLoadTool => needs_load,
                    _ => true,
                };
                if needed {
                    Mode::Failed {
                        reason: format!("{reason}; fix: {fix}"),
                    }
                } else {
                    Mode::Fake {
                        reason: format!("{reason}, and no real check needs it"),
                    }
                }
            }
        };
        summary.mode(*capability, mode);
    }
    let tool = |capability: Capability, name: &str| match found(capability) {
        Probe::Real => Ok(roots.bin.join(name)),
        Probe::Absent { reason } => Err(reason.clone()),
        Probe::Unusable { reason, fix } => Err(format!("{reason}; fix: {fix}")),
    };
    let tools = Tools {
        sensors: tool(Capability::SensorsTool, "sensors"),
        ffmpeg: tool(Capability::GpuLoadTool, "ffmpeg"),
        free: roots.bin.join("free"),
        top: roots.bin.join("top"),
    };
    let fake = FakePc::new(&out_dir.join("sensors/tree"));
    let mut real_pc =
        (CHECKS.iter().any(|(_, c)| real(*c)) || real_gpu).then(|| RealPc::new(roots, tools));
    for (name, capability) in CHECKS {
        let result = match found(capability) {
            Probe::Real => match real_pc.as_mut() {
                Some(pc) => pc.check(name),
                None => Err("no real run".to_owned()),
            },
            Probe::Absent { .. } => match &fake {
                Ok(pc) => pc.check(name),
                Err(e) => Err(e.clone()),
            },
            Probe::Unusable { reason, fix } => Err(format!("{reason}; fix: {fix}")),
        };
        summary.check(name, result);
    }
    for capability in GPUS {
        let driver = capability.gpu_driver().unwrap_or("other");
        let name = format!("gpu_{driver}");
        let results = match found(capability) {
            Probe::Real => match real_pc.as_mut() {
                Some(pc) => pc.gpus(capability),
                None => vec![Err("no real run".to_owned())],
            },
            Probe::Absent { .. } => vec![match &fake {
                Ok(pc) => pc.gpu(capability),
                Err(e) => Err(e.clone()),
            }],
            Probe::Unusable { reason, fix } => vec![Err(format!("{reason}; fix: {fix}"))],
        };
        for (i, result) in results.into_iter().enumerate() {
            let name = if i == 0 {
                name.clone()
            } else {
                format!("{name}_{}", i + 1)
            };
            summary.check(&name, result);
        }
    }
}

/// The tools the real checks compare with: a path, or why it can't be used.
struct Tools {
    sensors: Result<PathBuf, String>,
    ffmpeg: Result<PathBuf, String>,
    free: PathBuf,
    top: PathBuf,
}

/// The registry over the PC's own `/sys` and `/proc`.
struct RealPc<'a> {
    roots: &'a Roots,
    tools: Tools,
    registry: Registry<SystemClock>,
    /// CPU Usage and CPU Power, measured together under one load.
    cpu: Option<Result<CpuRun, String>>,
}

/// One CPU load's readings: the usage and `top`'s in the same second, and the power at idle and
/// under the load.
struct CpuRun {
    usage: Reading,
    top_busy: Result<i64, String>,
    idle_power: Reading,
    load_power: Reading,
}

impl<'a> RealPc<'a> {
    fn new(roots: &'a Roots, tools: Tools) -> Self {
        let mut registry = Registry::from_rules(SystemClock, roots.sensors());
        // The first snapshot binds the sources; the next one has the rates' first interval.
        registry.snapshot();
        thread::sleep(Duration::from_secs(1));
        registry.snapshot();
        RealPc {
            roots,
            tools,
            registry,
            cpu: None,
        }
    }

    fn check(&mut self, name: &str) -> Result<String, String> {
        match name {
            "cpu_temp" => self.cpu_temp(),
            "cpu_usage" => {
                let run = self.cpu_run()?;
                cpu_usage(&run.usage, run.top_busy.clone()).map(|note| format!("real: {note}"))
            }
            "cpu_power" => {
                if euid()? == 0 {
                    return Err(
                        "run as your own account: as root, the group's access isn't \
                                tested"
                            .to_owned(),
                    );
                }
                let run = self.cpu_run()?;
                cpu_power(&run.idle_power, &run.load_power)
                    .map(|note| format!("real, through the group: {note}"))
            }
            "memory_load" => {
                let ours = self.read(Metric::MemoryLoad)?;
                let reference = free_load(&self.tools.free)?;
                memory_load(&ours, reference).map(|note| format!("real: {note}"))
            }
            "fans" => self.fans(),
            _ => Err(format!("no check {name}")),
        }
    }

    /// A metric's reading now, with the source it's bound to.
    fn read(&mut self, metric: Metric) -> Result<Reading, String> {
        let reading = self.registry.snapshot().get(metric).clone();
        bound(&self.registry, metric)?;
        Ok(reading)
    }

    fn cpu_temp(&mut self) -> Result<String, String> {
        let ours = self.registry.snapshot().get(Metric::CpuTemperature).clone();
        if let Some((_, Binding::Unbound(Unavailable::NoRule { driver }))) = self
            .registry
            .bindings()
            .into_iter()
            .find(|(m, _)| *m == Metric::CpuTemperature)
            && driver == "k10temp"
        {
            return Ok("real: k10temp before family 17h has no reading, as the rules say".into());
        }
        let source = bound(&self.registry, Metric::CpuTemperature)?;
        let sensors = sensors_json(self.tools.sensors.as_ref())?;
        let reference = cpu_package(&sensors)?;
        close("CPU Temperature", milli(&ours)?, reference, 1_000)
            .map(|note| format!("real: {note} ({source})"))
    }

    /// Idle power, then CPU Usage and power under a load on every logical CPU, compared with
    /// `top` in the same second. Run once, for both checks.
    fn cpu_run(&mut self) -> Result<&CpuRun, String> {
        if self.cpu.is_none() {
            self.cpu = Some(self.measure_cpu());
        }
        self.cpu
            .as_ref()
            .expect("measured")
            .as_ref()
            .map_err(Clone::clone)
    }

    fn measure_cpu(&mut self) -> Result<CpuRun, String> {
        self.registry.snapshot();
        thread::sleep(Duration::from_secs(1));
        let idle_power = self.registry.snapshot().get(Metric::CpuPower).clone();
        let load = CpuLoad::start();
        thread::sleep(Duration::from_secs(1));
        self.registry.snapshot();
        let top_busy = top_busy(&self.tools.top);
        let usage = self.registry.snapshot().get(Metric::CpuUsage).clone();
        let mut load_power = Reading::Unavailable(Unavailable::NotYet);
        while load.started.elapsed() < LOAD {
            thread::sleep(Duration::from_millis(1_000));
            load_power = self.registry.snapshot().get(Metric::CpuPower).clone();
        }
        drop(load);
        bound(&self.registry, Metric::CpuUsage)?;
        Ok(CpuRun {
            usage,
            top_busy,
            idle_power,
            load_power,
        })
    }

    /// Every fan input the product lists, against `sensors -j`'s reading of the same chip and
    /// input.
    fn fans(&mut self) -> Result<String, String> {
        let sys = &self.roots.sysfs;
        let inputs = fan_inputs(sys);
        let ours: Vec<(String, u32, Reading)> = inputs
            .iter()
            .map(|input| (input.chip.clone(), input.input, fan_rpm(sys, input)))
            .collect();
        let sensors = sensors_json(self.tools.sensors.as_ref())?;
        for (chip, n, reading) in &ours {
            let reference = fan_reference(&sensors, chip, *n)?;
            fan(chip, *n, reading, reference)?;
        }
        Ok(format!(
            "real: {} fan inputs agree with sensors",
            ours.len()
        ))
    }

    /// Each real GPU on `capability`'s driver: chosen in turn and checked.
    fn gpus(&mut self, capability: Capability) -> Vec<Result<String, String>> {
        let sys = self.roots.sysfs.clone();
        let ranking = rank(&sys, &mut DriverKinds::new(&sys, &self.roots.dev));
        let ours: Vec<Gpu> = gpus(&sys, Some(&ranking))
            .into_iter()
            .filter(|gpu| match capability.gpu_driver() {
                Some(driver) => gpu.driver == driver,
                None => odc_gpu_rank::Signal::for_driver(&gpu.driver).is_none(),
            })
            .collect();
        if ours.is_empty() {
            return vec![Err(
                "the probe found the driver, the GPU list didn't".to_owned()
            )];
        }
        ours.iter().map(|gpu| self.gpu(gpu)).collect()
    }

    fn gpu(&mut self, gpu: &Gpu) -> Result<String, String> {
        self.registry.choose_gpu(Some(gpu));
        self.registry.snapshot();
        let mut notes = vec![format!("real {}", gpu.pci)];
        match gpu.driver.as_str() {
            driver @ ("amdgpu" | "i915" | "xe") => {
                let chip = pci_chip(driver, gpu.pci);
                let ours = self.registry.snapshot().get(Metric::GpuTemperature).clone();
                let sensors = sensors_json(self.tools.sensors.as_ref())?;
                let reference = gpu_temperature(&sensors, &chip, driver)?;
                notes.push(close("Gpu temperature", milli(&ours)?, reference, 1_000)?);
                bound(&self.registry, Metric::GpuTemperature)?;
                notes.push(self.gpu_power(gpu, &chip)?);
                notes.push(if driver == "amdgpu" {
                    self.gpu_usage(gpu)?
                } else {
                    no_usage(self.registry.snapshot().get(Metric::GpuUsage), driver)?
                });
            }
            "nvidia" => {
                let snapshot = self.registry.snapshot();
                notes.push(nvml_plausible(
                    snapshot.get(Metric::GpuTemperature),
                    snapshot.get(Metric::GpuUsage),
                    snapshot.get(Metric::GpuPower),
                )?);
                for metric in [Metric::GpuTemperature, Metric::GpuUsage, Metric::GpuPower] {
                    bound(&self.registry, metric)?;
                }
                notes.push(self.gpu_usage(gpu)?);
            }
            driver => {
                let snapshot = self.registry.snapshot();
                notes.push(no_readings(&snapshot_gpu(&snapshot), driver)?);
            }
        }
        Ok(notes.join("; "))
    }

    /// Gpu power: against `sensors -j`'s power for a card, its energy over the same second for
    /// Intel's drivers, and unavailable where the rules say so.
    fn gpu_power(&mut self, gpu: &Gpu, chip: &str) -> Result<String, String> {
        match (gpu.driver.as_str(), &gpu.kind) {
            ("amdgpu", GpuKind::Dedicated) => {
                let ours = self.registry.snapshot().get(Metric::GpuPower).clone();
                let sensors = sensors_json(self.tools.sensors.as_ref())?;
                let reference = chip_value(&sensors, chip, None, |s| {
                    s == "power1_average" || s == "power1_input"
                })
                .ok_or_else(|| format!("sensors shows no power for {chip}"))?;
                bound(&self.registry, Metric::GpuPower)?;
                gpu_power(&ours, reference)
            }
            ("amdgpu", kind) => {
                let expected = match kind {
                    GpuKind::Integrated => Unavailable::IntegratedGpuPower,
                    _ => Unavailable::UnknownGpuKind,
                };
                unavailable(self.registry.snapshot().get(Metric::GpuPower), &expected)
            }
            _ => {
                self.registry.snapshot();
                let before = sensors_json(self.tools.sensors.as_ref())?;
                let from = Instant::now();
                thread::sleep(Duration::from_secs(1));
                let ours = self.registry.snapshot().get(Metric::GpuPower).clone();
                let after = sensors_json(self.tools.sensors.as_ref())?;
                let micros = i64::try_from(from.elapsed().as_micros()).unwrap_or(i64::MAX);
                let energy = |json: &Json| {
                    chip_value(json, chip, None, |s| s == "energy1_input")
                        .ok_or_else(|| format!("sensors shows no energy for {chip}"))
                };
                // Joules in thousandths over microseconds: watts in thousandths.
                let reference = (energy(&after)? - energy(&before)?) * 1_000_000 / micros.max(1);
                bound(&self.registry, Metric::GpuPower)?;
                gpu_power(&ours, reference)
            }
        }
    }

    /// Gpu usage under an `ffmpeg` Vulkan load on the GPU, then after it.
    fn gpu_usage(&mut self, gpu: &Gpu) -> Result<String, String> {
        let ffmpeg = self.tools.ffmpeg.clone()?;
        let node = render_node(&self.roots.sysfs, &self.roots.dev, gpu.pci)?;
        let mut load = GpuLoad::start(&ffmpeg, &node)?;
        let started = Instant::now();
        let mut during = Vec::new();
        while started.elapsed() < LOAD {
            thread::sleep(Duration::from_millis(500));
            load.still_running()?;
            during.push(milli(self.registry.snapshot().get(Metric::GpuUsage))?);
        }
        drop(load);
        let stopped = Instant::now();
        let mut after = Vec::new();
        while stopped.elapsed() < LOAD {
            thread::sleep(Duration::from_millis(500));
            after.push(milli(self.registry.snapshot().get(Metric::GpuUsage))?);
            if after.last().is_some_and(|&u| u < 50_000) {
                break;
            }
        }
        bound(&self.registry, Metric::GpuUsage)?;
        gpu_usage(&during, &after)
    }
}

/// The GPU's three readings, for the drivers without any.
fn snapshot_gpu(snapshot: &odc_sensors::Snapshot) -> [Reading; 3] {
    [
        snapshot.get(Metric::GpuTemperature).clone(),
        snapshot.get(Metric::GpuUsage).clone(),
        snapshot.get(Metric::GpuPower).clone(),
    ]
}

/// The same checks over the Task 3 fixtures, on a fake clock: nothing waits.
struct FakePc {
    tree: FakeTree,
    nvidia: &'static str,
}

/// The fixtures' values, chosen to pass the checks.
const PACKAGE_MILLIC: i64 = 57_000;
const MEM_TOTAL_KB: u64 = 16_000_000;
const MEM_AVAILABLE_KB: u64 = 6_000_000;
const FANS: [(u32, u64, Option<&str>); 3] =
    [(1, 1_234, None), (2, 3_456, Some("Pump")), (3, 0, None)];
const CARD: &str = "0000:0a:00.0";
const IGPU: &str = "0000:00:02.0";
const CARD2: &str = "0000:0b:00.0";
const NVIDIA: &str = "0000:0d:00.0";
const NOUVEAU: &str = "0000:0e:00.0";
const RAPL: &str = "sys/devices/virtual/powercap/intel-rapl/intel-rapl:0/energy_uj";

impl FakePc {
    fn new(dir: &Path) -> Result<FakePc, String> {
        let tree = FakeTree::create(dir).map_err(|e| e.to_string())?;
        let built = coretemp(&tree, PACKAGE_MILLIC)
            .and_then(|_| {
                rapl(
                    &tree,
                    &[("intel-rapl:0", "package-0", 1_000_000, 262_143_328_850)],
                )
            })
            .and_then(|()| proc_meminfo(&tree, MEM_TOTAL_KB, MEM_AVAILABLE_KB))
            .and_then(|_| board_fans(&tree, BOARD_CHIP, &FANS))
            .and_then(|_| amdgpu_dgpu(&tree, CARD, 41_000, 37, 182_000_000))
            .and_then(|_| i915(&tree, IGPU, 44_000, 9_100_000))
            .and_then(|_| xe(&tree, CARD2, 47_000, 7_300_000))
            .and_then(|_| nvidia_proprietary(&tree, NVIDIA))
            .and_then(|_| nouveau(&tree, NOUVEAU));
        built.map_err(|e| e.to_string())?;
        Ok(FakePc {
            tree,
            nvidia: NVIDIA,
        })
    }

    /// A registry over the tree, its fake clock, and NVML's fake.
    fn registry(&self) -> (Registry<FakeClock>, FakeClock) {
        let clock = FakeClock::new();
        let nvml = FakeNvml::new(Ok(()), &[(self.nvidia, 61, 97, 150_000)]);
        let roots = odc_sensors::Roots {
            sys: self.tree.roots().sysfs,
            proc: self.tree.roots().proc,
        };
        (
            Registry::from_rules_with_nvml(clock.clone(), roots, Box::new(nvml)),
            clock,
        )
    }

    fn write(&self, path: &str, value: impl ToString) -> Result<(), String> {
        self.tree
            .file(path, format!("{}\n", value.to_string()))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// `/proc/stat` whose aggregate line has `busy` busy and `idle` idle jiffies.
    fn stat(&self, busy: u64, idle: u64) -> Result<(), String> {
        proc_stat(
            &self.tree,
            &format!("cpu  {busy} 0 0 {idle} 0 0 0 0 0 0\ncpu0 {busy} 0 0 {idle} 0 0 0 0 0 0\n"),
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }

    fn check(&self, name: &str) -> Result<String, String> {
        let (mut registry, clock) = self.registry();
        let note = match name {
            "cpu_temp" => {
                let ours = registry.snapshot().get(Metric::CpuTemperature).clone();
                bound(&registry, Metric::CpuTemperature)?;
                close("CPU Temperature", milli(&ours)?, PACKAGE_MILLIC, 1_000)?
            }
            "cpu_usage" => {
                self.stat(1_500, 8_500)?;
                registry.snapshot();
                clock.advance(Duration::from_secs(1));
                self.stat(1_597, 8_503)?;
                let ours = registry.snapshot().get(Metric::CpuUsage).clone();
                bound(&registry, Metric::CpuUsage)?;
                // `top`'s share of the same counters: 97 busy jiffies of 100.
                cpu_usage(&ours, Ok(97_000))?
            }
            "cpu_power" => {
                self.write(RAPL, 1_000_000)?;
                registry.snapshot();
                clock.advance(Duration::from_secs(1));
                self.write(RAPL, 7_000_000)?;
                let idle = registry.snapshot().get(Metric::CpuPower).clone();
                clock.advance(Duration::from_secs(1));
                self.write(RAPL, 95_000_000)?;
                let load = registry.snapshot().get(Metric::CpuPower).clone();
                bound(&registry, Metric::CpuPower)?;
                cpu_power(&idle, &load)?
            }
            "memory_load" => {
                let ours = registry.snapshot().get(Metric::MemoryLoad).clone();
                bound(&registry, Metric::MemoryLoad)?;
                let reference =
                    i64::try_from((MEM_TOTAL_KB - MEM_AVAILABLE_KB) * 100_000 / MEM_TOTAL_KB)
                        .map_err(|e| e.to_string())?;
                memory_load(&ours, reference)?
            }
            "fans" => {
                let sys = self.tree.roots().sysfs;
                let inputs = fan_inputs(&sys);
                if inputs.len() != FANS.len() {
                    return Err(format!(
                        "{} fan inputs listed, not {}",
                        inputs.len(),
                        FANS.len()
                    ));
                }
                for input in &inputs {
                    let (_, rpm, _) = FANS
                        .iter()
                        .find(|(n, _, _)| *n == input.input)
                        .ok_or_else(|| format!("fan{} isn't the fixture's", input.input))?;
                    let rpm = i64::try_from(*rpm).map_err(|e| e.to_string())? * 1_000;
                    fan(&input.chip, input.input, &fan_rpm(&sys, input), rpm)?;
                }
                format!("{} fan inputs agree with the fixture", inputs.len())
            }
            _ => return Err(format!("no check {name}")),
        };
        Ok(format!("fake: {note}"))
    }

    /// The GPU of `capability`'s driver in the tree, chosen and checked.
    fn gpu(&self, capability: Capability) -> Result<String, String> {
        let (mut registry, clock) = self.registry();
        let ranked = |pci: &str, driver: &str, kind: RankedKind| -> Result<Ranked, String> {
            Ok(Ranked {
                pci: pci.parse()?,
                driver: driver.to_owned(),
                kind,
            })
        };
        let dedicated = || RankedKind::Dedicated {
            signal: "the fixture".into(),
        };
        let ranking = Ranking {
            gpus: vec![
                ranked(CARD, "amdgpu", dedicated())?,
                ranked(
                    IGPU,
                    "i915",
                    RankedKind::Integrated {
                        signal: "the fixture".into(),
                    },
                )?,
                ranked(CARD2, "xe", dedicated())?,
                ranked(NVIDIA, "nvidia", dedicated())?,
                ranked(NOUVEAU, "nouveau", RankedKind::NoSignal)?,
            ],
        };
        let sys = self.tree.roots().sysfs;
        let driver = capability.gpu_driver().unwrap_or("nouveau");
        let gpu = gpus(&sys, Some(&ranking))
            .into_iter()
            .find(|gpu| gpu.driver == driver)
            .ok_or_else(|| format!("no fake GPU on {driver}"))?;
        registry.choose_gpu(Some(&gpu));
        registry.snapshot();
        let hwmon_file = |name: &str| -> Result<String, String> {
            let dir = sys
                .join("bus/pci/devices")
                .join(gpu.pci.to_string())
                .join("hwmon");
            let hwmon = fs::read_dir(&dir)
                .map_err(|e| format!("{}: {e}", dir.display()))?
                .flatten()
                .next()
                .ok_or("no hwmon")?
                .file_name()
                .into_string()
                .map_err(|_| "a strange name")?;
            Ok(format!(
                "sys/devices/pci0000:00/{}/hwmon/{hwmon}/{name}",
                gpu.pci
            ))
        };
        let mut notes = Vec::new();
        match driver {
            "amdgpu" => {
                let snapshot = registry.snapshot();
                notes.push(close(
                    "Gpu temperature",
                    milli(snapshot.get(Metric::GpuTemperature))?,
                    41_000,
                    1_000,
                )?);
                notes.push(gpu_power(snapshot.get(Metric::GpuPower), 182_000)?);
                let busy = format!("sys/devices/pci0000:00/{}/gpu_busy_percent", gpu.pci);
                self.write(&busy, 97)?;
                let during = milli(registry.snapshot().get(Metric::GpuUsage))?;
                self.write(&busy, 4)?;
                let after = milli(registry.snapshot().get(Metric::GpuUsage))?;
                notes.push(gpu_usage(&[during], &[after])?);
            }
            "i915" | "xe" => {
                let (temp, energy): (i64, i64) = if driver == "i915" {
                    (44_000, 9_100_000)
                } else {
                    (47_000, 7_300_000)
                };
                let snapshot = registry.snapshot();
                notes.push(close(
                    "Gpu temperature",
                    milli(snapshot.get(Metric::GpuTemperature))?,
                    temp,
                    1_000,
                )?);
                notes.push(no_usage(snapshot.get(Metric::GpuUsage), driver)?);
                // The energy counter a second later: its rise is the power, in watts.
                let file = hwmon_file("energy1_input")?;
                clock.advance(Duration::from_secs(1));
                self.write(&file, energy * 2)?;
                let power = registry.snapshot().get(Metric::GpuPower).clone();
                // Microjoules over one second, in thousandths of a watt.
                notes.push(gpu_power(&power, energy / 1_000)?);
            }
            "nvidia" => {
                let snapshot = registry.snapshot();
                notes.push(nvml_plausible(
                    snapshot.get(Metric::GpuTemperature),
                    snapshot.get(Metric::GpuUsage),
                    snapshot.get(Metric::GpuPower),
                )?);
                notes.push(gpu_usage(&[milli(snapshot.get(Metric::GpuUsage))?], &[])?);
            }
            _ => {
                let snapshot = registry.snapshot();
                notes.push(no_readings(&snapshot_gpu(&snapshot), driver)?);
            }
        }
        Ok(format!("fake {}: {}", gpu.pci, notes.join("; ")))
    }
}

/// The source a metric is bound to, by name; an unbound one is an error with its reason.
fn bound<C: Clock>(registry: &Registry<C>, metric: Metric) -> Result<String, String> {
    match registry
        .bindings()
        .into_iter()
        .find(|(m, _)| *m == metric)
        .map(|(_, binding)| binding)
    {
        Some(Binding::Bound(source)) if !source.is_empty() => Ok(source),
        Some(Binding::Bound(_)) => Err(format!("{}: a source without a name", metric.spec_name())),
        Some(Binding::Unbound(reason)) => Err(format!("{}: unbound, {reason}", metric.spec_name())),
        None => Err(format!("{}: not in the bindings", metric.spec_name())),
    }
}

/// A reading's value in thousandths of its unit, or why it has none.
fn milli(reading: &Reading) -> Result<i64, String> {
    match reading {
        Reading::Value(value) => {
            let thousandths = i128::from(value.num()) * 1_000 / i128::from(value.den());
            i64::try_from(thousandths).map_err(|e| e.to_string())
        }
        Reading::Unavailable(reason) => Err(format!("no reading: {reason}")),
    }
}

/// Thousandths written as a decimal number.
fn show(milli: i64) -> String {
    let sign = if milli < 0 { "-" } else { "" };
    let (whole, part) = (milli.abs() / 1_000, milli.abs() % 1_000);
    if part == 0 {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{part:03}")
            .trim_end_matches('0')
            .to_owned()
    }
}

/// `ours` within `tolerance` of `reference`, all in thousandths.
fn close(what: &str, ours: i64, reference: i64, tolerance: i64) -> Result<String, String> {
    if (ours - reference).abs() <= tolerance {
        Ok(format!("{what} {} ≈ {}", show(ours), show(reference)))
    } else {
        Err(format!(
            "{what} {} against {}: more than {} apart",
            show(ours),
            show(reference),
            show(tolerance)
        ))
    }
}

/// CPU Usage: at least 95 % under the load, and within 5 points of `top`'s busy share.
fn cpu_usage(ours: &Reading, top: Result<i64, String>) -> Result<String, String> {
    let ours = milli(ours)?;
    if ours < 95_000 {
        return Err(format!(
            "CPU Usage {} % under the load, below 95 %",
            show(ours)
        ));
    }
    close("CPU Usage under load, %", ours, top?, 5_000)
}

/// CPU Power: between 1 W and 1000 W, and higher under the load than at idle.
fn cpu_power(idle: &Reading, load: &Reading) -> Result<String, String> {
    let (idle, load) = (milli(idle)?, milli(load)?);
    for watts in [idle, load] {
        if !(1_000..=1_000_000).contains(&watts) {
            return Err(format!("CPU Power {} W, outside 1–1000 W", show(watts)));
        }
    }
    if load <= idle {
        return Err(format!(
            "CPU Power {} W under the load, not above {} W at idle",
            show(load),
            show(idle)
        ));
    }
    Ok(format!(
        "CPU Power {} W at idle, {} W under load",
        show(idle),
        show(load)
    ))
}

/// Memory Load within half a point of the reference share.
fn memory_load(ours: &Reading, reference: i64) -> Result<String, String> {
    close("Memory Load, %", milli(ours)?, reference, 500)
}

/// A fan input within 3 % of the reference, both in thousandths of RPM.
fn fan(chip: &str, n: u32, ours: &Reading, reference: i64) -> Result<(), String> {
    let ours = milli(ours).map_err(|e| format!("{chip} fan{n}: {e}"))?;
    let tolerance = reference.abs() * 3 / 100;
    close(&format!("{chip} fan{n}, RPM"), ours, reference, tolerance).map(|_| ())
}

/// Gpu power within 10 % or 5 W of the reference, whichever is larger, in thousandths of a watt.
fn gpu_power(ours: &Reading, reference: i64) -> Result<String, String> {
    let tolerance = (reference.abs() / 10).max(5_000);
    close("Gpu power, W", milli(ours)?, reference, tolerance)
}

/// Gpu usage: at least 90 % under the load, then below 50 % within the time after it.
fn gpu_usage(during: &[i64], after: &[i64]) -> Result<String, String> {
    let peak = during
        .iter()
        .copied()
        .max()
        .ok_or("no reading under the load")?;
    if peak < 90_000 {
        return Err(format!(
            "Gpu usage peaked at {} % under the load, below 90 %",
            show(peak)
        ));
    }
    match after.last() {
        None => Ok(format!("Gpu usage {} % under load", show(peak))),
        Some(&last) if last < 50_000 => Ok(format!(
            "Gpu usage {} % under load, {} % after",
            show(peak),
            show(last)
        )),
        Some(&last) => Err(format!(
            "Gpu usage still {} % 5 s after the load stopped",
            show(last)
        )),
    }
}

/// The reading is unavailable for `expected`, as the rules say.
fn unavailable(reading: &Reading, expected: &Unavailable) -> Result<String, String> {
    match reading {
        Reading::Unavailable(reason) if reason == expected => Ok(format!("unavailable: {reason}")),
        other => Err(format!("{other:?}, not unavailable for {expected:?}")),
    }
}

/// Gpu usage on Intel's drivers: unavailable, with no rule for the driver.
fn no_usage(reading: &Reading, driver: &str) -> Result<String, String> {
    unavailable(
        reading,
        &Unavailable::NoRule {
            driver: driver.to_owned(),
        },
    )
    .map(|note| format!("Gpu usage {note}"))
}

/// A driver without rules: every GPU metric unavailable, with that reason.
fn no_readings(readings: &[Reading; 3], driver: &str) -> Result<String, String> {
    let expected = Unavailable::NoRule {
        driver: driver.to_owned(),
    };
    for reading in readings {
        unavailable(reading, &expected)?;
    }
    Ok(format!("{driver}: no GPU reading, as the rules say"))
}

/// NVML's three readings: values in plausible ranges.
fn nvml_plausible(temp: &Reading, usage: &Reading, power: &Reading) -> Result<String, String> {
    let (temp, usage, power) = (milli(temp)?, milli(usage)?, milli(power)?);
    if !(0..=120_000).contains(&temp)
        || !(0..=100_000).contains(&usage)
        || !(0..=1_000_000).contains(&power)
    {
        return Err(format!(
            "NVML gave {} °C, {} %, {} W",
            show(temp),
            show(usage),
            show(power)
        ));
    }
    Ok(format!(
        "NVML {} °C, {} %, {} W",
        show(temp),
        show(usage),
        show(power)
    ))
}

/// The effective user ID, from `/proc/self/status` (proc_pid_status(5)).
fn euid() -> Result<u32, String> {
    fs::read_to_string("/proc/self/status")
        .map_err(|e| e.to_string())?
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|ids| ids.split_whitespace().nth(1))
        .and_then(|id| id.parse().ok())
        .ok_or_else(|| "no effective uid".to_owned())
}

/// `sensors -j`'s output.
fn sensors_json(sensors: Result<&PathBuf, &String>) -> Result<Json, String> {
    let sensors = sensors.map_err(|e| format!("needs sensors: {e}"))?;
    let output = Command::new(sensors)
        .arg("-j")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", sensors.display()))?;
    json::parse(&String::from_utf8_lossy(&output.stdout)).map_err(|e| format!("sensors -j: {e}"))
}

/// A JSON number in thousandths, from its text: no floating point.
fn number_milli(text: &str) -> Option<i64> {
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text),
    };
    let (whole, part) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !part.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let part: String = part.chars().chain("000".chars()).take(3).collect();
    Some(sign * (whole.parse::<i64>().ok()? * 1_000 + part.parse::<i64>().ok()?))
}

/// The members of `sensors -j`'s chip `name`.
fn chip<'a>(json: &'a Json, name: &str) -> Option<&'a [(String, Json)]> {
    let Json::Object(chips) = json else {
        return None;
    };
    chips.iter().find_map(|(chip, members)| match members {
        Json::Object(members) if chip == name => Some(members.as_slice()),
        _ => None,
    })
}

/// In a chip's features, or only in `feature`, the first subfeature `wanted` accepts, in
/// thousandths.
fn feature_value(
    members: &[(String, Json)],
    feature: Option<&str>,
    wanted: impl Fn(&str) -> bool,
) -> Option<i64> {
    members.iter().find_map(|(name, value)| {
        let Json::Object(subfeatures) = value else {
            return None;
        };
        if feature.is_some_and(|f| f != name) {
            return None;
        }
        subfeatures.iter().find_map(|(sub, value)| match value {
            Json::Number(text) if wanted(sub) => number_milli(text),
            _ => None,
        })
    })
}

fn chip_value(
    json: &Json,
    name: &str,
    feature: Option<&str>,
    wanted: impl Fn(&str) -> bool,
) -> Option<i64> {
    feature_value(chip(json, name)?, feature, wanted)
}

/// The CPU package temperature `sensors -j` shows: coretemp's `Package id 0`, or k10temp's
/// `Tdie`, else `Tctl`, as the CPU temperature rules read them.
fn cpu_package(json: &Json) -> Result<i64, String> {
    let Json::Object(chips) = json else {
        return Err("sensors -j isn't an object".into());
    };
    let input = |s: &str| s.starts_with("temp") && s.ends_with("_input");
    for (name, members) in chips {
        let Json::Object(members) = members else {
            continue;
        };
        let found = if name.starts_with("coretemp-") {
            feature_value(members, Some("Package id 0"), input)
        } else if name.starts_with("k10temp-") {
            feature_value(members, Some("Tdie"), input)
                .or_else(|| feature_value(members, Some("Tctl"), input))
        } else {
            None
        };
        if let Some(found) = found {
            return Ok(found);
        }
    }
    Err("sensors shows no CPU package temperature".into())
}

/// libsensors' name for a PCI device's chip: the hwmon name, `pci`, and the address as
/// `(domain << 16) + (bus << 8) + (device << 3) + function` in hexadecimal (lm-sensors,
/// `lib/sysfs.c`); on the owner's PC, domain 0 only.
fn pci_chip(prefix: &str, pci: PciAddr) -> String {
    let text = pci.to_string();
    let number = |part: &str| u32::from_str_radix(part, 16).unwrap_or(0);
    let parts: Vec<&str> = text.split([':', '.']).collect();
    let address = match parts.as_slice() {
        [domain, bus, device, function] => {
            (number(domain) << 16) + (number(bus) << 8) + (number(device) << 3) + number(function)
        }
        _ => 0,
    };
    format!("{prefix}-pci-{address:04x}")
}

/// The GPU's main temperature in `sensors -j`, as its rule reads it: amdgpu's `edge`, i915's
/// `temp1_input`, xe's `temp2_input`.
fn gpu_temperature(json: &Json, chip_name: &str, driver: &str) -> Result<i64, String> {
    let members =
        chip(json, chip_name).ok_or_else(|| format!("sensors shows no chip {chip_name}"))?;
    let found = match driver {
        "amdgpu" => feature_value(members, Some("edge"), |s| {
            s.starts_with("temp") && s.ends_with("_input")
        }),
        "i915" => feature_value(members, None, |s| s == "temp1_input"),
        _ => feature_value(members, None, |s| s == "temp2_input"),
    };
    found.ok_or_else(|| format!("sensors shows no GPU temperature for {chip_name}"))
}

/// `sensors -j`'s reading of fan `n` of the one chip named `chip-…`.
fn fan_reference(json: &Json, chip_name: &str, n: u32) -> Result<i64, String> {
    let Json::Object(chips) = json else {
        return Err("sensors -j isn't an object".into());
    };
    let subfeature = format!("fan{n}_input");
    let found: Vec<i64> = chips
        .iter()
        .filter(|(name, _)| name.starts_with(&format!("{chip_name}-")))
        .filter_map(|(_, members)| match members {
            Json::Object(members) => feature_value(members, None, |s| s == subfeature),
            _ => None,
        })
        .collect();
    match found.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("sensors shows no {subfeature} for {chip_name}")),
        _ => Err(format!(
            "sensors shows {subfeature} for more than one {chip_name}"
        )),
    }
}

/// `free -b`'s used share, (total − available) ÷ total, in thousandths of a percent.
fn free_load(free: &Path) -> Result<i64, String> {
    let output = Command::new(free)
        .arg("-b")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", free.display()))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let numbers: Vec<i128> = text
        .lines()
        .find_map(|line| line.strip_prefix("Mem:"))
        .ok_or("free -b has no Mem: line")?
        .split_whitespace()
        .map(|n| n.parse().map_err(|_| format!("free -b: {n:?}")))
        .collect::<Result<_, _>>()?;
    let (total, available) = match numbers.as_slice() {
        [total, _, _, _, _, available, ..] if *total > 0 => (*total, *available),
        _ => return Err("free -b's Mem: line isn't total … available".into()),
    };
    i64::try_from((total - available) * 100_000 / total).map_err(|e| e.to_string())
}

/// `top`'s busy share over one second, in thousandths of a percent: 100 − idle − iowait, as CPU
/// Usage counts it, from the second `%Cpu(s)` line of `top -b -n 2 -d 1`.
fn top_busy(top: &Path) -> Result<i64, String> {
    let output = Command::new(top)
        .args(["-b", "-n", "2", "-d", "1"])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", top.display()))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text
        .lines()
        .filter_map(|line| line.strip_prefix("%Cpu(s):"))
        .nth(1)
        .ok_or("top shows no second %Cpu(s) line")?;
    let share = |key: &str| {
        line.split(',')
            .find_map(|field| {
                let (value, name) = field.trim().split_once(' ')?;
                (name.trim() == key).then(|| number_milli(value.trim()))?
            })
            .ok_or_else(|| format!("top's %Cpu(s) line has no {key}"))
    };
    Ok(100_000 - share("id")? - share("wa")?)
}

/// A busy thread on every logical CPU until dropped.
struct CpuLoad {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    started: Instant,
}

impl CpuLoad {
    fn start() -> CpuLoad {
        let stop = Arc::new(AtomicBool::new(false));
        let cpus = thread::available_parallelism().map_or(1, usize::from);
        let threads = (0..cpus)
            .map(|_| {
                let stop = Arc::clone(&stop);
                thread::spawn(move || {
                    let mut x: u64 = 1;
                    while !stop.load(Ordering::Relaxed) {
                        x = hint::black_box(
                            x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1),
                        );
                    }
                })
            })
            .collect();
        CpuLoad {
            stop,
            threads,
            started: Instant::now(),
        }
    }
}

impl Drop for CpuLoad {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/// The GPU's render node in `<dev>/dri`, as sysfs names it.
fn render_node(sys: &Path, dev: &Path, pci: PciAddr) -> Result<PathBuf, String> {
    let drm = sys
        .join("bus/pci/devices")
        .join(pci.to_string())
        .join("drm");
    let mut names: Vec<String> = fs::read_dir(&drm)
        .map_err(|e| format!("{}: {e}", drm.display()))?
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("renderD"))
        .collect();
    names.sort();
    let name = names
        .first()
        .ok_or_else(|| format!("{}: no render node", drm.display()))?;
    Ok(dev.join("dri").join(name))
}

/// `ffmpeg` loading one GPU with a Vulkan filter until dropped. The Vulkan device is derived
/// from the GPU's DRM render node, so the load lands on that GPU.
struct GpuLoad(Child);

impl GpuLoad {
    fn start(ffmpeg: &Path, node: &Path) -> Result<GpuLoad, String> {
        Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-init_hw_device",
            ])
            .arg(format!("drm=odc:{}", node.display()))
            .args([
                "-init_hw_device",
                "vulkan=odcvk@odc",
                "-filter_hw_device",
                "odcvk",
                "-f",
                "lavfi",
                "-i",
                "color=c=gray:s=1920x1080:r=1000",
                "-vf",
                "format=yuv420p,hwupload,nlmeans_vulkan",
                "-f",
                "null",
                "-",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(GpuLoad)
            .map_err(|e| format!("{}: {e}", ffmpeg.display()))
    }

    /// An error when `ffmpeg` stopped by itself, so the load didn't last.
    fn still_running(&mut self) -> Result<(), String> {
        match self.0.try_wait() {
            Ok(None) => Ok(()),
            Ok(Some(status)) => Err(format!("ffmpeg's Vulkan load ended early: {status}")),
            Err(e) => Err(e.to_string()),
        }
    }
}

impl Drop for GpuLoad {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_numbers_read_in_thousandths_without_floating_point() {
        assert_eq!(number_milli("865.000"), Some(865_000));
        assert_eq!(number_milli("41.5"), Some(41_500));
        assert_eq!(number_milli("-0.250"), Some(-250));
        assert_eq!(number_milli("23"), Some(23_000));
        assert_eq!(number_milli("1.23456"), Some(1_234));
        for bad in ["", ".5", "1e3", "x"] {
            assert_eq!(number_milli(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn pci_chips_are_named_as_libsensors_names_them() {
        let card: PciAddr = "0000:03:00.0".parse().expect("an address");
        assert_eq!(pci_chip("amdgpu", card), "amdgpu-pci-0300");
        let igpu: PciAddr = "0000:00:02.0".parse().expect("an address");
        assert_eq!(pci_chip("i915", igpu), "i915-pci-0010");
    }

    #[test]
    fn sensors_json_gives_the_values_the_rules_read() {
        let text = r#"{
  "coretemp-isa-0000": {"Adapter": "ISA adapter", "Package id 0": {"temp1_input": 57.000}, "Core 0": {"temp2_input": 53.000}},
  "fakeboardchip-isa-0a20": {"fan1": {"fan1_input": 1234.000}, "fan2": {"fan2_input": 3456.000}},
  "amdgpu-pci-0a00": {"edge": {"temp1_input": 41.000}, "junction": {"temp2_input": 77.000}, "PPT": {"power1_average": 182.000}}
}"#;
        let json = json::parse(text).expect("JSON");
        assert_eq!(cpu_package(&json), Ok(57_000));
        assert_eq!(fan_reference(&json, "fakeboardchip", 2), Ok(3_456_000));
        assert!(fan_reference(&json, "fakeboardchip", 3).is_err());
        assert_eq!(
            gpu_temperature(&json, "amdgpu-pci-0a00", "amdgpu"),
            Ok(41_000)
        );
        assert_eq!(
            chip_value(&json, "amdgpu-pci-0a00", None, |s| s == "power1_average"),
            Some(182_000)
        );
    }

    #[test]
    fn the_thresholds_are_the_plans() {
        let value =
            |milli: i64| Reading::Value(odc_sensors::Exact::new(milli, 1_000).expect("exact"));
        assert!(cpu_usage(&value(97_000), Ok(99_000)).is_ok());
        assert!(cpu_usage(&value(94_000), Ok(94_000)).is_err(), "below 95 %");
        assert!(
            cpu_usage(&value(96_000), Ok(90_000)).is_err(),
            "6 points from top"
        );
        assert!(cpu_power(&value(6_000), &value(88_000)).is_ok());
        assert!(
            cpu_power(&value(88_000), &value(6_000)).is_err(),
            "not above idle"
        );
        assert!(cpu_power(&value(500), &value(88_000)).is_err(), "below 1 W");
        assert!(memory_load(&value(62_400), 62_500).is_ok());
        assert!(memory_load(&value(61_900), 62_500).is_err());
        assert!(fan("chip", 1, &value(1_000_000), 1_030_000).is_ok());
        assert!(fan("chip", 1, &value(1_000_000), 1_040_000).is_err());
        assert!(gpu_power(&value(30_000), 25_000).is_ok(), "5 W");
        assert!(gpu_power(&value(200_000), 182_000).is_ok(), "10 %");
        assert!(gpu_power(&value(210_000), 182_000).is_err());
        assert!(gpu_usage(&[50_000, 95_000], &[30_000]).is_ok());
        assert!(gpu_usage(&[85_000], &[]).is_err(), "below 90 %");
        assert!(gpu_usage(&[95_000], &[60_000]).is_err(), "didn't drop");
    }
}
