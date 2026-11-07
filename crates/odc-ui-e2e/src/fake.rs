//! The fake service's files (`odc_testkit::fake_service`; Phase 10 plan, Task 8) as the driver
//! writes and reads them: the state file, in the contract's keys (`docs/spec/dbus.md`, State)
//! and the driver's own words for them, and the call log. Every chip, GPU and PCI ID here is
//! made up (AGENTS.md rule 10).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::driver::{Result, wait};

/// A GPU of `Gpus`.
#[derive(Clone, Debug, PartialEq)]
pub struct Gpu {
    pub pci: &'static str,
    pub driver: &'static str,
    pub kind: &'static str,
    pub vendor: u16,
    pub device: u16,
    pub has_rule: bool,
}

/// A fan input of `FanInputs`; `None` for a speed that can't be read.
#[derive(Clone, Debug, PartialEq)]
pub struct Fan {
    pub input: u32,
    pub label: &'static str,
    pub rpm: Option<u16>,
}

/// The made-up fan chip every fan input is on.
pub const CHIP: &str = "acme6000";
pub const DEVICE: &str = "devices/platform/acme6000.2592";

/// The made-up GPUs, in the service's order (dedicated, integrated, then unknown, each by PCI
/// address). Their vendor IDs are listed in no `pci.ids` (checked at the start of a run), so
/// each is labelled `<kind> · <driver> · <address>` (`docs/design/settings-app.md`, GPU).
pub const DEDICATED: Gpu = Gpu {
    pci: "0000:0a:00.0",
    driver: "acmegpu",
    kind: "dedicated",
    vendor: 0x0002,
    device: 0x0001,
    has_rule: true,
};
pub const INTEGRATED: Gpu = Gpu {
    pci: "0000:00:02.0",
    driver: "acmegfx",
    kind: "integrated",
    vendor: 0x0003,
    device: 0x0001,
    has_rule: true,
};
/// No reading rule: listed last, with "no readings".
pub const NO_RULE: Gpu = Gpu {
    pci: "0000:07:00.0",
    driver: "othergpu",
    kind: "unknown",
    vendor: 0x0003,
    device: 0x0002,
    has_rule: false,
};
/// Its driver couldn't be asked for its kind.
pub const QUERY_FAILED: Gpu = Gpu {
    pci: "0000:0b:00.0",
    driver: "othergpu",
    kind: "query-failed",
    vendor: 0x0002,
    device: 0x0002,
    has_rule: true,
};

/// The made-up vendor IDs, which no `pci.ids` may list.
pub const VENDORS: [u16; 2] = [0x0002, 0x0003];

/// The service's state, as the driver means it.
#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub mode: u32,
    pub rotation: u8,
    pub unit: &'static str,
    /// `CoolerState`, any text, so a value the app doesn't know can be sent.
    pub cooler: &'static str,
    pub available: Vec<u32>,
    pub reasons: BTreeMap<u32, String>,
    pub gpus: Vec<Gpu>,
    pub chosen_gpu: &'static str,
    pub gpu_in_use: &'static str,
    pub fans: Vec<Fan>,
    /// Chosen fan inputs, by input number.
    pub cooler_fan: Option<u32>,
    pub pump: Option<u32>,
    pub cooler_fan_present: bool,
    pub pump_present: bool,
    pub peaks: bool,
    pub bar_fall: &'static str,
    pub peak_fall: &'static str,
    pub audio_connected: bool,
}

impl Service {
    /// A first start (`docs/design/settings-app.md`, States): Load · CPU, 0°, °C, no inputs,
    /// peak dots on, Moderate and Slow, the GPU on Automatic; here with two GPUs, so the GPU row
    /// shows, and three fan inputs.
    pub fn first_start() -> Service {
        Service {
            mode: 1,
            rotation: 0,
            unit: "celsius",
            cooler: "connected",
            available: (1..=23).collect(),
            reasons: BTreeMap::new(),
            gpus: vec![DEDICATED, INTEGRATED],
            chosen_gpu: "",
            gpu_in_use: DEDICATED.pci,
            fans: vec![
                Fan {
                    input: 1,
                    label: "",
                    rpm: Some(1180),
                },
                Fan {
                    input: 2,
                    label: "",
                    rpm: Some(3400),
                },
                Fan {
                    input: 3,
                    label: "Rear",
                    rpm: Some(940),
                },
            ],
            cooler_fan: None,
            pump: None,
            cooler_fan_present: true,
            pump_present: true,
            peaks: true,
            bar_fall: "moderate",
            peak_fall: "slow",
            audio_connected: true,
        }
    }

    /// The speed of input `n`, as `CoolerFanRpm` and `PumpRpm` give it.
    fn rpm(&self, chosen: Option<u32>, present: bool) -> f64 {
        let speed = chosen
            .filter(|_| present)
            .and_then(|n| self.fans.iter().find(|fan| fan.input == n))
            .and_then(|fan| fan.rpm);
        speed.map_or(f64::NAN, f64::from)
    }

    fn table(&self) -> toml::Table {
        use toml::Value::{Array, Boolean, Float, Integer, String as Text};
        let text = |t: &str| Text(t.to_owned());
        let int = |n: u32| Integer(i64::from(n));
        let input = |chosen: Option<u32>| match chosen {
            Some(n) => Array(vec![text(CHIP), text(DEVICE), int(n)]),
            None => Array(vec![text(""), text(""), int(0)]),
        };
        let mut table = toml::Table::new();
        let mut put = |key: &str, value: toml::Value| {
            table.insert(key.to_owned(), value);
        };
        put("Mode", int(self.mode));
        put("Rotation", int(u32::from(self.rotation)));
        put("TemperatureUnit", text(self.unit));
        put("CoolerConnected", Boolean(self.cooler == "connected"));
        put("CoolerState", text(self.cooler));
        put(
            "AvailableModes",
            Array(self.available.iter().map(|&m| int(m)).collect()),
        );
        put(
            "UnavailableReasons",
            toml::Value::Table(
                self.reasons
                    .iter()
                    .map(|(mode, reason)| (mode.to_string(), text(reason)))
                    .collect(),
            ),
        );
        put(
            "Gpus",
            Array(
                self.gpus
                    .iter()
                    .map(|gpu| {
                        Array(vec![
                            text(gpu.pci),
                            text(gpu.driver),
                            text(gpu.kind),
                            int(u32::from(gpu.vendor)),
                            int(u32::from(gpu.device)),
                            Boolean(gpu.has_rule),
                        ])
                    })
                    .collect(),
            ),
        );
        put("ChosenGpu", text(self.chosen_gpu));
        put("GpuInUse", text(self.gpu_in_use));
        put(
            "FanInputs",
            Array(
                self.fans
                    .iter()
                    .map(|fan| {
                        Array(vec![
                            text(CHIP),
                            text(DEVICE),
                            int(fan.input),
                            text(fan.label),
                            Float(fan.rpm.map_or(f64::NAN, f64::from)),
                        ])
                    })
                    .collect(),
            ),
        );
        put("CoolerFanInput", input(self.cooler_fan));
        put("PumpInput", input(self.pump));
        put("CoolerFanInputPresent", Boolean(self.cooler_fan_present));
        put("PumpInputPresent", Boolean(self.pump_present));
        put(
            "CoolerFanRpm",
            Float(self.rpm(self.cooler_fan, self.cooler_fan_present)),
        );
        put("PumpRpm", Float(self.rpm(self.pump, self.pump_present)));
        put("MusicPeaks", Boolean(self.peaks));
        put("MusicBarFall", text(self.bar_fall));
        put("MusicPeakFall", text(self.peak_fall));
        put("AudioConnected", Boolean(self.audio_connected));
        put("AudioWanted", Boolean((13..=23).contains(&self.mode)));
        table
    }
}

/// The fake: what it answers, and its state.
pub struct Fake {
    dir: PathBuf,
    serial: u64,
    /// Owns the service's bus name.
    pub on_bus: bool,
    /// Answers reads and watches; false refuses them with `NotAuthorized`.
    pub reads: bool,
    /// `<error>: <text>`: every change and Refresh refused so.
    pub refuse: Option<String>,
    pub service: Service,
}

pub const STATE_FILE: &str = "state.toml";
pub const CALL_LOG: &str = "calls.log";

/// The reads and watches, refused or not.
const READS: [&str; 6] = [
    "Watch()",
    "Unwatch()",
    "WatchFrames()",
    "UnwatchFrames()",
    "GetState()",
    "Diagnostics()",
];

impl Fake {
    /// The fake whose files are in `dir`; it serves nothing until [`Fake::write`].
    pub fn new(dir: &Path) -> Fake {
        Fake {
            dir: dir.to_owned(),
            serial: 0,
            on_bus: false,
            reads: true,
            refuse: None,
            service: Service::first_start(),
        }
    }

    /// Writes the state file, then waits until the fake has taken it: its watchers have the
    /// changed keys, and its bus name is taken or given up.
    pub fn write(&mut self) -> Result<()> {
        self.serial += 1;
        let mut fake = toml::Table::new();
        fake.insert(
            "serial".to_owned(),
            toml::Value::Integer(i64::try_from(self.serial).unwrap_or(i64::MAX)),
        );
        fake.insert("on_bus".to_owned(), toml::Value::Boolean(self.on_bus));
        fake.insert("reads".to_owned(), toml::Value::Boolean(self.reads));
        if let Some(refuse) = &self.refuse {
            fake.insert("refuse".to_owned(), toml::Value::String(refuse.clone()));
        }
        let mut file = toml::Table::new();
        file.insert("fake".to_owned(), toml::Value::Table(fake));
        file.insert("state".to_owned(), toml::Value::Table(self.service.table()));
        let text = toml::to_string(&file).map_err(|e| format!("the state file: {e}"))?;
        let next = self.dir.join(format!("{STATE_FILE}.new"));
        fs::write(&next, text).map_err(|e| format!("{}: {e}", next.display()))?;
        fs::rename(&next, self.dir.join(STATE_FILE)).map_err(|e| format!("the state file: {e}"))?;
        let taken = format!("state {}", self.serial);
        wait(&format!("the fake taking {taken}"), || {
            let log = self.log()?;
            if let Some(error) = log.iter().rev().find(|l| l.starts_with("state error")) {
                return Err(error.clone());
            }
            Ok(log.contains(&taken).then_some(()))
        })
    }

    fn log(&self) -> Result<Vec<String>> {
        let path = self.dir.join(CALL_LOG);
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(text.lines().map(str::to_owned).collect())
    }

    /// Where the call log ends now.
    pub fn mark(&self) -> Result<usize> {
        Ok(self.log()?.len())
    }

    /// The calls logged since `mark`, the fake's own `state` lines left out.
    pub fn calls_since(&self, mark: usize) -> Result<Vec<String>> {
        Ok(self
            .log()?
            .into_iter()
            .skip(mark)
            .filter(|line| !line.starts_with("state "))
            .collect())
    }

    /// The changes and Refreshes logged since `mark`: every call but the reads and watches,
    /// which the app repeats whenever it connects again (`docs/design/settings-app.md`, Talking
    /// to the service).
    pub fn changes_since(&self, mark: usize) -> Result<Vec<String>> {
        Ok(self
            .calls_since(mark)?
            .into_iter()
            .filter(|call| !READS.iter().any(|read| call.starts_with(read)))
            .collect())
    }

    /// Waits until `call` is logged after `mark`.
    pub fn called(&self, mark: usize, call: &str) -> Result<()> {
        wait(&format!("{call} at the fake"), || {
            Ok(self
                .calls_since(mark)?
                .iter()
                .any(|c| c == call)
                .then_some(()))
        })
        .map_err(|e| {
            let calls = self.calls_since(mark).unwrap_or_default();
            format!("{e}; calls since: {calls:?}")
        })
    }
}
