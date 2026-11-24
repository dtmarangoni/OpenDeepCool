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
    ("service_down", service_down),
    ("first_start", first_start),
    ("names", names),
    ("frame_log", frame_log),
    ("tiles", tiles),
    ("unit", unit),
    ("rotation", rotation),
    ("gpu", gpu),
    ("gpu_notes", gpu_notes),
    ("one_gpu", one_gpu),
    ("music", music),
    ("refresh", refresh),
    ("shortcuts", shortcuts),
    ("refused", refused),
    ("cooler_states", cooler_states),
    ("unavailable_metric", unavailable_metric),
    ("reading_lost", reading_lost),
    ("music_sound", music_sound),
    ("service_stops", service_stops),
    ("not_available", not_available),
    ("boot_on", boot_on),
    ("boot_flip", boot_flip),
    ("boot_refused", boot_refused),
    ("boot_only_sound", boot_only_sound),
    ("boot_partly", boot_partly),
    ("boot_off_screen", boot_off_screen),
    ("boot_remote", boot_remote),
    ("theme", theme),
    ("tiles_live", tiles_live),
    ("tile_cost", tile_cost),
];

const NO_NOTE: Result<String> = Ok(String::new());

/// Service not running, at the start: the fake hasn't taken the bus, and systemd answers as a
/// new install. The off screen, "OpenDeepCool is off", with the switch off and usable, and
/// no terminal commands (they are in the README; owner, 2026-11-19; The off screen; D10).
fn service_down(run: &mut Run) -> Result<String> {
    run.expect_description("group", "Service message", IS_OFF)?;
    run.check_state("switch", BOOT, State::Sensitive, true)?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    run.expect_state("switch", BOOT, State::Focused, true)?;
    run.gone("group", "Start at boot status")?;
    run.no_terminal()?;
    run.node("group", "Startup settings")?;
    // The gear stays, dimmed and not pressed; Refresh belongs to the Display page, which the off
    // screen replaces; the close icon always works.
    run.check_state("toggle button", "Configuration", State::Sensitive, false)?;
    run.check_state("toggle button", "Configuration", State::Pressed, false)?;
    run.check_state("button", "Close", State::Sensitive, true)?;
    run.gone("button", "Refresh display")?;
    run.expect_texts("group", "Cooler status", &["Cooler: unknown"])?;
    run.expect_texts("group", "Service status", &["Service off"])?;
    run.expect_description("image", "Cooler display preview", "No picture")?;
    run.gone("grid cell", "Load: CPU")?;
    let calls = run.fake.calls_since(0)?;
    if !calls.is_empty() {
        return Err(format!("calls with no service: {calls:?}"));
    }
    // systemd read without a password: each unit's file state and run state, and the session.
    let read = run.systemd.calls_since((0, 0))?;
    for call in [
        "system GetUnitFileState(\"opendeepcool.service\")",
        "system LoadUnit(\"opendeepcool.service\")",
        "user GetUnitFileState(\"opendeepcool-audio.service\")",
        "user LoadUnit(\"opendeepcool-audio.service\")",
        "GetSession(\"auto\")",
    ] {
        if !read.iter().any(|c| c == call) {
            return Err(format!("{call} not read: {read:?}"));
        }
    }
    if read.iter().any(|c| c.contains("Subscribe")) {
        return Err(format!("subscribed with no job: {read:?}"));
    }
    // No Omarchy folder: the default theme (Theme).
    run.logged("opendeepcool: theme: no Omarchy theme")?;
    NO_NOTE
}

/// The service appears with a first start's state: the pages come back with no action, and
/// show the first start's defaults (States, First start).
fn first_start(run: &mut Run) -> Result<String> {
    let mark = run.fake.mark()?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    // Watched, then read, then the frames (Talking to the service).
    let calls = run.fake.calls_since(mark)?;
    if calls.get(..3) != Some(&["Watch()", "GetState()", "WatchFrames()"].map(String::from)[..]) {
        return Err(format!("the app's first calls: {calls:?}"));
    }
    run.gone("group", "Service message")?;
    run.expect_texts("group", "Cooler status", &["Cooler connected"])?;
    run.expect_texts("group", "Service status", &["Service running"])?;
    run.gone("group", "Music sound status")?;
    run.expect_description("image", "Cooler display preview", "Load · CPU")?;
    run.node("label", "Load · CPU")?;
    run.check_state("toggle button", "Configuration", State::Sensitive, true)?;
    run.check_state("toggle button", "Configuration", State::Pressed, false)?;
    run.check_state("button", "Close", State::Sensitive, true)?;
    run.check_state("button", "Refresh display", State::Sensitive, true)?;
    run.page("Configuration")?;
    for degrees in [0, 90, 180, 270] {
        let name = format!("Rotation {degrees}°");
        run.check_state("toggle button", &name, State::Pressed, degrees == 0)?;
    }
    run.check_state("toggle button", "Celsius", State::Pressed, true)?;
    run.check_state("toggle button", "Fahrenheit", State::Pressed, false)?;
    // The Theme card: no Omarchy theme and no portal here, so no system theme can be found: the
    // switch off and dimmed, Dark pressed and both buttons usable, and the warning.
    run.check_state("switch", "Use system theme", State::Checked, false)?;
    run.check_state("switch", "Use system theme", State::Sensitive, false)?;
    run.check_state("toggle button", "Dark theme", State::Pressed, true)?;
    run.check_state("toggle button", "Light theme", State::Pressed, false)?;
    run.check_state("toggle button", "Dark theme", State::Sensitive, true)?;
    run.expect_texts(
        "group",
        "Theme status",
        &["OpenDeepCool can't find the system's theme and is using its own."],
    )?;
    run.expect_texts("combo box", "GPU", &["Automatic"])?;
    run.expect_description("combo box", "GPU", &format!("Using {DEDICATED_LABEL}"))?;
    run.expect_texts(
        "group",
        "GPU in use",
        &[&format!("Using {DEDICATED_LABEL}.")],
    )?;
    run.check_state("switch", "Peak dots", State::Checked, true)?;
    run.expect_value("slider", "Bar fall", 2.0, "Moderate")?;
    run.expect_value("slider", "Peak fall", 1.0, "Slow")?;
    run.page("Display")?;
    NO_NOTE
}

/// The design's accessible names that always show, each once, with its role as GTK gives it
/// (Accessible names; the others are checked in their own states).
fn names(run: &mut Run) -> Result<String> {
    run.reset()?;
    let owned = |list: &[(&'static str, &str)]| -> Vec<(&'static str, String)> {
        list.iter()
            .map(|(role, name)| (*role, (*name).to_owned()))
            .collect()
    };
    let always = owned(&[
        ("toggle button", "Configuration"),
        ("button", "Close"),
        ("image", "Cooler display preview"),
        ("group", "Cooler status"),
        ("group", "Service status"),
    ]);
    let mut display = owned(&[
        ("button", "Refresh display"),
        ("grid", "Load options"),
        ("grid", "Temperature options"),
        ("grid", "Power options"),
        ("grid", "Music options"),
    ]);
    for mode in MODES {
        // Each tile, and its picture named as its tile (decisions log D57).
        display.push(("grid cell", tile(mode)));
        display.push(("image", tile(mode)));
    }
    let configuration = owned(&[
        ("button", "Back"),
        ("group", "Theme settings"),
        ("switch", "Use system theme"),
        ("toggle button", "Dark theme"),
        ("toggle button", "Light theme"),
        // The Default Theme subcategory (Phase 11a Task 50).
        ("group", "Default Theme"),
        ("group", "Appearance"),
        ("group", "Window Opacity"),
        ("group", "Window Corners"),
        ("toggle button", "Opaque window"),
        ("toggle button", "Transparent window"),
        ("toggle button", "Glassy window"),
        ("toggle button", "Sharp corners"),
        ("toggle button", "Rounded corners"),
        ("group", "Theme status"),
        ("group", "Display settings"),
        ("group", "Music settings"),
        ("toggle button", "Rotation 0°"),
        ("toggle button", "Rotation 90°"),
        ("toggle button", "Rotation 180°"),
        ("toggle button", "Rotation 270°"),
        ("toggle button", "Celsius"),
        ("toggle button", "Fahrenheit"),
        ("combo box", "GPU"),
        ("group", "GPU in use"),
        ("switch", "Peak dots"),
        ("slider", "Bar fall"),
        ("slider", "Peak fall"),
        ("group", "Startup settings"),
        ("switch", BOOT),
    ]);
    let check = |run: &Run, names: &[(&str, String)]| -> Result<()> {
        let tree = run.driver.tree()?;
        for (role, name) in names {
            let count = tree
                .iter()
                .filter(|(_, n)| n.role == *role && n.name == *name)
                .count();
            if count != 1 {
                return Err(format!("{role} {name:?} shown {count} times"));
            }
        }
        Ok(())
    };
    check(run, &always)?;
    check(run, &display)?;
    run.page("Configuration")?;
    check(run, &always)?;
    check(run, &configuration)?;
    run.page("Display")?;
    NO_NOTE
}

/// The frames the service sends reach the app, which logs each with its time (`--frame-log`,
/// for the real run's comparison with the dev recording; decisions log D84).
fn frame_log(run: &mut Run) -> Result<String> {
    run.reset()?;
    for lit in [0_usize, 195, 97] {
        let mut dots = vec!['0'; 196];
        dots[lit] = '1';
        let dots: String = dots.into_iter().collect();
        run.fake.frame = Some(dots.clone());
        run.fake.write()?;
        wait(
            &format!("the app logging the frame with dot {lit} lit"),
            || {
                let log = fs::read_to_string(&run.frame_log)
                    .map_err(|e| format!("{}: {e}", run.frame_log.display()))?;
                let last = log.lines().last().unwrap_or("");
                let (time, logged) = last.split_once(' ').unwrap_or(("", ""));
                Ok((logged == dots && time.parse::<u64>().is_ok()).then_some(()))
            },
        )?;
    }
    NO_NOTE
}

/// Each of the 25 tiles asks for its own mode, once, and shows as chosen only once the service
/// confirms it; the preview's caption follows (Display page, Selection; Coverage check, One
/// choice, one mode). Music modes add the sound's status (Preview column, Status line).
fn tiles(run: &mut Run) -> Result<String> {
    run.reset()?;
    let mut saved = 1;
    for mode in MODES.into_iter().skip(1).chain([1]) {
        let (name, before) = (tile(mode), tile(saved));
        run.change(
            |run| run.act("grid cell", &name, "tile.activate"),
            &format!("SetMode({mode})"),
            |run| {
                run.check_state("grid cell", &before, State::Selected, true)?;
                if mode == saved {
                    Ok(())
                } else {
                    run.check_state("grid cell", &name, State::Selected, false)
                }
            },
            |service| service.mode = mode,
            |run| {
                run.expect_state("grid cell", &name, State::Selected, true)?;
                if mode != saved {
                    run.expect_state("grid cell", &before, State::Selected, false)?;
                }
                run.expect_description("image", "Cooler display preview", &caption(mode))?;
                if (13..=26).contains(&mode) {
                    run.expect_texts("group", "Music sound status", &["Sound connected"])
                } else {
                    run.gone("group", "Music sound status")
                }
            },
        )?;
        saved = mode;
    }
    NO_NOTE
}

/// The temperature unit, both ways (Display page, Temperature unit).
fn unit(run: &mut Run) -> Result<String> {
    run.reset()?;
    // The unit is on the Configuration page, after Rotation.
    run.page("Configuration")?;
    for (name, other, unit) in [
        ("Fahrenheit", "Celsius", "fahrenheit"),
        ("Celsius", "Fahrenheit", "celsius"),
    ] {
        run.change(
            |run| run.act("toggle button", name, "Click"),
            &format!("SetTemperatureUnit({unit:?})"),
            |run| {
                run.check_state("toggle button", other, State::Pressed, true)?;
                run.check_state("toggle button", name, State::Pressed, false)
            },
            |service| service.unit = unit,
            |run| {
                run.expect_state("toggle button", name, State::Pressed, true)?;
                run.expect_state("toggle button", other, State::Pressed, false)
            },
        )?;
    }
    run.page("Display")?;
    NO_NOTE
}

/// Each rotation (Configuration page, Display, Rotation).
fn rotation(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    let mut saved = 0_u8;
    for turn in [1_u8, 2, 3, 0] {
        let name = format!("Rotation {}°", u32::from(turn) * 90);
        let before = format!("Rotation {}°", u32::from(saved) * 90);
        run.change(
            |run| run.act("toggle button", &name, "Click"),
            &format!("SetRotation({turn})"),
            |run| {
                run.check_state("toggle button", &before, State::Pressed, true)?;
                run.check_state("toggle button", &name, State::Pressed, false)
            },
            |service| service.rotation = turn,
            |run| {
                run.expect_state("toggle button", &name, State::Pressed, true)?;
                run.expect_state("toggle button", &before, State::Pressed, false)
            },
        )?;
        saved = turn;
    }
    run.page("Display")?;
    NO_NOTE
}

/// The GPU: Automatic first and the default, naming the GPU it reads; a GPU chosen by hand,
/// then Automatic again (Configuration page, GPU).
fn gpu(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    run.open("GPU")?;
    run.expect_items(
        "GPU",
        &[
            ("Automatic", ""),
            (DEDICATED_LABEL, ""),
            (INTEGRATED_LABEL, ""),
        ],
    )?;
    run.change(
        |run| run.pick("GPU", INTEGRATED_LABEL),
        &format!("ChooseGpu({:?})", INTEGRATED.pci),
        |run| run.check_texts("combo box", "GPU", &["Automatic"]),
        |service| {
            service.chosen_gpu = INTEGRATED.pci;
            service.gpu_in_use = INTEGRATED.pci;
        },
        |run| {
            run.expect_texts("combo box", "GPU", &[INTEGRATED_LABEL])?;
            run.expect_description("combo box", "GPU", "")?;
            // Chosen by hand, the GPU is named on the button alone.
            run.gone("group", "GPU in use")
        },
    )?;
    run.open("GPU")?;
    run.change(
        |run| run.pick("GPU", "Automatic"),
        "ChooseGpu(\"\")",
        |run| run.check_texts("combo box", "GPU", &[INTEGRATED_LABEL]),
        |service| {
            service.chosen_gpu = "";
            service.gpu_in_use = DEDICATED.pci;
        },
        |run| {
            run.expect_texts("combo box", "GPU", &["Automatic"])?;
            run.expect_texts(
                "group",
                "GPU in use",
                &[&format!("Using {DEDICATED_LABEL}.")],
            )?;
            run.expect_description("combo box", "GPU", &format!("Using {DEDICATED_LABEL}"))
        },
    )?;
    run.page("Display")?;
    NO_NOTE
}

/// A GPU whose kind query failed, with the unknown ones and its note; a GPU without a reading
/// rule last, with its note; chosen, its hint, and the GPU tiles disabled with the reason
/// (Configuration page, GPU; States, A GPU without a reading rule in use).
fn gpu_notes(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.service.gpus = vec![DEDICATED, INTEGRATED, NO_RULE, QUERY_FAILED];
    run.fake.write()?;
    run.page("Configuration")?;
    run.open("GPU")?;
    run.expect_items(
        "GPU",
        &[
            ("Automatic", ""),
            (DEDICATED_LABEL, ""),
            (INTEGRATED_LABEL, ""),
            (QUERY_FAILED_LABEL, "driver couldn't be asked"),
            (NO_RULE_LABEL, "no readings"),
        ],
    )?;
    let reasons: Vec<(u32, String)> = GPU_MODES
        .iter()
        .map(|&(mode, metric)| (mode, format!("{metric}: no reading for this driver")))
        .collect();
    run.change(
        |run| run.pick("GPU", NO_RULE_LABEL),
        &format!("ChooseGpu({:?})", NO_RULE.pci),
        |run| run.check_texts("combo box", "GPU", &["Automatic"]),
        |service| {
            service.chosen_gpu = NO_RULE.pci;
            service.gpu_in_use = NO_RULE.pci;
            service.reasons = reasons.iter().cloned().collect();
            service.available = MODES
                .into_iter()
                .filter(|m| !reasons.iter().any(|(r, _)| r == m))
                .collect();
        },
        |run| {
            run.expect_texts("combo box", "GPU", &[NO_RULE_LABEL])?;
            run.expect_description("combo box", "GPU", "no readings")?;
            run.node(
                "label",
                "No reading for this GPU's driver: the GPU options are unavailable.",
            )
            .map(|_| ())
        },
    )?;
    run.page("Display")?;
    for (mode, metric) in GPU_MODES {
        let name = tile(mode);
        run.expect_state("grid cell", &name, State::Sensitive, false)?;
        run.expect_description(
            "grid cell",
            &name,
            &format!("{}: no reading for this driver", shown_metric(metric)),
        )?;
    }
    NO_NOTE
}

/// One GPU: no GPU row at all (States, One GPU).
fn one_gpu(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.service.gpus = vec![DEDICATED];
    run.fake.write()?;
    run.page("Configuration")?;
    run.gone("combo box", "GPU")?;
    run.gone("group", "GPU in use")?;
    run.page("Display")?;
    NO_NOTE
}

/// The peak dots off and on, and each fall at each of its five stops, by name (Configuration
/// page, Music). AT-SPI reads the switch's handle, which moves at the press (GTK's delayed
/// state, decisions log D63), and a scale follows the drag (D61), so neither is held: the call
/// and the confirmed state are checked.
fn music(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    for on in [false, true] {
        run.change(
            |run| run.act("switch", "Peak dots", "Toggle"),
            &format!("SetMusicPeaks({on})"),
            |_| Ok(()),
            |service| service.peaks = on,
            |run| {
                run.expect_state("switch", "Peak dots", State::Checked, on)?;
                thread::sleep(HOLD);
                run.check_state("switch", "Peak dots", State::Checked, on)
            },
        )?;
    }
    for (scale, call, saved) in [
        ("Bar fall", "SetMusicBarFall", 2_usize),
        ("Peak fall", "SetMusicPeakFall", 1),
    ] {
        let order: Vec<usize> = (0..STOPS.len())
            .filter(|&s| s != saved)
            .chain([saved])
            .collect();
        for stop in order {
            let (bus, word) = STOPS[stop];
            run.change(
                |run| {
                    let node = run.node("slider", scale)?;
                    run.driver.set_value(&node, stop as f64)
                },
                &format!("{call}({bus:?})"),
                |_| Ok(()),
                |service| {
                    if scale == "Bar fall" {
                        service.bar_fall = bus;
                    } else {
                        service.peak_fall = bus;
                    }
                },
                |run| run.expect_value("slider", scale, stop as f64, word),
            )?;
        }
    }
    run.page("Display")?;
    NO_NOTE
}

/// Refresh: its call, and "Refreshed" for a moment (Refresh).
fn refresh(run: &mut Run) -> Result<String> {
    run.reset()?;
    let mark = run.fake.mark()?;
    run.act("button", "Refresh display", "Click")?;
    run.fake.called(mark, "Refresh()")?;
    run.expect_texts("group", "Change feedback", &["Refreshed"])?;
    // About 1.5 s, then gone.
    run.gone("group", "Change feedback")?;
    let changes = run.fake.changes_since(mark)?;
    if changes != ["Refresh()"] {
        return Err(format!("the fake got {changes:?}"));
    }
    NO_NOTE
}

/// The shortcuts' actions, which the window offers AT-SPI: Configuration, Back and Refresh
/// (Keyboard, Shortcuts). A key typed through AT-SPI's device event controller is tried too.
fn shortcuts(run: &mut Run) -> Result<String> {
    run.reset()?;
    let window = run.node("window", "OpenDeepCool")?;
    run.driver.act(&window, "win.configuration")?;
    run.expect_state("toggle button", "Configuration", State::Pressed, true)?;
    run.node("toggle button", "Rotation 0°")?;
    run.driver.act(&window, "win.back")?;
    run.expect_state("toggle button", "Configuration", State::Pressed, false)?;
    run.node("grid", "Load options")?;
    let mark = run.fake.mark()?;
    run.driver.act(&window, "win.refresh")?;
    run.fake.called(mark, "Refresh()")?;
    run.expect_texts("group", "Change feedback", &["Refreshed"])?;
    run.gone("group", "Change feedback")?;
    // F5, typed through the registry (XK_F5 = 0xffc2).
    let mark = run.fake.mark()?;
    let typed = run.driver.key_sym(0xffc2);
    thread::sleep(Duration::from_secs(1));
    let reached = run
        .fake
        .changes_since(mark)?
        .iter()
        .any(|c| c == "Refresh()");
    Ok(match (typed, reached) {
        (_, true) => "F5 typed through AT-SPI reached the app".to_owned(),
        _ => "keys typed through AT-SPI don't reach a Wayland app: their bindings are checked \
              in-process (decisions log D71)"
            .to_owned(),
    })
}

/// Refused changes: the control stays on the saved value, and the feedback says "Not changed."
/// or "Not refreshed." with the service's reason, or the app's own for NotAuthorized (Refused
/// changes).
fn refused(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.refuse = Some("ModeUnavailable: Gpu usage: no device found".to_owned());
    run.fake.write()?;
    run.refused(
        |run| run.act("grid cell", "Load: GPU", "tile.activate"),
        "SetMode(2) refused ModeUnavailable",
        "Not changed. GPU load: no device found",
    )?;
    thread::sleep(HOLD);
    run.check_state("grid cell", "Load: CPU", State::Selected, true)?;
    run.check_state("grid cell", "Load: GPU", State::Selected, false)?;
    run.fake.refuse = Some("NotAuthorized: this session isn't the active one".to_owned());
    run.fake.write()?;
    const NOT_YOURS: &str = "Not changed. This session may not change settings.";
    run.refused(
        |run| run.act("button", "Refresh display", "Click"),
        "Refresh() refused NotAuthorized",
        "Not refreshed. This session may not change settings.",
    )?;
    run.page("Configuration")?;
    run.refused(
        |run| run.act("toggle button", "Fahrenheit", "Click"),
        "SetTemperatureUnit(\"fahrenheit\") refused NotAuthorized",
        NOT_YOURS,
    )?;
    run.expect_state("toggle button", "Celsius", State::Pressed, true)?;
    run.refused(
        |run| run.act("toggle button", "Rotation 90°", "Click"),
        "SetRotation(1) refused NotAuthorized",
        NOT_YOURS,
    )?;
    run.expect_state("toggle button", "Rotation 0°", State::Pressed, true)?;
    run.check_state("toggle button", "Rotation 90°", State::Pressed, false)?;
    run.refused(
        |run| {
            let node = run.node("slider", "Bar fall")?;
            run.driver.set_value(&node, 4.0)
        },
        "SetMusicBarFall(\"faster\") refused NotAuthorized",
        NOT_YOURS,
    )?;
    run.expect_value("slider", "Bar fall", 2.0, "Moderate")?;
    run.refused(
        |run| run.act("switch", "Peak dots", "Toggle"),
        "SetMusicPeaks(false) refused NotAuthorized",
        NOT_YOURS,
    )?;
    run.expect_state("switch", "Peak dots", State::Checked, true)?;
    run.open("GPU")?;
    run.refused(
        |run| run.pick("GPU", INTEGRATED_LABEL),
        &format!("ChooseGpu({:?}) refused NotAuthorized", INTEGRATED.pci),
        NOT_YOURS,
    )?;
    run.expect_texts("combo box", "GPU", &["Automatic"])?;
    run.fake.refuse = None;
    run.fake.write()?;
    run.page("Display")?;
    NO_NOTE
}

/// The cooler's states: one banner at a time, the status and the preview's description (no line
/// of the column says it), everything still changeable; `write-failed` as connected for its
/// first 2 s (Banners; States).
fn cooler_states(run: &mut Run) -> Result<String> {
    run.reset()?;
    const NOT_CONNECTED: (Option<&str>, &str, &str) = (
        Some(
            "The cooler isn't connected. Changes are still saved, and the cooler shows them \
             when it's back.",
        ),
        "Cooler not connected",
        "Load · CPU. Not on the cooler: it isn't connected.",
    );
    for (state, (banner, status, described)) in [
        ("not-found", NOT_CONNECTED),
        (
            "no-access",
            (
                Some(
                    "The cooler is plugged in, but the service can't open it. The udev rule \
                     that gives the service access is missing or not applied yet. Restart the \
                     PC; if that doesn't help, reinstall OpenDeepCool. The journal says more.",
                ),
                "Cooler: no access",
                "Load · CPU. Not on the cooler: the service can't open it.",
            ),
        ),
        // The dev copy's own state, and one a later service might send: as not-found.
        ("released", NOT_CONNECTED),
        ("on fire", NOT_CONNECTED),
        ("connected", (None, "Cooler connected", "Load · CPU")),
    ] {
        run.fake.service.cooler = state;
        run.fake.write()?;
        match banner {
            Some(text) => run.expect_description("group", "Cooler banner", text)?,
            None => run.gone("group", "Cooler banner")?,
        }
        run.expect_texts("group", "Cooler status", &[status])
            .map_err(|e| format!("{state}: {e}"))?;
        run.expect_description("image", "Cooler display preview", described)
            .map_err(|e| format!("{state}: {e}"))?;
        // Nothing is disabled.
        let tile = run.node("grid cell", "Load: GPU")?;
        if !run.driver.has(&tile, State::Sensitive)? {
            return Err(format!("{state}: the tiles are disabled"));
        }
    }
    run.fake.service.cooler = "write-failed";
    run.fake.write()?;
    let taken = Instant::now();
    thread::sleep(Duration::from_millis(500));
    run.gone("group", "Cooler banner")?;
    run.check_texts("group", "Cooler status", &["Cooler connected"])?;
    run.expect_description("image", "Cooler display preview", "Load · CPU")?;
    run.expect_description(
        "group",
        "Cooler banner",
        "The cooler is connected, but sending it the picture fails. The service keeps trying \
         by itself, and changes are still saved. The journal says more.",
    )?;
    let after = taken.elapsed();
    if after < Duration::from_millis(1900) {
        return Err(format!("write-failed's banner after {after:?}, not 2 s"));
    }
    run.expect_texts("group", "Cooler status", &["Cooler: sending fails"])?;
    run.expect_description(
        "image",
        "Cooler display preview",
        "Load · CPU. Not on the cooler: sending it fails.",
    )?;
    run.fake.service.cooler = "connected";
    run.fake.write()?;
    run.gone("group", "Cooler banner")?;
    Ok(format!(
        "write-failed's banner after {:.1} s",
        after.as_secs_f64()
    ))
}

/// The GPU's metrics unavailable: their tiles insensitive, "Unavailable", with the reason, and
/// each group's reasons once (States, Metric unavailable).
fn unavailable_metric(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.service.reasons = GPU_MODES
        .iter()
        .map(|&(mode, metric)| (mode, format!("{metric}: no device found")))
        .collect();
    run.fake.service.available = MODES
        .into_iter()
        .filter(|m| !GPU_MODES.iter().any(|(g, _)| g == m))
        .collect();
    // The readings the reasons are about have no value.
    run.fake
        .service
        .readings
        .retain(|metric, _| !metric.starts_with("Gpu"));
    run.fake.write()?;
    for mode in MODES {
        let name = tile(mode);
        let (_, option) = entry(mode);
        match GPU_MODES.iter().find(|(m, _)| *m == mode) {
            Some((_, metric)) => {
                run.expect_state("grid cell", &name, State::Sensitive, false)?;
                run.expect_description(
                    "grid cell",
                    &name,
                    &format!("{}: no device found", shown_metric(metric)),
                )?;
                run.check_texts("grid cell", &name, &[option, "Unavailable"])?;
                // No action while insensitive: GTK offers none (decisions log D67).
                let node = run.node("grid cell", &name)?;
                let actions = run.driver.actions(&node)?;
                if !actions.is_empty() {
                    return Err(format!("{name:?} offers {actions:?}"));
                }
            }
            None => {
                run.check_state("grid cell", &name, State::Sensitive, true)?;
                run.check_texts("grid cell", &name, &[option])?;
            }
        }
    }
    for (group, reasons) in [
        ("Load", "GPU load: no device found"),
        ("Temperature", "GPU temperature: no device found"),
        ("Power", "GPU power: no device found"),
        (
            "Music",
            "GPU load: no device found\nGPU temperature: no device found",
        ),
    ] {
        run.expect_texts("group", &format!("{group} unavailable reasons"), &[reasons])?;
    }
    run.reset()?;
    run.gone("group", "Load unavailable reasons")?;
    NO_NOTE
}

/// The saved mode loses its reading: its tile stays chosen and usable, flagged "Reading lost",
/// and the reason shows under the preview (States, The saved mode lost a reading).
fn reading_lost(run: &mut Run) -> Result<String> {
    run.reset()?;
    let usage = [2, 4, 6, 21];
    run.fake.service.mode = 2;
    run.fake.service.reasons = usage
        .iter()
        .map(|&m| (m, "Gpu usage: no device found".to_owned()))
        .collect();
    run.fake.service.available = MODES.into_iter().filter(|m| !usage.contains(m)).collect();
    run.fake.service.readings.remove("Gpu usage");
    run.fake.write()?;
    run.expect_state("grid cell", "Load: GPU", State::Selected, true)?;
    run.check_state("grid cell", "Load: GPU", State::Sensitive, true)?;
    run.expect_texts("grid cell", "Load: GPU", &["GPU", "Reading lost"])?;
    run.expect_description(
        "grid cell",
        "Load: GPU",
        "Reading lost: GPU load: no device found",
    )?;
    run.expect_texts(
        "group",
        "Unavailable reading",
        &["GPU load: no device found. The cooler shows dashes until it's back."],
    )?;
    run.expect_state("grid cell", "Load: GPU + RAM", State::Sensitive, false)?;
    run.reset()?;
    run.gone("group", "Unavailable reading")?;
    NO_NOTE
}

/// The sound's status, only in a Music mode (Preview column, Status line).
fn music_sound(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.gone("group", "Music sound status")?;
    run.fake.service.mode = 13;
    run.fake.write()?;
    run.expect_texts("group", "Music sound status", &["Sound connected"])?;
    run.fake.service.audio_connected = false;
    run.fake.write()?;
    run.expect_texts(
        "group",
        "Music sound status",
        &["No sound: spectrum is silent"],
    )?;
    run.fake.service.mode = 1;
    run.fake.write()?;
    run.gone("group", "Music sound status")?;
    NO_NOTE
}

/// The service crashes: systemd waits to restart it, and its name goes. The off screen, "isn't
/// running", says it failed; it returns: the pages come back with no action (The window around
/// a flip).
fn service_stops(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.systemd.units.service = ("enabled", "activating", "auto-restart");
    run.systemd.write()?;
    run.fake.on_bus = false;
    run.fake.write()?;
    run.expect_description("group", "Service message", NOT_RUNNING)?;
    run.note(
        "Not fully on: the display service failed. Turn it on to finish. The journal says why the \
         service failed.",
    )?;
    run.no_terminal()?;
    run.expect_state("toggle button", "Configuration", State::Sensitive, false)?;
    run.gone("grid cell", "Load: CPU")?;
    run.expect_texts("group", "Service status", &["Service not running"])?;
    run.expect_texts("group", "Cooler status", &["Cooler: unknown"])?;
    let mark = run.fake.mark()?;
    run.systemd.units = Units::all_on();
    run.systemd.write()?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    run.gone("group", "Service message")?;
    let calls = run.fake.calls_since(mark)?;
    for call in ["Watch()", "GetState()"] {
        if !calls.iter().any(|c| c == call) {
            return Err(format!("no {call} after the service came back: {calls:?}"));
        }
    }
    NO_NOTE
}

/// Not available in this session: the service refuses every read, so the window holds one
/// message and nothing else, and sends nothing; allowed again, nothing changes until the window
/// is active again, with no polling (Not available in this session).
fn not_available(run: &mut Run) -> Result<String> {
    run.reset()?;
    let mark = run.fake.mark()?;
    run.fake.reads = false;
    run.fake.on_bus = false;
    run.fake.write()?;
    // A new owner of the service's name: the app connects again, and is refused.
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_description(
        "group",
        "Not available in this session",
        "OpenDeepCool can be used only from the session at the PC. This session is remote or \
         not the active one.",
    )?;
    // Only the title and the close icon of the header remain.
    run.node("button", "Close")?;
    run.check_state("button", "Close", State::Sensitive, true)?;
    for (role, name) in [
        ("toggle button", "Configuration"),
        ("button", "Refresh display"),
        ("button", "Back"),
        ("image", "Cooler display preview"),
        ("grid cell", "Load: CPU"),
        ("group", "Cooler status"),
        ("group", "Service message"),
        ("switch", BOOT),
    ] {
        run.gone(role, name)?;
    }
    let window = run.node("window", "OpenDeepCool")?;
    let actions: Vec<String> = run
        .driver
        .actions(&window)?
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    for action in ["win.refresh", "win.configuration", "win.back"] {
        if actions.iter().any(|a| a == action) {
            return Err(format!("{action} offered when not available"));
        }
    }
    // Refused at its first call, `Watch()`, the app asks nothing more.
    let calls = run.fake.calls_since(mark)?;
    if !calls.iter().any(|c| c.ends_with(" refused NotAuthorized")) {
        return Err(format!("no refused read: {calls:?}"));
    }
    let changes = run.fake.changes_since(mark)?;
    if !changes.is_empty() {
        return Err(format!("calls when not available: {changes:?}"));
    }
    // Allowed again: the app doesn't poll, so nothing changes yet.
    run.fake.reads = true;
    run.fake.write()?;
    let quiet = run.fake.mark()?;
    thread::sleep(Duration::from_millis(1500));
    run.node("group", "Not available in this session")?;
    let calls = run.fake.calls_since(quiet)?;
    if !calls.is_empty() {
        return Err(format!("the app asked with no reason to: {calls:?}"));
    }
    // The window active again: it connects again, and the pages come back.
    run.activate_again()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    run.gone("group", "Not available in this session")?;
    let calls = run.fake.calls_since(quiet)?;
    for call in ["Watch()", "GetState()"] {
        if !calls.iter().any(|c| c == call) {
            return Err(format!("no {call} on activation: {calls:?}"));
        }
    }
    NO_NOTE
}

/// Whether a fake systemd log line is a job or a subscription, not a read.
fn is_job(call: &str) -> bool {
    ["StartUnit(", "StopUnit(", "Subscribe()", "Unsubscribe()"]
        .iter()
        .any(|word| call.contains(word))
}

/// Fully on: the Startup card is the Configuration page's last, the switch on and usable, with
/// no note and no commands (Configuration page, Startup).
fn boot_on(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.units(Units::all_on())?;
    run.page("Configuration")?;
    run.node("group", "Startup settings")?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.check_state("switch", BOOT, State::Sensitive, true)?;
    run.gone("group", "Start at boot status")?;
    run.no_terminal()?;
    // The card holds the title and the switch, and a note when one applies: no hint at its foot
    // (owner, 2026-11-19).
    run.gone(
        "label",
        "When it's on, the display service starts at every boot, and the sound for the Music \
         options at every login.",
    )?;
    run.page("Display")?;
    NO_NOTE
}

/// A flip off from the Configuration page, then on from the off screen (A flip, step by step;
/// The window around a flip): exactly one `pkexec` run each, with the program's path and the
/// direction; the switch changing meanwhile; the sound in this session stopped or started
/// after it; and, after `on`, the off screen kept until the service's name has an owner.
fn boot_flip(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.units(Units::all_on())?;
    run.page("Configuration")?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    let mark = run.flip("off")?;
    run.expect_state("switch", BOOT, State::Sensitive, false)?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    thread::sleep(HOLD);
    run.no_note()?;
    if run.systemd.calls_since(mark)?.iter().any(|c| is_job(c)) {
        return Err("the sound moved before pkexec ended".to_owned());
    }
    // What the program leaves: both units disabled, the service stopped and its name gone.
    run.systemd.units = Units {
        sound: ("disabled", "active"),
        ..Units::new_install()
    };
    run.systemd.write()?;
    run.fake.on_bus = false;
    run.fake.write()?;
    run.systemd.answer(
        0,
        "opendeepcool-boot: turned off at boot, and the service stopped\n",
    )?;
    run.systemd.called(mark, &format!("{STOP_SOUND} done"))?;
    run.systemd.units.sound.1 = "inactive";
    run.expect_description("group", "Service message", IS_OFF)?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    run.gone("group", "Start at boot status")?;
    run.expect_texts("group", "Service status", &["Service off"])?;
    run.no_terminal()?;

    // On, from the off screen.
    let mark = run.flip("on")?;
    run.expect_state("switch", BOOT, State::Sensitive, false)?;
    run.check_state("switch", BOOT, State::Checked, true)?;
    thread::sleep(HOLD);
    run.no_note()?;
    // What the program leaves: both enabled, the service started. It takes its name just
    // before it's ready, here after pkexec ends, so the flip waits for it.
    run.systemd.units = Units {
        service: ("enabled", "activating", "start"),
        sound: ("enabled", "inactive"),
        ..Units::new_install()
    };
    run.systemd.write()?;
    run.systemd.answer(
        0,
        "opendeepcool-boot: turned on at boot, and the service started\n",
    )?;
    run.systemd.called(mark, &format!("{START_SOUND} done"))?;
    run.systemd.units.sound.1 = "active";
    thread::sleep(HOLD);
    run.expect_description("group", "Service message", IS_OFF)?;
    run.no_note()?;
    run.check_state("switch", BOOT, State::Sensitive, false)?;
    // The service owns its name: the pages come back on the page last shown, the switch on.
    run.systemd.units.service = ("enabled", "active", "running");
    run.systemd.write()?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("toggle button", "Configuration", State::Pressed, true)?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.gone("group", "Start at boot status")?;
    run.gone("group", "Service message")?;
    let calls = run.systemd.calls_since(mark)?;
    let jobs: Vec<&str> = calls
        .iter()
        .map(String::as_str)
        .filter(|c| is_job(c))
        .collect();
    let want = [
        "user Subscribe()".to_owned(),
        format!("{START_SOUND} done"),
        "user Unsubscribe()".to_owned(),
    ];
    if jobs != want {
        return Err(format!("the sound's start: {jobs:?}"));
    }
    if calls.iter().any(|c| c == "system Subscribe()") {
        return Err("subscribed to the system manager".to_owned());
    }
    run.page("Display")?;
    NO_NOTE
}

/// A flip that doesn't happen: the dialog closed (126), not authorized or no agent (127), and a
/// failed step (1, the program's message). The switch goes back, usable; a closed dialog has no
/// note (it changed nothing: owner, 2026-11-19), the others a note; the app's own step is
/// skipped. The note goes once systemd shows what the
/// flip wanted (States and their text).
fn boot_refused(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.units(Units::all_on())?;
    run.page("Configuration")?;
    // A closed dialog: the switch back, usable, with no note.
    let mark = run.flip("off")?;
    run.expect_state("switch", BOOT, State::Sensitive, false)?;
    run.systemd.answer(126, "")?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.check_state("switch", BOOT, State::Checked, true)?;
    run.gone("group", "Start at boot status")?;
    let calls = run.systemd.calls_since(mark)?;
    if calls.iter().any(|c| c.contains("StopUnit")) {
        return Err(format!("the sound stopped after exit 126: {calls:?}"));
    }
    for (code, stderr, note) in [
        (
            127,
            "the fake pkexec: not authorized\n",
            "Not changed: the password wasn't accepted, or no password dialog could open in this \
             session.",
        ),
        (
            1,
            "Failed to disable unit: Access denied\nopendeepcool-boot: step 1 of 2, `systemctl \
             --global disable opendeepcool-audio.service`, failed: exit status: 1\n",
            "Didn't finish: step 1 of 2, `systemctl --global disable \
             opendeepcool-audio.service`, failed: exit status: 1. The switch shows what's set \
             now; the journal says more.",
        ),
    ] {
        let mark = run.flip("off")?;
        run.expect_state("switch", BOOT, State::Sensitive, false)?;
        run.systemd.answer(code, stderr)?;
        run.note(note)?;
        run.expect_state("switch", BOOT, State::Sensitive, true)?;
        run.check_state("switch", BOOT, State::Checked, true)?;
        run.no_terminal()?;
        let calls = run.systemd.calls_since(mark)?;
        if calls.iter().any(|c| c.contains("StopUnit")) {
            return Err(format!("the sound stopped after exit {code}: {calls:?}"));
        }
    }
    // Turned off in a terminal: the note goes once the window is active again.
    run.units(Units::new_install())?;
    run.gone("group", "Start at boot status")?;
    run.no_terminal()?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    run.page("Display")?;
    NO_NOTE
}

/// Only the sound in this session missing: a flip on starts it through the user's own manager,
/// with no `pkexec` and no password; when it doesn't start, its note.
fn boot_only_sound(run: &mut Run) -> Result<String> {
    run.reset()?;
    let missing = Units {
        sound: ("enabled", "inactive"),
        ..Units::all_on()
    };
    run.units(missing.clone())?;
    run.page("Configuration")?;
    run.note(
        "Not fully on: the sound for Music isn't running in this session. Turn it on to start \
         it, with no password.",
    )?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    let mark = run.systemd.mark()?;
    run.act("switch", BOOT, "Toggle")?;
    run.systemd.called(mark, &format!("{START_SOUND} done"))?;
    run.systemd.units.sound.1 = "active";
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.gone("group", "Start at boot status")?;
    let runs = run.systemd.pkexec_since(mark)?;
    if !runs.is_empty() {
        return Err(format!("pkexec ran: {runs:?}"));
    }
    // The sound doesn't start: its note.
    run.units(Units {
        job: "failed",
        ..missing
    })?;
    let mark = run.systemd.mark()?;
    run.act("switch", BOOT, "Toggle")?;
    run.systemd.called(mark, &format!("{START_SOUND} failed"))?;
    run.note(
        "Didn't finish: the sound for Music didn't start in this session. It starts at your next \
         login; the journal says more.",
    )?;
    run.no_terminal()?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    // The note stays at a read that still misses the sound, and goes once a read shows it
    // running: the state the flip wanted.
    run.units(Units {
        job: "failed",
        ..missing
    })?;
    run.check_texts("group", "Start at boot status", &[
        "Didn't finish: the sound for Music didn't start in this session. It starts at your next \
         login; the journal says more.",
    ])?;
    run.units(Units::all_on())?;
    run.gone("group", "Start at boot status")?;
    run.no_terminal()?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.page("Display")?;
    NO_NOTE
}

/// Partly on: the note names what's missing, and a flip on runs `pkexec on` whatever it is
/// (The note).
fn boot_partly(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    for (units, note) in [
        (
            Units {
                service: ("disabled", "active", "running"),
                ..Units::new_install()
            },
            "Not fully on: the display service runs now, but doesn't start at boot, and the sound \
             for Music is off. Turn it on to finish.",
        ),
        (
            Units {
                sound: ("disabled", "active"),
                ..Units::all_on()
            },
            "Not fully on: the sound for Music runs here, but doesn't start at login. Turn it on \
             to finish.",
        ),
    ] {
        run.units(units)?;
        run.note(note)?;
        run.check_state("switch", BOOT, State::Checked, false)?;
        run.check_state("switch", BOOT, State::Sensitive, true)?;
        run.no_terminal()?;
    }
    let mark = run.flip("on")?;
    run.systemd.units = Units::all_on();
    run.systemd.write()?;
    run.systemd.answer(0, "")?;
    run.systemd.called(mark, &format!("{START_SOUND} done"))?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.gone("group", "Start at boot status")?;
    run.page("Display")?;
    NO_NOTE
}

/// The off screen's states (Where its state comes from; States and their text): starting, a
/// masked unit, a failed read and one that doesn't answer within 1 s; each insensitive, with its
/// note.
fn boot_off_screen(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.systemd.units.service = ("enabled", "activating", "start");
    run.systemd.write()?;
    run.fake.on_bus = false;
    run.fake.write()?;
    run.expect_description("group", "Service message", NOT_RUNNING)?;
    run.note("The display service is starting.")?;
    run.expect_state("switch", BOOT, State::Sensitive, false)?;
    for (units, note) in [
        (
            Units {
                service: ("masked", "inactive", "dead"),
                ..Units::all_on()
            },
            "Can't change it here: systemd reports opendeepcool.service as “masked”.",
        ),
        (
            Units {
                service: ("", "inactive", "dead"),
                ..Units::all_on()
            },
            "Can't tell whether OpenDeepCool is on: reading it from systemd failed.",
        ),
        (
            Units {
                delay_ms: 1500,
                ..Units::all_on()
            },
            "Can't tell whether OpenDeepCool is on: reading it from systemd failed.",
        ),
    ] {
        run.units(units)?;
        run.note(note)?;
        run.expect_description("group", "Service message", NOT_RUNNING)?;
        run.check_state("switch", BOOT, State::Sensitive, false)?;
        run.check_state("switch", BOOT, State::Checked, false)?;
        run.no_terminal()?;
    }
    run.systemd.units = Units::all_on();
    run.systemd.write()?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    NO_NOTE
}

/// A session logind reports as remote: the off screen keeps its heading, but has no switch,
/// since polkit would refuse the flip there (Who may flip it). logind's change is read when it
/// comes.
fn boot_remote(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.systemd.units = Units::new_install();
    run.systemd.write()?;
    run.fake.on_bus = false;
    run.fake.write()?;
    run.expect_description("group", "Service message", IS_OFF)?;
    run.node("switch", BOOT)?;
    run.systemd.units.session = (true, true);
    run.systemd.write()?;
    run.gone("switch", BOOT)?;
    run.gone("group", "Startup settings")?;
    run.expect_description("group", "Service message", IS_OFF)?;
    run.no_terminal()?;
    run.systemd.units = Units::all_on();
    run.systemd.write()?;
    run.node("switch", BOOT)?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    NO_NOTE
}

/// An Omarchy theme switch, done as Omarchy does it: the new theme staged, then moved in. The
/// app reloads it and keeps its page (Theme; States, Live theme switch).
fn theme(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    let current = run.omarchy_state.join("current");
    let next = current.join("next-theme");
    fs::create_dir_all(&next).map_err(|e| format!("{}: {e}", next.display()))?;
    fs::write(next.join("colors.toml"), COLORS).map_err(|e| format!("colors.toml: {e}"))?;
    fs::rename(&next, current.join("theme")).map_err(|e| format!("the theme's move: {e}"))?;
    run.logged("opendeepcool: theme: reloaded, the Omarchy theme")?;
    run.check_state("toggle button", "Configuration", State::Pressed, true)?;
    run.node("toggle button", "Rotation 0°")?;
    // The Theme card follows it live: the Omarchy theme is the system's, followed by default, so
    // the switch is on and Dark and Light are dimmed on its mode (light).
    run.expect_state("switch", "Use system theme", State::Checked, true)?;
    run.check_state("switch", "Use system theme", State::Sensitive, true)?;
    run.expect_texts("group", "Theme status", &["Using the Omarchy theme."])?;
    run.check_state("toggle button", "Light theme", State::Pressed, true)?;
    run.check_state("toggle button", "Dark theme", State::Sensitive, false)?;
    run.check_state("toggle button", "Light theme", State::Sensitive, false)?;
    // The whole Default Theme is dimmed while the system theme is followed, the saved opacity and
    // corners still shown (Phase 11a Task 50).
    for name in [
        "Opaque window",
        "Transparent window",
        "Glassy window",
        "Sharp corners",
        "Rounded corners",
    ] {
        run.check_state("toggle button", name, State::Sensitive, false)?;
    }
    run.check_state("toggle button", "Opaque window", State::Pressed, true)?;
    run.check_state("toggle button", "Rounded corners", State::Pressed, true)?;
    // The switch turned off asks the service, and keeps its state until the service confirms;
    // then OpenDeepCool's own theme shows, and the buttons come alive on `DarkTheme`.
    run.change(
        |run| run.act("switch", "Use system theme", "Toggle"),
        "SetUseSystemTheme(false)",
        |run| run.check_state("toggle button", "Light theme", State::Sensitive, false),
        |service| service.use_system_theme = false,
        |run| {
            run.expect_state("switch", "Use system theme", State::Checked, false)?;
            run.expect_state("toggle button", "Dark theme", State::Sensitive, true)?;
            run.expect_state("toggle button", "Dark theme", State::Pressed, true)?;
            run.expect_texts(
                "group",
                "Theme status",
                &["Using OpenDeepCool's dark theme."],
            )
        },
    )?;
    // Window Opacity: asked with its word, the saved one stays pressed until the service says so.
    run.change(
        |run| run.act("toggle button", "Glassy window", "Click"),
        "SetWindowOpacity(\"glassy\")",
        |run| run.check_state("toggle button", "Opaque window", State::Pressed, true),
        |service| service.window_opacity = "glassy",
        |run| run.expect_state("toggle button", "Glassy window", State::Pressed, true),
    )?;
    run.change(
        |run| run.act("toggle button", "Sharp corners", "Click"),
        "SetWindowCorners(\"sharp\")",
        |run| run.check_state("toggle button", "Rounded corners", State::Pressed, true),
        |service| service.window_corners = "sharp",
        |run| run.expect_state("toggle button", "Sharp corners", State::Pressed, true),
    )?;
    // Light: asked, the saved Dark stays pressed until the service says so.
    run.change(
        |run| run.act("toggle button", "Light theme", "Click"),
        "SetDarkTheme(false)",
        |run| run.check_state("toggle button", "Dark theme", State::Pressed, true),
        |service| service.dark_theme = false,
        |run| {
            run.expect_state("toggle button", "Light theme", State::Pressed, true)?;
            run.expect_texts(
                "group",
                "Theme status",
                &["Using OpenDeepCool's light theme."],
            )
        },
    )?;
    // And on again: the Omarchy theme is followed once more.
    run.change(
        |run| run.act("switch", "Use system theme", "Toggle"),
        "SetUseSystemTheme(true)",
        |_| Ok(()),
        |service| {
            service.use_system_theme = true;
            service.dark_theme = true;
        },
        |run| run.expect_texts("group", "Theme status", &["Using the Omarchy theme."]),
    )?;
    run.page("Display")?;
    NO_NOTE
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

/// The live tiles (Phase 11a Task 14; design: Live tiles). A picture is no accessible
/// property, and the tiles' names and descriptions are static, so layer 4 checks what a person
/// with assistive technology gets: the flags follow what the service lists, not the readings. A
/// reading the service has no value for, with no reason listed, leaves every tile as it was
/// (its picture draws dashes, which the app's own tests check); with the reason listed for the
/// modes that show it, those tiles are "Unavailable", or "Reading lost" on the saved mode's,
/// and the others are untouched; the reading and the reason gone, everything is as it was.
fn tiles_live(run: &mut Run) -> Result<String> {
    run.reset()?;
    flags(run, &[], None)?;
    // GPU usage has no value for a moment: nothing changes for assistive technology.
    run.fake.service.readings.remove("Gpu usage");
    run.fake.write()?;
    flags(run, &[], None)?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    // The service lists the reason for the modes that show it.
    let usage = [2_u32, 4, 6, 21];
    run.fake.service.reasons = usage
        .iter()
        .map(|&m| (m, "Gpu usage: no device found".to_owned()))
        .collect();
    run.fake.service.available = MODES.into_iter().filter(|m| !usage.contains(m)).collect();
    run.fake.write()?;
    flags(run, &usage, None)?;
    // The saved mode is one of them: its tile stays usable and says what was lost.
    run.fake.service.mode = 2;
    run.fake.write()?;
    flags(run, &[4, 6, 21], Some(2))?;
    // The reading comes back, and the reason goes.
    run.fake.service.readings.insert("Gpu usage", (63, 1));
    run.fake.service.reasons.clear();
    run.fake.service.available = MODES.to_vec();
    run.fake.write()?;
    flags(run, &[], None)?;
    run.check_texts("grid cell", "Load: GPU", &["GPU"])?;
    NO_NOTE
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
const COST_CEILING: [(&str, f64); 3] = [("steady", 1.0), ("busy", 1.0), ("still", 0.01)];

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

fn tile_cost(run: &mut Run) -> Result<String> {
    let pid = run
        .app_pid
        .ok_or("--app-pid is needed for the tiles' cost")?;
    let levels = |n: u32| -> Vec<f64> {
        (0..14)
            .map(|c| (f64::from((n + c) % 20) + 1.0) / 21.0)
            .collect()
    };
    let mut notes = Vec::new();
    let mut measured = Vec::new();
    for (state, ceiling) in COST_CEILING {
        run.reset()?;
        run.fake.service.readings.insert("CPU Usage", (150, 1));
        let used = match state {
            // Readings once a second, one of them 100 or more: the scrolling tiles.
            "steady" => {
                run.fake.write()?;
                cost(run, pid, |run, tick| {
                    if tick % 10 == 0 {
                        run.fake
                            .service
                            .readings
                            .insert("CPU Usage", (150 + i64::from(tick / 10), 1));
                        run.fake.write()?;
                    }
                    Ok(())
                })?
            }
            // A Music mode with sound, ten `Levels` a second that change each time: every tile
            // that moves, at its busiest.
            "busy" => {
                run.fake.service.mode = 13;
                run.fake.write()?;
                cost(run, pid, |run, tick| {
                    run.fake.levels = Some(levels(tick));
                    run.fake.write()
                })?
            }
            // The Configuration page: no tile is on screen, so nothing moves.
            _ => {
                run.fake.write()?;
                run.page("Configuration")?;
                let used = cost(run, pid, |_, _| Ok(()))?;
                run.page("Display")?;
                used
            }
        };
        run.fake.levels = None;
        measured.push((state, used, ceiling));
        notes.push(format!("{state} {:.2} % of a core", used * 100.0));
    }
    run.reset()?;
    for (state, used, ceiling) in &measured {
        if used > ceiling {
            return Err(format!(
                "{state}: {:.1} % of a core, over its ceiling of {:.0} % ({})",
                used * 100.0,
                ceiling * 100.0,
                notes.join(", ")
            ));
        }
    }
    Ok(format!("{} (ceilings 100 %, 100 %, 1 %)", notes.join(", ")))
}
