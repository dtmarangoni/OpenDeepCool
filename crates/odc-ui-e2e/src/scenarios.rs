//! The scenarios (Phase 10 plan, Task 8): the rows of the design's coverage check and States
//! table (`docs/design/settings-app.md`) that AT-SPI can reach, each through the real app.
//!
//! Every change is checked as the design's Refused changes asks: the control's action, then
//! exactly one call at the fake, then the control still on the saved value until the driver,
//! as the service, confirms the change with a new state, then the control on the new value.
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
use crate::fake::{self, DEDICATED, Fake, INTEGRATED, NO_RULE, QUERY_FAILED, Service};

/// How long a control must stay on the saved value after its call, without the service's word.
const HOLD: Duration = Duration::from_millis(250);

/// The design's tiles (Display page, Tiles), mode 1 first: group and option.
pub const TILES: [(&str, &str); 23] = [
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
    ("Music", "Emoji 1"),
    ("Music", "Emoji 2"),
    ("Music", "Emoji 3"),
    ("Music", "Emoji 4"),
    ("Music", "CPU temperature"),
    ("Music", "GPU temperature"),
    ("Music", "CPU usage"),
    ("Music", "GPU usage"),
    ("Music", "RAM usage"),
    ("Music", "Dancer"),
];

/// A tile's accessible name, `<group>: <option>`.
pub fn tile(mode: u32) -> String {
    let (group, option) = TILES[mode as usize - 1];
    format!("{group}: {option}")
}

/// The preview's caption, `<group> · <option>`.
pub fn caption(mode: u32) -> String {
    let (group, option) = TILES[mode as usize - 1];
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

/// The display's name for a metric the service names by the spec (decisions log D35).
fn shown_metric(metric: &str) -> String {
    metric.replacen("Gpu", "GPU", 1)
}

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
    app_log: PathBuf,
    frame_log: PathBuf,
    omarchy_state: PathBuf,
    wayland_display: PathBuf,
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
            app_log: paths.app_log.clone(),
            frame_log: paths.frame_log.clone(),
            omarchy_state: paths.omarchy_state.clone(),
            wayland_display: paths.wayland_display.clone(),
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

    /// Shows the page `name` through its switcher toggle.
    fn page(&self, name: &str) -> Result<()> {
        let toggle = self.node("toggle button", name)?;
        if !self.driver.has(&toggle, State::Pressed)? {
            self.driver.act(&toggle, "Click")?;
        }
        self.expect_state("toggle button", name, State::Pressed, true)?;
        // A control of the page, shown.
        let control = if name == "Display" {
            "Celsius"
        } else {
            "Rotation 0°"
        };
        self.node("toggle button", control).map(|_| ())
    }

    /// The first start's state on the bus, every answer given, no list open, the Display page.
    fn reset(&mut self) -> Result<()> {
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
        self.node("toggle button", "Display")?;
        for combo in ["GPU", "Cooler fans input", "Pump input"] {
            if let Some(node) = self.driver.find("combo box", combo)?
                && self.driver.has(&node, State::Expanded)?
            {
                self.close(combo)?;
            }
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

pub const ALL: [(&str, Scenario); 24] = [
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
    ("inputs", inputs),
    ("missing_input", missing_input),
    ("no_fan_sensors", no_fan_sensors),
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
    ("theme", theme),
];

const NO_NOTE: Result<String> = Ok(String::new());

/// Service not running, at the start: the fake hasn't taken the bus (Banners, Service).
fn service_down(run: &mut Run) -> Result<String> {
    run.expect_description(
        "group",
        "Service banner",
        "The OpenDeepCool service isn't running. The cooler shows nothing until it starts. \
         Start it from a terminal: sudo systemctl start opendeepcool.service The app connects \
         by itself once it runs.",
    )?;
    run.check_state("button", "Copy start command", State::Sensitive, true)?;
    for toggle in ["Display", "Configuration"] {
        run.check_state("toggle button", toggle, State::Sensitive, false)?;
    }
    run.check_state("button", "Refresh display", State::Sensitive, false)?;
    run.node("label", "Settings appear here when the service runs")?;
    run.expect_texts("group", "Cooler status", &["Cooler: unknown"])?;
    run.expect_texts("group", "Service status", &["Service not running"])?;
    run.expect_texts("group", "Cooler fans speed", &["Cooler fans", "—"])?;
    run.expect_texts("group", "Pump speed", &["Pump", "—"])?;
    run.expect_description("image", "Cooler display preview", "No picture")?;
    run.gone("grid cell", "Load: CPU")?;
    let calls = run.fake.calls_since(0)?;
    if !calls.is_empty() {
        return Err(format!("calls with no service: {calls:?}"));
    }
    // No Omarchy folder: the default theme (Theme).
    run.logged("opendeepcool: theme: the default theme")?;
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
    run.gone("group", "Service banner")?;
    run.expect_texts("group", "Cooler status", &["Cooler connected"])?;
    run.expect_texts("group", "Service status", &["Service running"])?;
    run.gone("group", "Music sound status")?;
    run.expect_description("image", "Cooler display preview", "Load · CPU")?;
    run.node("label", "Load · CPU")?;
    run.node("label", "Shown on the cooler now")?;
    run.node("button", "Choose cooler fans input")?;
    run.node("button", "Choose pump input")?;
    run.check_state("toggle button", "Celsius", State::Pressed, true)?;
    run.check_state("toggle button", "Fahrenheit", State::Pressed, false)?;
    for toggle in ["Display", "Configuration"] {
        run.check_state("toggle button", toggle, State::Sensitive, true)?;
    }
    run.check_state("button", "Refresh display", State::Sensitive, true)?;
    run.page("Configuration")?;
    for degrees in [0, 90, 180, 270] {
        let name = format!("Rotation {degrees}°");
        run.check_state("toggle button", &name, State::Pressed, degrees == 0)?;
    }
    run.expect_texts("combo box", "GPU", &["Automatic"])?;
    run.expect_description("combo box", "GPU", &format!("Using {DEDICATED_LABEL}"))?;
    run.expect_texts(
        "group",
        "GPU in use",
        &[&format!("Using {DEDICATED_LABEL}.")],
    )?;
    run.expect_texts("combo box", "Cooler fans input", &["None"])?;
    run.expect_texts("combo box", "Pump input", &["None"])?;
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
        ("toggle button", "Display"),
        ("toggle button", "Configuration"),
        ("button", "Refresh display"),
        ("image", "Cooler display preview"),
        ("group", "Cooler status"),
        ("group", "Service status"),
        ("group", "Cooler fans speed"),
        ("group", "Pump speed"),
        ("button", "Choose cooler fans input"),
        ("button", "Choose pump input"),
    ]);
    let mut display = owned(&[
        ("toggle button", "Celsius"),
        ("toggle button", "Fahrenheit"),
        ("grid", "Load options"),
        ("grid", "Temperature options"),
        ("grid", "Power options"),
        ("grid", "Music options"),
    ]);
    for mode in 1..=23 {
        // Each tile, and its picture named as its tile (decisions log D57).
        display.push(("grid cell", tile(mode)));
        display.push(("image", tile(mode)));
    }
    let configuration = owned(&[
        ("group", "Display settings"),
        ("group", "Cooler settings"),
        ("group", "Music settings"),
        ("toggle button", "Rotation 0°"),
        ("toggle button", "Rotation 90°"),
        ("toggle button", "Rotation 180°"),
        ("toggle button", "Rotation 270°"),
        ("combo box", "GPU"),
        ("group", "GPU in use"),
        ("combo box", "Cooler fans input"),
        ("combo box", "Pump input"),
        ("switch", "Peak dots"),
        ("slider", "Bar fall"),
        ("slider", "Peak fall"),
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

/// Each of the 23 tiles asks for its own mode, once, and shows as chosen only once the service
/// confirms it; the preview's caption follows (Display page, Selection; Coverage check, One
/// choice, one mode). Music modes add the sound's status (Preview column, Status line).
fn tiles(run: &mut Run) -> Result<String> {
    run.reset()?;
    let mut saved = 1;
    for mode in (2..=23).chain([1]) {
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
                if (13..=23).contains(&mode) {
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
            service.available = (1..=23)
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
    run.node("combo box", "Cooler fans input")?;
    run.page("Display")?;
    NO_NOTE
}

/// The cooler fans and pump inputs: None first, each input with its live speed in the open
/// list, a pick, the speed under the preview, an unreadable speed, and None again
/// (Configuration page, Cooler; Preview column, Fan and pump RPM).
fn inputs(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    run.open("Cooler fans input")?;
    run.expect_items(
        "Cooler fans input",
        &[
            ("None", ""),
            ("acme6000 · fan1", "1,180 RPM"),
            ("acme6000 · fan2", "3,400 RPM"),
            ("acme6000 · Rear", "940 RPM"),
        ],
    )?;
    // The speeds change while the list is open.
    run.fake.service.fans[0].rpm = Some(1250);
    run.fake.write()?;
    run.expect_items(
        "Cooler fans input",
        &[
            ("None", ""),
            ("acme6000 · fan1", "1,250 RPM"),
            ("acme6000 · fan2", "3,400 RPM"),
            ("acme6000 · Rear", "940 RPM"),
        ],
    )?;
    let input =
        |call: &str, n: u32| format!("{call}(\"{}\", \"{}\", {n})", fake::CHIP, fake::DEVICE);
    run.change(
        |run| run.pick("Cooler fans input", "acme6000 · fan1"),
        &input("SetCoolerFanInput", 1),
        |run| run.check_texts("combo box", "Cooler fans input", &["None"]),
        |service| service.cooler_fan = Some(1),
        |run| {
            run.expect_texts("combo box", "Cooler fans input", &["acme6000 · fan1"])?;
            run.expect_texts("group", "Cooler fans speed", &["Cooler fans", "1,250 RPM"])
        },
    )?;
    run.open("Pump input")?;
    run.change(
        |run| run.pick("Pump input", "acme6000 · fan2"),
        &input("SetPumpInput", 2),
        |run| run.check_texts("combo box", "Pump input", &["None"]),
        |service| service.pump = Some(2),
        |run| {
            run.expect_texts("combo box", "Pump input", &["acme6000 · fan2"])?;
            run.expect_texts("group", "Pump speed", &["Pump", "3,400 RPM"])
        },
    )?;
    // The pump's speed changes, then can't be read.
    run.fake.service.fans[1].rpm = Some(3500);
    run.fake.write()?;
    run.expect_texts("group", "Pump speed", &["Pump", "3,500 RPM"])?;
    run.fake.service.fans[1].rpm = None;
    run.fake.write()?;
    run.expect_texts("group", "Pump speed", &["Pump", "No reading"])?;
    run.open("Cooler fans input")?;
    run.change(
        |run| run.pick("Cooler fans input", "None"),
        "SetCoolerFanInput(\"\", \"\", 0)",
        |run| run.check_texts("combo box", "Cooler fans input", &["acme6000 · fan1"]),
        |service| service.cooler_fan = None,
        |run| {
            run.expect_texts("combo box", "Cooler fans input", &["None"])?;
            run.node("button", "Choose cooler fans input").map(|_| ())
        },
    )?;
    run.page("Display")?;
    NO_NOTE
}

/// A saved input that isn't there: "— not found" on its picker, its hint, "Input not found"
/// under the preview, and no other input's speed (States, Saved fan or pump input missing).
fn missing_input(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.service.cooler_fan = Some(4);
    run.fake.service.cooler_fan_present = false;
    run.fake.write()?;
    run.expect_texts(
        "group",
        "Cooler fans speed",
        &["Cooler fans", "Input not found"],
    )?;
    run.page("Configuration")?;
    run.expect_texts(
        "combo box",
        "Cooler fans input",
        &["acme6000 · fan4 — not found"],
    )?;
    run.expect_description("combo box", "Cooler fans input", "not found")?;
    run.node(
        "label",
        "The saved input isn't found. Pick another, or it comes back when the input does.",
    )?;
    run.page("Display")?;
    NO_NOTE
}

/// No fan sensors: both pickers insensitive with the note, and the preview column says so
/// (States, No fan sensors).
fn no_fan_sensors(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.service.fans = Vec::new();
    run.fake.write()?;
    run.expect_texts(
        "group",
        "Cooler fans speed",
        &["Cooler fans", "No fan sensors found"],
    )?;
    run.expect_texts("group", "Pump speed", &["Pump", "No fan sensors found"])?;
    run.page("Configuration")?;
    for combo in ["Cooler fans input", "Pump input"] {
        run.expect_state("combo box", combo, State::Sensitive, false)?;
        run.expect_texts("combo box", combo, &["No fan sensors found"])?;
    }
    run.node(
        "label",
        "No fan sensors were found. The board's sensor driver may not be loaded; the journal \
         says more.",
    )?;
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

/// The shortcuts' actions, which the window offers AT-SPI: the two pages and Refresh
/// (Keyboard, Shortcuts). A key typed through AT-SPI's device event controller is tried too.
fn shortcuts(run: &mut Run) -> Result<String> {
    run.reset()?;
    let window = run.node("window", "OpenDeepCool")?;
    run.driver.act(&window, "win.page-config")?;
    run.expect_state("toggle button", "Configuration", State::Pressed, true)?;
    run.node("toggle button", "Rotation 0°")?;
    run.driver.act(&window, "win.page-display")?;
    run.expect_state("toggle button", "Display", State::Pressed, true)?;
    run.node("toggle button", "Celsius")?;
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
        "Not changed. GPU usage: no device found",
    )?;
    thread::sleep(HOLD);
    run.check_state("grid cell", "Load: CPU", State::Selected, true)?;
    run.check_state("grid cell", "Load: GPU", State::Selected, false)?;
    run.fake.refuse = Some("NotAuthorized: this session isn't the active one".to_owned());
    run.fake.write()?;
    const NOT_YOURS: &str = "Not changed. This session may not change settings.";
    run.refused(
        |run| run.act("toggle button", "Fahrenheit", "Click"),
        "SetTemperatureUnit(\"fahrenheit\") refused NotAuthorized",
        NOT_YOURS,
    )?;
    run.expect_state("toggle button", "Celsius", State::Pressed, true)?;
    run.refused(
        |run| run.act("button", "Refresh display", "Click"),
        "Refresh() refused NotAuthorized",
        "Not refreshed. This session may not change settings.",
    )?;
    run.page("Configuration")?;
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

/// The cooler's states: one banner at a time, the status and the caption's line, everything
/// still changeable; `write-failed` as connected for its first 2 s (Banners; States).
fn cooler_states(run: &mut Run) -> Result<String> {
    run.reset()?;
    const NOT_CONNECTED: (Option<&str>, &str, &str) = (
        Some(
            "The cooler isn't connected. Changes are still saved, and the cooler shows them \
             when it's back.",
        ),
        "Cooler not connected",
        "Not on the cooler: it isn't connected",
    );
    for (state, (banner, status, line)) in [
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
                "Not on the cooler: the service can't open it",
            ),
        ),
        // The dev copy's own state, and one a later service might send: as not-found.
        ("released", NOT_CONNECTED),
        ("on fire", NOT_CONNECTED),
        (
            "connected",
            (None, "Cooler connected", "Shown on the cooler now"),
        ),
    ] {
        run.fake.service.cooler = state;
        run.fake.write()?;
        match banner {
            Some(text) => run.expect_description("group", "Cooler banner", text)?,
            None => run.gone("group", "Cooler banner")?,
        }
        run.expect_texts("group", "Cooler status", &[status])
            .map_err(|e| format!("{state}: {e}"))?;
        run.node("label", line)?;
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
    run.node("label", "Shown on the cooler now")?;
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
    run.node("label", "Not on the cooler: sending it fails")?;
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
    run.fake.service.available = (1..=23)
        .filter(|m| !GPU_MODES.iter().any(|(g, _)| g == m))
        .collect();
    run.fake.write()?;
    for mode in 1..=23 {
        let name = tile(mode);
        let (_, option) = TILES[mode as usize - 1];
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
        ("Load", "GPU usage: no device found"),
        ("Temperature", "GPU temperature: no device found"),
        ("Power", "GPU power: no device found"),
        (
            "Music",
            "GPU temperature: no device found\nGPU usage: no device found",
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
    run.fake.service.available = (1..=23).filter(|m| !usage.contains(m)).collect();
    run.fake.write()?;
    run.expect_state("grid cell", "Load: GPU", State::Selected, true)?;
    run.check_state("grid cell", "Load: GPU", State::Sensitive, true)?;
    run.expect_texts("grid cell", "Load: GPU", &["GPU", "Reading lost"])?;
    run.expect_description(
        "grid cell",
        "Load: GPU",
        "Reading lost: GPU usage: no device found",
    )?;
    run.expect_texts(
        "group",
        "Unavailable reading",
        &["GPU usage: no device found. The cooler shows dashes until it's back."],
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

/// The service stops: its banner and the pages' message; it returns: the pages come back with
/// no action (Banners, Service).
fn service_stops(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.on_bus = false;
    run.fake.write()?;
    run.node("group", "Service banner")?;
    run.node("label", "Settings appear here when the service runs")?;
    run.expect_state("toggle button", "Display", State::Sensitive, false)?;
    run.gone("grid cell", "Load: CPU")?;
    run.expect_texts("group", "Service status", &["Service not running"])?;
    run.expect_texts("group", "Cooler status", &["Cooler: unknown"])?;
    let mark = run.fake.mark()?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    run.gone("group", "Service banner")?;
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
    for (role, name) in [
        ("toggle button", "Display"),
        ("toggle button", "Configuration"),
        ("button", "Refresh display"),
        ("image", "Cooler display preview"),
        ("grid cell", "Load: CPU"),
        ("group", "Cooler status"),
        ("group", "Service banner"),
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
    for action in ["win.refresh", "win.page-display", "win.page-config"] {
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
    run.page("Display")?;
    NO_NOTE
}
