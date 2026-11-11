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
    /// The preview frame sent to frame watchers: 196 dots, `0` or `1`, row by row.
    pub frame: Option<String>,
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
            frame: None,
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
        if let Some(frame) = &self.frame {
            fake.insert("frame".to_owned(), toml::Value::String(frame.clone()));
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

/// The fake systemd's files (`odc_testkit::fake_systemd`; Phase 11 plan, Task 3): its state
/// file and its call log, and the fake `pkexec`'s call log and answer, in one folder.
pub const SYSTEMD_FILE: &str = "systemd.toml";
pub const SYSTEMD_LOG: &str = "systemd.log";
pub const PKEXEC_LOG: &str = "pkexec.log";
pub const PKEXEC_ANSWER: &str = "pkexec.answer";

/// The two units and the session, as the driver means them: each value as systemd gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Units {
    /// `opendeepcool.service`: its file state ("" fails the read), `ActiveState`, `SubState`,
    /// and how long its file state takes to answer.
    pub service: (&'static str, &'static str, &'static str),
    pub delay_ms: u64,
    /// `opendeepcool-audio.service`: its file state and `ActiveState`, and its jobs' result
    /// ("" fails the call).
    pub sound: (&'static str, &'static str),
    pub job: &'static str,
    /// logind's session: active, and remote.
    pub session: (bool, bool),
}

impl Units {
    /// A new install: what the fake answers before its first file.
    pub fn new_install() -> Units {
        Units {
            service: ("disabled", "inactive", "dead"),
            delay_ms: 0,
            sound: ("disabled", "inactive"),
            job: "done",
            session: (true, false),
        }
    }

    /// OpenDeepCool ready to use: all four facts hold.
    pub fn all_on() -> Units {
        Units {
            service: ("enabled", "active", "running"),
            sound: ("enabled", "active"),
            ..Units::new_install()
        }
    }

    fn text(&self, serial: u64) -> String {
        format!(
            "serial = {serial}\n\n[service]\nfile = {:?}\nactive = {:?}\nsub = {:?}\n\
             delay_ms = {}\n\n[sound]\nfile = {:?}\nactive = {:?}\njob = {:?}\n\n\
             [session]\nactive = {}\nremote = {}\n",
            self.service.0,
            self.service.1,
            self.service.2,
            self.delay_ms,
            self.sound.0,
            self.sound.1,
            self.job,
            self.session.0,
            self.session.1
        )
    }
}

/// The fake systemd and the fake `pkexec`.
pub struct Systemd {
    dir: PathBuf,
    serial: u64,
    pub units: Units,
}

impl Systemd {
    /// The fake whose files are in `dir`; it answers as a new install until [`Systemd::write`].
    pub fn new(dir: &Path) -> Systemd {
        Systemd {
            dir: dir.to_owned(),
            serial: 0,
            units: Units::new_install(),
        }
    }

    /// Writes the state file, then waits until the fake has taken it.
    pub fn write(&mut self) -> Result<()> {
        self.serial += 1;
        let next = self.dir.join(format!("{SYSTEMD_FILE}.new"));
        fs::write(&next, self.units.text(self.serial))
            .map_err(|e| format!("{}: {e}", next.display()))?;
        fs::rename(&next, self.dir.join(SYSTEMD_FILE))
            .map_err(|e| format!("the systemd file: {e}"))?;
        let taken = format!("state {}", self.serial);
        wait(&format!("the fake systemd taking {taken}"), || {
            let log = self.log(SYSTEMD_LOG)?;
            if let Some(error) = log.iter().rev().find(|l| l.starts_with("state error")) {
                return Err(error.clone());
            }
            Ok(log.contains(&taken).then_some(()))
        })
    }

    fn log(&self, name: &str) -> Result<Vec<String>> {
        let path = self.dir.join(name);
        match fs::read_to_string(&path) {
            Ok(text) => Ok(text.lines().map(str::to_owned).collect()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Where the systemd and `pkexec` logs end now.
    pub fn mark(&self) -> Result<(usize, usize)> {
        Ok((self.log(SYSTEMD_LOG)?.len(), self.log(PKEXEC_LOG)?.len()))
    }

    /// The systemd calls logged since `mark`, the fake's own `state` lines left out.
    pub fn calls_since(&self, mark: (usize, usize)) -> Result<Vec<String>> {
        Ok(self
            .log(SYSTEMD_LOG)?
            .into_iter()
            .skip(mark.0)
            .filter(|line| !line.starts_with("state "))
            .collect())
    }

    /// The `pkexec` runs logged since `mark`.
    pub fn pkexec_since(&self, mark: (usize, usize)) -> Result<Vec<String>> {
        Ok(self.log(PKEXEC_LOG)?.into_iter().skip(mark.1).collect())
    }

    /// Waits until `pkexec` ran with `args` after `mark`, and is the only run since.
    pub fn pkexec_ran(&self, mark: (usize, usize), args: &str) -> Result<()> {
        let want = format!("pkexec {args}");
        wait(&format!("{want:?} at the fake pkexec"), || {
            let runs = self.pkexec_since(mark)?;
            match runs.as_slice() {
                [] => Ok(None),
                [run] if *run == want => Ok(Some(())),
                _ => Err(format!("the fake pkexec ran {runs:?}")),
            }
        })
    }

    /// Waits until `call` is logged after `mark`.
    pub fn called(&self, mark: (usize, usize), call: &str) -> Result<()> {
        wait(&format!("{call} at the fake systemd"), || {
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

    /// Ends the waiting fake `pkexec` with `code`, writing `stderr`.
    pub fn answer(&self, code: i32, stderr: &str) -> Result<()> {
        let next = self.dir.join(format!("{PKEXEC_ANSWER}.new"));
        fs::write(&next, format!("exit {code}\n{stderr}"))
            .map_err(|e| format!("{}: {e}", next.display()))?;
        fs::rename(&next, self.dir.join(PKEXEC_ANSWER)).map_err(|e| format!("the answer: {e}"))?;
        wait("the fake pkexec taking its answer", || {
            Ok((!self.dir.join(PKEXEC_ANSWER).exists()).then_some(()))
        })
    }
}
