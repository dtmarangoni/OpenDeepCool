//! The scenarios (Phase 10 plan, Task 8): the rows of the design's coverage check and States
//! table (`docs/design/settings-app.md`) that AT-SPI can reach, each through the real app.
//!
//! Every change is checked as the design's Refused changes asks: the control's action, then
//! exactly one call at the fake, then the control still on the saved value until the driver,
//! as the service, confirms the change with a new state, then the control on the new value. A
//! Start at boot flip is checked the same way (Phase 11 plan, Task 3): the switch's action, then
//! exactly one run of the fake `pkexec`, the switch changing until the driver, as the program,
//! writes what the program leaves at the fake systemd and answers, then what the app does.
//! Every expected word is typed here from the design, apart from the app's code. Each scenario
//! starts from the first start's state, on the Display page, and leaves the app usable.

use std::fs;

use std::path::PathBuf;

use std::process::{Command, Stdio};

use std::thread;

use std::time::{Duration, Instant};

use atspi::State;

use crate::Paths;

use crate::driver::{Driver, Node, Result, wait};

use crate::fake::{
    self, DEDICATED, Fake, INTEGRATED, NO_RULE, QUERY_FAILED, Service, Systemd, Units,
};

mod basics;
mod boot;
mod settings;
mod states;
mod theme;

/// How long a control must stay on the saved value after its call, without the service's word.
const HOLD: Duration = Duration::from_millis(250);

/// Every mode, in order: 17 is no mode (DeepCreative's Emoji 4, dropped on 2026-11-23). This crate
/// links no other of the workspace's, so it keeps its own list, which the design's tile table pins.
pub const MODES: [u32; 25] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 22, 23, 24, 25, 26,
];

/// The design's tiles (Display page, Tiles), in the order of [`MODES`]: group and option.
pub const TILES: [(&str, &str); 25] = [
    ("Load", "CPU"),
    ("Load", "GPU"),
    ("Load", "RAM"),
    ("Load", "CPU + GPU"),
    ("Load", "CPU + RAM"),
    ("Load", "GPU + RAM"),
    ("Temperature", "CPU"),
    ("Temperature", "GPU"),
    ("Temperature", "CPU + GPU"),
    ("Power", "CPU"),
    ("Power", "GPU"),
    ("Power", "CPU + GPU"),
    ("Music", "Spectrum"),
    ("Music", "Lub-dub + Spectrum"),
    ("Music", "Coffee break + Spectrum"),
    ("Music", "Cat tail + Spectrum"),
    ("Music", "CPU temperature"),
    ("Music", "GPU temperature"),
    ("Music", "CPU load"),
    ("Music", "GPU load"),
    ("Music", "RAM load"),
    ("Music", "Dancer"),
    ("Music", "Lub-dub"),
    ("Music", "Coffee break"),
    ("Music", "Cat tail"),
];

/// A mode's tile: its group and option.
pub fn entry(mode: u32) -> (&'static str, &'static str) {
    let at = MODES
        .iter()
        .position(|&m| m == mode)
        .unwrap_or_else(|| panic!("mode {mode} is no mode"));
    TILES[at]
}

/// A tile's accessible name, `<group>: <option>`.
pub fn tile(mode: u32) -> String {
    let (group, option) = entry(mode);
    format!("{group}: {option}")
}

/// The preview's caption, `<group> · <option>`.
pub fn caption(mode: u32) -> String {
    let (group, option) = entry(mode);
    format!("{group} · {option}")
}

/// The falls' five stops, slowest first: their names on the bus and on the scale.
const STOPS: [(&str, &str); 5] = [
    ("slower", "Slower"),
    ("slow", "Slow"),
    ("moderate", "Moderate"),
    ("fast", "Fast"),
    ("faster", "Faster"),
];

/// The made-up GPUs' labels (`<kind> · <driver> · <address>`, no product name).
const DEDICATED_LABEL: &str = "Dedicated · acmegpu · 0000:0a:00.0";

const INTEGRATED_LABEL: &str = "Integrated · acmegfx · 0000:00:02.0";

const NO_RULE_LABEL: &str = "Unknown type · othergpu · 0000:07:00.0";

const QUERY_FAILED_LABEL: &str = "Unknown type · othergpu · 0000:0b:00.0";

/// The GPU's modes and the metric each loses, as the service names it (`docs/spec/sensors.md`).
const GPU_MODES: [(u32, &str); 9] = [
    (2, "Gpu usage"),
    (4, "Gpu usage"),
    (6, "Gpu usage"),
    (8, "Gpu temperature"),
    (9, "Gpu temperature"),
    (11, "Gpu power"),
    (12, "Gpu power"),
    (19, "Gpu temperature"),
    (21, "Gpu usage"),
];

/// The display's name for a metric the service names by the spec (decisions log D35): the load
/// metric is "GPU load" since Task 58 (owner, 2026-11-23).
fn shown_metric(metric: &str) -> String {
    metric
        .replacen("Gpu usage", "GPU load", 1)
        .replacen("Gpu", "GPU", 1)
}

/// The Start at boot switch's name, the arguments the app gives `pkexec` before `on` or `off`,
/// and the sound's job as the fake systemd logs it (Start at boot).
const BOOT: &str = "Start at boot";

const PKEXEC_ARGS: &str = "--disable-internal-agent /usr/lib/opendeepcool/opendeepcool-boot";

const START_SOUND: &str = "user StartUnit(\"opendeepcool-audio.service\", \"replace\")";

const STOP_SOUND: &str = "user StopUnit(\"opendeepcool-audio.service\", \"replace\")";

/// The off screen's message, as its group describes it (The off screen).
const IS_OFF: &str =
    "OpenDeepCool is off. The cooler shows nothing until the display service runs.";
const NOT_RUNNING: &str =
    "OpenDeepCool isn't running. The cooler shows nothing until the display service runs.";

/// A made-up light Omarchy theme.
const COLORS: &str = r##"mode = "light"
accent = "#8a4b16"
selection = "#e6d6bf"
muted = "#6b6255"
background = "#f4efe6"
lighter_background = "#fffaf2"
foreground = "#2b2620"
red = "#a3302a"
yellow = "#7a5a00"
green = "#2f6b2a"
"##;

pub struct Run {
    pub driver: Driver,
    pub fake: Fake,
    pub systemd: Systemd,
    app_log: PathBuf,
    frame_log: PathBuf,
    omarchy_state: PathBuf,
    wayland_display: PathBuf,
    app_pid: Option<u32>,
}

/// The made-up vendor IDs are in no `pci.ids` here, so every label names the GPU's driver.
fn vendors_unlisted() -> Result<()> {
    let path = "/usr/share/hwdata/pci.ids";
    let Ok(text) = fs::read_to_string(path) else {
        // Without the list, no GPU gets a product name anyway.
        return Ok(());
    };
    for vendor in fake::VENDORS {
        let start = format!("{vendor:04x} ");
        if text.lines().any(|line| line.starts_with(&start)) {
            return Err(format!(
                "{path} lists vendor {vendor:04x}, which the fake's GPUs use: pick another"
            ));
        }
    }
    Ok(())
}

impl Run {
    pub fn start(paths: &Paths) -> Result<Run> {
        vendors_unlisted()?;
        Ok(Run {
            driver: Driver::connect(&paths.a11y_bus)?,
            fake: Fake::new(&paths.fake),
            systemd: Systemd::new(
                paths
                    .systemd
                    .as_deref()
                    .ok_or("--systemd is needed against the fake")?,
            ),
            app_log: paths.app_log.clone(),
            frame_log: paths.frame_log.clone(),
            omarchy_state: paths.omarchy_state.clone(),
            wayland_display: paths.wayland_display.clone(),
            app_pid: paths.app_pid,
        })
    }

    fn node(&self, role: &str, name: &str) -> Result<Node> {
        self.driver.shown(role, name)
    }

    fn act(&self, role: &str, name: &str, action: &str) -> Result<()> {
        let node = self.node(role, name)?;
        self.driver.act(&node, action)
    }

    /// Waits until `name` shows with `state` on or off.
    fn expect_state(&self, role: &str, name: &str, state: State, on: bool) -> Result<()> {
        let what = if on { "with" } else { "without" };
        wait(&format!("{role} {name:?} {what} {state:?}"), || {
            let Some(node) = self.driver.find(role, name)? else {
                return Ok(None);
            };
            Ok((self.driver.has(&node, state)? == on).then_some(()))
        })
    }

    /// `name` shows with `state` on or off now.
    fn check_state(&self, role: &str, name: &str, state: State, on: bool) -> Result<()> {
        let node = self.node(role, name)?;
        if self.driver.has(&node, state)? == on {
            Ok(())
        } else {
            let what = if on { "without" } else { "with" };
            Err(format!("{role} {name:?} {what} {state:?}"))
        }
    }

    /// Waits until the labels under `name` read `want`.
    fn expect_texts(&self, role: &str, name: &str, want: &[&str]) -> Result<()> {
        let mut last = Vec::new();
        wait(&format!("{role} {name:?} reading {want:?}"), || {
            let Some(node) = self.driver.find(role, name)? else {
                return Ok(None);
            };
            last = self.driver.texts(&node)?;
            Ok((last == want).then_some(()))
        })
        .map_err(|e| format!("{e}; it reads {last:?}"))
    }

    /// The labels under `name` read `want` now.
    fn check_texts(&self, role: &str, name: &str, want: &[&str]) -> Result<()> {
        let texts = self.driver.texts(&self.node(role, name)?)?;
        if texts == want {
            Ok(())
        } else {
            Err(format!("{role} {name:?} reads {texts:?}, not {want:?}"))
        }
    }

    fn expect_description(&self, role: &str, name: &str, want: &str) -> Result<()> {
        let mut last = String::new();
        wait(&format!("{role} {name:?} described {want:?}"), || {
            let Some(node) = self.driver.find(role, name)? else {
                return Ok(None);
            };
            last = self.driver.description(&node)?;
            Ok((last == want).then_some(()))
        })
        .map_err(|e| format!("{e}; it's described {last:?}"))
    }

    /// Waits until a value widget holds `value`, with `text` as its value text.
    fn expect_value(&self, role: &str, name: &str, value: f64, text: &str) -> Result<()> {
        let mut last = (f64::NAN, String::new());
        wait(&format!("{role} {name:?} at {value} {text:?}"), || {
            let Some(node) = self.driver.find(role, name)? else {
                return Ok(None);
            };
            last = self.driver.value(&node)?;
            Ok((last.0 == value && last.1 == text).then_some(()))
        })
        .map_err(|e| format!("{e}; it's at {last:?}"))
    }

    fn gone(&self, role: &str, name: &str) -> Result<()> {
        self.driver.gone(role, name)
    }

    /// Shows the page `name`: the gear opens Configuration, where it stays pressed, and the
    /// Back icon returns to Display.
    fn page(&self, name: &str) -> Result<()> {
        let gear = self.node("toggle button", "Configuration")?;
        let on_config = self.driver.has(&gear, State::Pressed)?;
        match (name, on_config) {
            ("Configuration", false) => self.driver.act(&gear, "Click")?,
            ("Display", true) => {
                let back = self.node("button", "Back")?;
                self.driver.act(&back, "Click")?;
            }
            _ => {}
        }
        self.expect_state(
            "toggle button",
            "Configuration",
            State::Pressed,
            name == "Configuration",
        )?;
        // A control of the page, shown.
        let (role, control) = if name == "Display" {
            ("grid", "Load options")
        } else {
            ("toggle button", "Rotation 0°")
        };
        self.node(role, control).map(|_| ())
    }

    /// The first start's state on the bus, every answer given, no list open, the Display page;
    /// systemd fully on (the app reads it at its next reason to).
    fn reset(&mut self) -> Result<()> {
        if self.systemd.units != Units::all_on() {
            self.systemd.units = Units::all_on();
            self.systemd.write()?;
        }
        self.fake.on_bus = true;
        self.fake.reads = true;
        self.fake.refuse = None;
        self.fake.service = Service::first_start();
        self.fake.write()?;
        // A window left not available waits for its activation (Not available in this session).
        if self
            .driver
            .find("group", "Not available in this session")?
            .is_some()
        {
            self.activate_again()?;
        }
        self.node("toggle button", "Configuration")?;
        if let Some(node) = self.driver.find("combo box", "GPU")?
            && self.driver.has(&node, State::Expanded)?
        {
            self.close("GPU")?;
        }
        self.page("Display")?;
        self.expect_state("grid cell", "Load: CPU", State::Selected, true)
    }

    /// A change, as Refused changes asks: `act`, then exactly `call` at the fake, then `held`
    /// still true after a while, then the state `confirm` makes, then `after`.
    fn change(
        &mut self,
        act: impl Fn(&Run) -> Result<()>,
        call: &str,
        held: impl Fn(&Run) -> Result<()>,
        confirm: impl FnOnce(&mut Service),
        after: impl Fn(&Run) -> Result<()>,
    ) -> Result<()> {
        let mark = self.fake.mark()?;
        act(self)?;
        self.fake.called(mark, call)?;
        thread::sleep(HOLD);
        held(self).map_err(|e| format!("before the service confirmed {call}: {e}"))?;
        let changes = self.fake.changes_since(mark)?;
        if changes != [call] {
            return Err(format!("for {call}, the fake got {changes:?}"));
        }
        confirm(&mut self.fake.service);
        self.fake.write()?;
        after(self).map_err(|e| format!("after {call}: {e}"))
    }

    /// A refused change or Refresh: `act`, then `line` at the fake, then the feedback's words.
    fn refused(&self, act: impl Fn(&Run) -> Result<()>, line: &str, feedback: &str) -> Result<()> {
        let mark = self.fake.mark()?;
        act(self)?;
        self.fake.called(mark, line)?;
        self.expect_texts("group", "Change feedback", &[feedback])
    }

    /// Opens dropdown `combo`'s list through its button.
    fn open(&self, combo: &str) -> Result<()> {
        let node = self.node("combo box", combo)?;
        let button = self
            .driver
            .find_in(&node, "toggle button", combo)?
            .ok_or(format!("{combo:?} has no button"))?;
        self.driver.act(&button, "Click")?;
        self.expect_state("combo box", combo, State::Expanded, true)
    }

    fn close(&self, combo: &str) -> Result<()> {
        let node = self.node("combo box", combo)?;
        if let Some(button) = self.driver.find_in(&node, "toggle button", combo)? {
            self.driver.act(&button, "Click")?;
        }
        self.expect_state("combo box", combo, State::Expanded, false)
    }

    /// The open list's items: each label, and its note in the description.
    fn items(&self, combo: &str) -> Result<Vec<(String, String)>> {
        let node = self.node("combo box", combo)?;
        let mut items = Vec::new();
        for item in self.driver.subtree(&node)? {
            if item.role == "list item" {
                items.push((item.name.clone(), self.driver.description(&item)?));
            }
        }
        Ok(items)
    }

    fn expect_items(&self, combo: &str, want: &[(&str, &str)]) -> Result<()> {
        let mut last = Vec::new();
        wait(&format!("{combo:?}'s list"), || {
            last = self.items(combo)?;
            let same = last.len() == want.len()
                && last
                    .iter()
                    .zip(want)
                    .all(|((l, n), (wl, wn))| l == wl && n == wn);
            Ok(same.then_some(()))
        })
        .map_err(|e| format!("{e}: {last:?}, not {want:?}"))
    }

    /// Picks `item` in `combo`'s open list through the row's action, which the app gives every
    /// open row (decisions log D69); the list closes.
    fn pick(&self, combo: &str, item: &str) -> Result<()> {
        let node = self.node("combo box", combo)?;
        let entry = self
            .driver
            .find_in(&node, "list item", item)?
            .ok_or(format!("{combo:?} lists no {item:?}"))?;
        let row = self
            .driver
            .children(&entry)?
            .into_iter()
            .next()
            .ok_or(format!("{item:?} has no row"))?;
        self.driver.act(&row, "row.activate")?;
        self.expect_state("combo box", combo, State::Expanded, false)
    }

    /// Waits until the app's log has `line`.
    fn logged(&self, line: &str) -> Result<()> {
        wait(&format!("the app's log saying {line:?}"), || {
            let log = fs::read_to_string(&self.app_log)
                .map_err(|e| format!("{}: {e}", self.app_log.display()))?;
            Ok(log.lines().any(|l| l == line).then_some(()))
        })
    }

    /// The window turns inactive and active again: another client's window takes the focus,
    /// then closes.
    /// Writes systemd's state, then has the app read it: the window active again (Where its
    /// state comes from: no polling).
    fn units(&mut self, units: Units) -> Result<()> {
        self.systemd.units = units;
        self.systemd.write()?;
        self.activate_again()
    }

    /// Waits until the switch's note reads `text`.
    fn note(&self, text: &str) -> Result<()> {
        self.expect_texts("group", "Start at boot status", &[text])?;
        self.expect_description("switch", BOOT, text)
    }

    /// A running flip shows no note and the switch has no description (owner, 2026-11-22: no
    /// "Turning on…" or "Turning off…" line).
    fn no_note(&self) -> Result<()> {
        self.gone("group", "Start at boot status")?;
        self.expect_description("switch", BOOT, "")
    }

    /// The app has no terminal commands and no Copy button: they are in the README (owner,
    /// 2026-11-19).
    fn no_terminal(&self) -> Result<()> {
        self.gone("group", "Terminal commands")?;
        self.gone("button", "Copy terminal commands")
    }

    /// Flips the switch; returns once the fake `pkexec` runs with `to`, the only run since.
    fn flip(&self, to: &str) -> Result<(usize, usize)> {
        let mark = self.systemd.mark()?;
        self.act("switch", BOOT, "Toggle")?;
        self.systemd
            .pkexec_ran(mark, &format!("{PKEXEC_ARGS} {to}"))?;
        Ok(mark)
    }

    fn activate_again(&self) -> Result<()> {
        let runtime = self
            .wayland_display
            .parent()
            .ok_or("the Wayland socket has no folder")?;
        let mut other = Command::new("weston-simple-shm")
            .env_clear()
            .env("WAYLAND_DISPLAY", &self.wayland_display)
            .env("XDG_RUNTIME_DIR", runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("weston-simple-shm (weston's own client): {e}"))?;
        let away = self.expect_state("window", "OpenDeepCool", State::Active, false);
        let _ = other.kill();
        let _ = other.wait();
        away?;
        self.expect_state("window", "OpenDeepCool", State::Active, true)
    }
}

pub type Scenario = fn(&mut Run) -> Result<String>;

pub const ALL: [(&str, Scenario); 30] = [
    ("service_down", basics::service_down),
    ("first_start", basics::first_start),
    ("names", basics::names),
    ("frame_log", basics::frame_log),
    ("tiles", basics::tiles),
    ("unit", settings::unit),
    ("rotation", settings::rotation),
    ("gpu", settings::gpu),
    ("gpu_notes", settings::gpu_notes),
    ("one_gpu", settings::one_gpu),
    ("music", settings::music),
    ("refresh", settings::refresh),
    ("shortcuts", settings::shortcuts),
    ("refused", settings::refused),
    ("cooler_states", states::cooler_states),
    ("unavailable_metric", states::unavailable_metric),
    ("reading_lost", states::reading_lost),
    ("music_sound", states::music_sound),
    ("service_stops", states::service_stops),
    ("not_available", states::not_available),
    ("boot_on", boot::boot_on),
    ("boot_flip", boot::boot_flip),
    ("boot_refused", boot::boot_refused),
    ("boot_only_sound", boot::boot_only_sound),
    ("boot_partly", boot::boot_partly),
    ("boot_off_screen", boot::boot_off_screen),
    ("boot_remote", boot::boot_remote),
    ("theme", theme::theme),
    ("tiles_live", theme::tiles_live),
    ("tile_cost", theme::tile_cost),
];

const NO_NOTE: Result<String> = Ok(String::new());

/// Whether a fake systemd log line is a job or a subscription, not a read.
fn is_job(call: &str) -> bool {
    ["StartUnit(", "StopUnit(", "Subscribe()", "Unsubscribe()"]
        .iter()
        .any(|word| call.contains(word))
}

/// Each tile's flag and its sensitivity, from assistive technology: `flagged` modes are
/// "Unavailable", `lost` is the saved mode's, usable and "Reading lost", the rest usable with
/// their caption alone.
fn flags(run: &Run, flagged: &[u32], lost: Option<u32>) -> Result<()> {
    for mode in MODES {
        let name = tile(mode);
        let (_, option) = entry(mode);
        if lost == Some(mode) {
            run.expect_state("grid cell", &name, State::Sensitive, true)?;
            run.expect_texts("grid cell", &name, &[option, "Reading lost"])?;
        } else if flagged.contains(&mode) {
            run.expect_state("grid cell", &name, State::Sensitive, false)?;
            run.check_texts("grid cell", &name, &[option, "Unavailable"])?;
        } else {
            run.expect_state("grid cell", &name, State::Sensitive, true)?;
            run.check_texts("grid cell", &name, &[option])?;
        }
    }
    Ok(())
}

/// The app's CPU time in seconds, from `/proc/<pid>/stat`: `utime` and `stime`, in the clock
/// ticks `/proc` gives, which are a hundredth of a second on every platform Linux builds for
/// (`USER_HZ`).
fn cpu_seconds(pid: u32) -> Result<f64> {
    let path = format!("/proc/{pid}/stat");
    let stat = fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    // The command's name, in brackets, may hold spaces: the fields count from the last one.
    let rest = stat
        .rsplit_once(')')
        .ok_or_else(|| format!("{path}: no command name"))?
        .1;
    let field = |n: usize| -> Result<f64> {
        rest.split_whitespace()
            .nth(n)
            .and_then(|f| f.parse::<f64>().ok())
            .ok_or_else(|| format!("{path}: no field {}", n + 3))
    };
    // After the command: the state is field 3, utime 14 and stime 15.
    Ok((field(11)? + field(12)?) / 100.0)
}

/// How long each state of the tiles' cost check runs.
const COST_FOR: Duration = Duration::from_secs(10);

/// The most CPU the app may use in each state, as a share of one core: about sixty times the
/// highest of three measurements here (owner, 2026-11-17: slower PCs must pass), floored at 1 %
/// and capped at one core (design: Live tiles, The cost). Measured on 2026-11-19 in this run's
/// build, in a headless weston with the Cairo renderer: steady 6.3 %, busy 12 % and still 0.0 %
/// of a core, so the first two reach the cap, where the check only catches a runaway and the
/// counts of steps and draws in `tests/live_tiles.rs` carry the proof, and the last the floor.
const COST_CEILING: [(Cost, f64); 3] =
    [(Cost::Steady, 1.0), (Cost::Busy, 1.0), (Cost::Still, 0.01)];

/// The three states of the tiles' cost check.
#[derive(Clone, Copy)]
enum Cost {
    Steady,
    Busy,
    Still,
}

impl Cost {
    fn name(self) -> &'static str {
        match self {
            Cost::Steady => "steady",
            Cost::Busy => "busy",
            Cost::Still => "still",
        }
    }
}

/// The tiles' cost (Phase 11a Task 14): the app's CPU over ten seconds in three states, each
/// under its ceiling. `pace` is called every 100 ms with the tick's number.
fn cost(run: &mut Run, pid: u32, mut pace: impl FnMut(&mut Run, u32) -> Result<()>) -> Result<f64> {
    // Let the page settle first.
    thread::sleep(Duration::from_millis(1000));
    let (cpu, wall) = (cpu_seconds(pid)?, Instant::now());
    let mut tick = 0;
    while wall.elapsed() < COST_FOR {
        pace(run, tick)?;
        tick += 1;
        let due = wall + Duration::from_millis(100) * tick;
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            thread::sleep(wait);
        }
    }
    Ok((cpu_seconds(pid)? - cpu) / wall.elapsed().as_secs_f64())
}
