//! The view model (Phase 10 plan, Task 3): what the window shows, from what the app knows of
//! the service and the window's own state, as plain data (`docs/design/settings-app.md`,
//! States). No GTK and no clock: the window draws a [`View`], and the `write-failed` wait is an
//! input, [`Ui::sending_failed_long`], kept by [`SendingFails`].

use std::collections::HashMap;
use std::time::{Duration, Instant};

use odc_ipc::{
    CoolerState, FallStop, FanInput, FanSpeed, Gpu, GpuKind, Input, State, TemperatureUnit,
};
use odc_render::{Kind, Mode, mode_info};
use odc_sensors::Metric;

/// What the app knows of the service.
#[derive(Clone, Debug, PartialEq)]
pub enum Service {
    /// Its bus name has no owner.
    Down,
    /// It refused `GetState()` with `NotAuthorized`: a remote or inactive session.
    Refused,
    /// Its state, from `GetState()` and every `StateChanged` since.
    Up(Box<State>),
}

/// The page shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Display,
    Configuration,
}

/// The window's own state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ui {
    pub page: Page,
    /// Whether `CoolerState` has been `write-failed` for [`SENDING_FAILS_AFTER`] (Banners).
    pub sending_failed_long: bool,
}

/// How long `write-failed` shows as connected (Banners: one bad write, or the cooler being
/// unplugged, lasts about a tick).
pub const SENDING_FAILS_AFTER: Duration = Duration::from_secs(2);

/// The `write-failed` wait: when the state became `write-failed`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SendingFails {
    since: Option<Instant>,
}

impl SendingFails {
    /// Follows a new cooler state at `now`. When the wait starts, how long until it ends, for a
    /// one-shot timer; the window then looks at [`SendingFails::lasted`] again.
    pub fn follow(&mut self, state: Option<CoolerState>, now: Instant) -> Option<Duration> {
        if state != Some(CoolerState::WriteFailed) {
            self.since = None;
            return None;
        }
        if self.since.is_some() {
            return None;
        }
        self.since = Some(now);
        Some(SENDING_FAILS_AFTER)
    }

    /// Whether `write-failed` has lasted its 2 s at `now`.
    pub fn lasted(&self, now: Instant) -> bool {
        self.since
            .is_some_and(|since| now.saturating_duration_since(since) >= SENDING_FAILS_AFTER)
    }
}

/// What the window shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum View {
    /// Not available in this session: the header bar keeps only the app name, and the body is
    /// [`NOT_AVAILABLE_HEADING`] and [`NOT_AVAILABLE_TEXT`]; nothing else.
    NotAvailable,
    /// The service isn't running: the service banner, the switcher and Refresh insensitive, the
    /// pages replaced by [`SERVICE_DOWN_PAGES`], the preview all unlit, and this column.
    ServiceDown(Column),
    /// The normal screens.
    Normal(Box<Normal>),
}

pub const NOT_AVAILABLE_HEADING: &str = "OpenDeepCool can be used only from the session at the PC";
pub const NOT_AVAILABLE_TEXT: &str = "This session is remote or not the active one.";

/// The service banner's parts: what happened, in bold; the fix; the start command; and how the
/// app goes on.
pub const SERVICE_BANNER: [&str; 4] = [
    "The OpenDeepCool service isn't running.",
    "The cooler shows nothing until it starts. Start it from a terminal:",
    "sudo systemctl start opendeepcool.service",
    "The app connects by itself once it runs.",
];
pub const SERVICE_DOWN_PAGES: &str = "Settings appear here when the service runs";

/// The normal screens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Normal {
    pub page: Page,
    /// At most one cooler banner.
    pub banner: Option<Banner>,
    pub column: Column,
    pub display: DisplayPage,
    pub config: ConfigPage,
}

/// A cooler banner, chosen by `CoolerState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Banner {
    /// `not-found`, and any value the app doesn't show otherwise (warning).
    NotConnected,
    /// `no-access` (error).
    NoAccess,
    /// `write-failed` for 2 s or more (warning).
    SendingFails,
}

impl Banner {
    /// What happened, in bold, then the rest.
    pub fn text(self) -> (&'static str, &'static str) {
        match self {
            Banner::NotConnected => (
                "The cooler isn't connected.",
                "Changes are still saved, and the cooler shows them when it's back.",
            ),
            Banner::NoAccess => (
                "The cooler is plugged in, but the service can't open it.",
                "The udev rule that gives the service access is missing or not applied yet. Restart the PC; if that doesn't help, reinstall OpenDeepCool. The journal says more.",
            ),
            Banner::SendingFails => (
                "The cooler is connected, but sending it the picture fails.",
                "The service keeps trying by itself, and changes are still saved. The journal says more.",
            ),
        }
    }

    /// An error banner, or else a warning.
    pub fn is_error(self) -> bool {
        self == Banner::NoAccess
    }
}

/// The preview column, under the preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// `<group> · <option>`, or "No picture".
    pub caption: String,
    /// Where the picture is.
    pub line: &'static str,
    /// The saved mode's lost reading.
    pub reason: Option<String>,
    /// Up to three status items.
    pub status: Vec<Status>,
    pub fans: Speed,
    pub pump: Speed,
}

/// A status item: its accessible name, its words and its dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub name: &'static str,
    pub text: &'static str,
    pub dot: Dot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    Good,
    Warn,
    Bad,
    /// Hollow: unknown.
    Off,
}

/// A fan or pump row under the preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    Rpm(u16),
    /// The input is there, but its speed can't be read (decisions log D34).
    NoReading,
    /// No input chosen: a link to the Configuration page.
    ChooseInput,
    NotFound,
    NoSensors,
    /// The service is down.
    Unknown,
}

impl Speed {
    pub fn text(self) -> String {
        match self {
            Speed::Rpm(rpm) => format!("{} RPM", grouped(rpm)),
            Speed::NoReading => "No reading".to_owned(),
            Speed::ChooseInput => "Choose input".to_owned(),
            Speed::NotFound => "Input not found".to_owned(),
            Speed::NoSensors => "No fan sensors found".to_owned(),
            Speed::Unknown => "—".to_owned(),
        }
    }
}

/// `n` with English digit grouping: `1,180`.
pub fn grouped(n: u16) -> String {
    if n >= 1000 {
        format!("{},{:03}", n / 1000, n % 1000)
    } else {
        n.to_string()
    }
}

/// The Display page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayPage {
    pub unit: TemperatureUnit,
    /// Load, Temperature, Power, Music.
    pub groups: Vec<Group>,
}

/// One tile group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub kind: Kind,
    /// Every distinct reason of its tiles, once, in tile order.
    pub reasons: Vec<String>,
    pub tiles: Vec<Tile>,
}

/// One mode's tile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    pub mode: u16,
    /// The caption under the picture: `CPU + GPU`, `Emoji 1`.
    pub caption: String,
    /// The accessible name: `Load: CPU`.
    pub label: String,
    pub selected: bool,
    pub sensitive: bool,
    pub flag: Option<Flag>,
    /// `<metric>: <reason>` when unavailable, or `Reading lost: …` on the saved mode.
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    /// "Unavailable", muted.
    Unavailable,
    /// "Reading lost", in the warning colour: the saved mode.
    ReadingLost,
}

impl Flag {
    pub fn text(self) -> &'static str {
        match self {
            Flag::Unavailable => "Unavailable",
            Flag::ReadingLost => "Reading lost",
        }
    }
}

/// The Configuration page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigPage {
    /// 0–3, a quarter turn each.
    pub rotation: u8,
    /// Only with more than one GPU.
    pub gpu: Option<GpuRow>,
    pub cooler_fans: InputRow,
    pub pump: InputRow,
    /// Under the Cooler rows.
    pub inputs_hint: &'static str,
    pub peaks: bool,
    pub bar_fall: FallStop,
    pub peak_fall: FallStop,
}

pub const INPUTS_HINT: &str = "Pick the inputs your cooler's radiator fans and pump are plugged into. Their speeds show in the app only, never on the cooler.";
pub const NO_SENSORS_HINT: &str = "No fan sensors were found. The board's sensor driver may not be loaded; the journal says more.";
pub const MISSING_INPUT_HINT: &str =
    "The saved input isn't found. Pick another, or it comes back when the input does.";
pub const NO_GPU_RULE_HINT: &str =
    "No reading for this GPU's driver: the GPU options are unavailable.";

/// The GPU dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuRow {
    /// Automatic first, then the GPUs: the service's order, those without a reading rule last.
    pub entries: Vec<GpuEntry>,
    /// The chosen entry: 0 for Automatic.
    pub chosen: usize,
    /// "Using <GPU>." while Automatic is chosen and the GPU in use is known.
    pub in_use: Option<String>,
    /// When the GPU in use has no reading rule.
    pub hint: Option<&'static str>,
    /// The dropdown's accessible description.
    pub description: String,
}

/// One GPU dropdown entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuEntry {
    /// The address `ChooseGpu` takes: empty for Automatic.
    pub pci: String,
    pub label: String,
    /// The muted note in the open list.
    pub note: Option<&'static str>,
}

/// A fan input dropdown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputRow {
    /// None first, then every fan input; empty without fan sensors.
    pub entries: Vec<InputEntry>,
    /// The chosen entry, when it's listed.
    pub chosen: Option<usize>,
    /// What the dropdown's button reads.
    pub button: String,
    pub sensitive: bool,
    /// The saved input isn't found: a warning border, [`MISSING_INPUT_HINT`] and the
    /// accessible description "not found".
    pub missing: bool,
}

/// One fan input dropdown entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputEntry {
    /// The input `SetCoolerFanInput` or `SetPumpInput` takes: `None` for none.
    pub input: Option<Input>,
    pub label: String,
    /// Its live speed, "no reading", "not found", or empty for None.
    pub rpm: String,
}

/// What the window shows for `service`, `ui`, and the products of the system's PCI ID list
/// ([`crate::pci_ids::names`], by vendor and device ID).
pub fn view(service: &Service, ui: Ui, products: &HashMap<(u16, u16), String>) -> View {
    let state = match service {
        Service::Refused => return View::NotAvailable,
        Service::Down => return View::ServiceDown(down_column()),
        Service::Up(state) => state,
    };
    let mode = state
        .mode
        .and_then(|m| u16::try_from(m).ok())
        .and_then(Mode::new);
    let mode = mode.unwrap_or_else(|| Mode::new(1).expect("mode 1"));
    let reasons: HashMap<u16, String> = state
        .unavailable_reasons
        .iter()
        .flatten()
        .filter_map(|(m, r)| Some((u16::try_from(*m).ok()?, reason_text(r))))
        .collect();
    let cooler = shown(state.cooler_state, ui.sending_failed_long);
    let fan_inputs: &[FanInput] = state.fan_inputs.as_deref().unwrap_or(&[]);
    let lost = reasons.get(&mode.number());
    let mut status = vec![cooler.status(), SERVICE_RUNNING];
    if mode_info(mode).kind == Kind::Music {
        status.push(if state.audio_connected == Some(true) {
            Status {
                name: "Music sound status",
                text: "Sound connected",
                dot: Dot::Good,
            }
        } else {
            Status {
                name: "Music sound status",
                text: "No sound: spectrum is silent",
                dot: Dot::Warn,
            }
        });
    }
    let fans = Fan {
        chosen: state.cooler_fan_input.clone().flatten(),
        present: state.cooler_fan_input_present != Some(false),
        rpm: state.cooler_fan_rpm,
    };
    let pump = Fan {
        chosen: state.pump_input.clone().flatten(),
        present: state.pump_input_present != Some(false),
        rpm: state.pump_rpm,
    };
    let column = Column {
        caption: caption(mode),
        line: cooler.line(),
        reason: lost.map(|r| format!("{r}. The cooler shows dashes until it's back.")),
        status,
        fans: fans.speed(fan_inputs),
        pump: pump.speed(fan_inputs),
    };
    let display = DisplayPage {
        unit: state.temperature_unit.unwrap_or(TemperatureUnit::Celsius),
        groups: groups(mode, &reasons),
    };
    let config = ConfigPage {
        rotation: state.rotation.unwrap_or(0),
        gpu: gpu_row(state, products),
        cooler_fans: fans.row(fan_inputs),
        pump: pump.row(fan_inputs),
        inputs_hint: if fan_inputs.is_empty() {
            NO_SENSORS_HINT
        } else {
            INPUTS_HINT
        },
        peaks: state.music_peaks.unwrap_or(true),
        bar_fall: state.music_bar_fall.unwrap_or(FallStop::Moderate),
        peak_fall: state.music_peak_fall.unwrap_or(FallStop::Slow),
    };
    View::Normal(Box::new(Normal {
        page: ui.page,
        banner: cooler.banner(),
        column,
        display,
        config,
    }))
}

const SERVICE_RUNNING: Status = Status {
    name: "Service status",
    text: "Service running",
    dot: Dot::Good,
};

fn down_column() -> Column {
    Column {
        caption: "No picture".to_owned(),
        line: "The service draws the picture",
        reason: None,
        status: vec![
            Status {
                name: "Cooler status",
                text: "Cooler: unknown",
                dot: Dot::Off,
            },
            Status {
                name: "Service status",
                text: "Service not running",
                dot: Dot::Bad,
            },
        ],
        fans: Speed::Unknown,
        pump: Speed::Unknown,
    }
}

/// The cooler as the window shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cooler {
    Connected,
    NotConnected,
    NoAccess,
    SendingFails,
}

/// `write-failed` shows as connected for its first 2 s; `released`, a missing value and any
/// value the app couldn't read show as `not-found` (Banners; decisions log D33).
fn shown(state: Option<CoolerState>, sending_failed_long: bool) -> Cooler {
    match state {
        Some(CoolerState::Connected) => Cooler::Connected,
        Some(CoolerState::WriteFailed) if !sending_failed_long => Cooler::Connected,
        Some(CoolerState::WriteFailed) => Cooler::SendingFails,
        Some(CoolerState::NoAccess) => Cooler::NoAccess,
        Some(CoolerState::NotFound | CoolerState::Released) | None => Cooler::NotConnected,
    }
}

impl Cooler {
    fn status(self) -> Status {
        let (text, dot) = match self {
            Cooler::Connected => ("Cooler connected", Dot::Good),
            Cooler::NotConnected => ("Cooler not connected", Dot::Warn),
            Cooler::NoAccess => ("Cooler: no access", Dot::Bad),
            Cooler::SendingFails => ("Cooler: sending fails", Dot::Warn),
        };
        Status {
            name: "Cooler status",
            text,
            dot,
        }
    }

    fn line(self) -> &'static str {
        match self {
            Cooler::Connected => "Shown on the cooler now",
            Cooler::NotConnected => "Not on the cooler: it isn't connected",
            Cooler::NoAccess => "Not on the cooler: the service can't open it",
            Cooler::SendingFails => "Not on the cooler: sending it fails",
        }
    }

    fn banner(self) -> Option<Banner> {
        match self {
            Cooler::Connected => None,
            Cooler::NotConnected => Some(Banner::NotConnected),
            Cooler::NoAccess => Some(Banner::NoAccess),
            Cooler::SendingFails => Some(Banner::SendingFails),
        }
    }
}

/// A group's name, as the app shows it.
pub fn group_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Load => "Load",
        Kind::Temperature => "Temperature",
        Kind::Power => "Power",
        Kind::Music => "Music",
    }
}

/// A metric's name as the display shows it (Text).
pub fn metric_name(metric: Metric) -> &'static str {
    match metric {
        Metric::CpuUsage => "CPU usage",
        Metric::GpuUsage => "GPU usage",
        Metric::MemoryLoad => "RAM usage",
        Metric::CpuTemperature => "CPU temperature",
        Metric::GpuTemperature => "GPU temperature",
        Metric::CpuPower => "CPU power",
        Metric::GpuPower => "GPU power",
    }
}

/// The device a metric reads, in a Load, Temperature or Power caption.
fn device(metric: Metric) -> &'static str {
    match metric {
        Metric::CpuUsage | Metric::CpuTemperature | Metric::CpuPower => "CPU",
        Metric::GpuUsage | Metric::GpuTemperature | Metric::GpuPower => "GPU",
        Metric::MemoryLoad => "RAM",
    }
}

/// A mode's option: `CPU + GPU`, `Spectrum`, `Emoji 2`, `GPU usage`, `Dancer`.
pub fn option(mode: Mode) -> String {
    let info = mode_info(mode);
    match (info.kind, mode.number()) {
        (Kind::Music, 13) => "Spectrum".to_owned(),
        (Kind::Music, n @ 14..=17) => format!("Emoji {}", n - 13),
        (Kind::Music, 23) => "Dancer".to_owned(),
        (Kind::Music, _) => info.metrics.iter().map(|&m| metric_name(m)).collect(),
        _ => info
            .metrics
            .iter()
            .map(|&m| device(m))
            .collect::<Vec<_>>()
            .join(" + "),
    }
}

/// `<group> · <option>`.
pub fn caption(mode: Mode) -> String {
    format!("{} · {}", group_name(mode_info(mode).kind), option(mode))
}

/// The service's reason as the app shows it: the service names the metric by the spec's name
/// (`CPU Usage`, `Memory Load`, `Gpu usage`), which the app writes as the display shows it; the
/// reason after it stays as the service wrote it (decisions log D35).
pub fn reason_text(reason: &str) -> String {
    match reason.split_once(": ") {
        Some((name, rest)) => match Metric::from_spec_name(name) {
            Some(metric) => format!("{}: {rest}", metric_name(metric)),
            None => reason.to_owned(),
        },
        None => reason.to_owned(),
    }
}

fn groups(saved: Mode, reasons: &HashMap<u16, String>) -> Vec<Group> {
    [Kind::Load, Kind::Temperature, Kind::Power, Kind::Music]
        .into_iter()
        .map(|kind| {
            let tiles: Vec<Tile> = Mode::all()
                .filter(|m| mode_info(*m).kind == kind)
                .map(|m| tile(m, m == saved, reasons.get(&m.number())))
                .collect();
            let mut group_reasons: Vec<String> = Vec::new();
            for tile in &tiles {
                if let Some(reason) = reasons.get(&tile.mode)
                    && !group_reasons.contains(reason)
                {
                    group_reasons.push(reason.clone());
                }
            }
            Group {
                kind,
                reasons: group_reasons,
                tiles,
            }
        })
        .collect()
}

fn tile(mode: Mode, selected: bool, reason: Option<&String>) -> Tile {
    let caption = option(mode);
    Tile {
        mode: mode.number(),
        label: format!("{}: {caption}", group_name(mode_info(mode).kind)),
        caption,
        selected,
        sensitive: selected || reason.is_none(),
        flag: reason.map(|_| {
            if selected {
                Flag::ReadingLost
            } else {
                Flag::Unavailable
            }
        }),
        description: reason.map(|r| {
            if selected {
                format!("Reading lost: {r}")
            } else {
                r.clone()
            }
        }),
    }
}

/// The kind as a GPU label shows it.
fn kind_word(kind: GpuKind) -> &'static str {
    match kind {
        GpuKind::Dedicated => "Dedicated",
        GpuKind::Integrated => "Integrated",
        GpuKind::Unknown | GpuKind::QueryFailed => "Unknown type",
    }
}

/// Every GPU's label: `<kind> · <product>`, or `<kind> · <driver> · <address>` without a PCI
/// ID list entry; GPUs whose labels would be the same get their address too (decisions log
/// D36).
fn gpu_labels(gpus: &[Gpu], products: &HashMap<(u16, u16), String>) -> Vec<String> {
    let first: Vec<String> = gpus
        .iter()
        .map(|g| match products.get(&(g.vendor, g.device)) {
            Some(product) => format!(
                "{} · {}",
                kind_word(g.kind),
                crate::pci_ids::product(product)
            ),
            None => format!("{} · {} · {}", kind_word(g.kind), g.driver, g.pci),
        })
        .collect();
    first
        .iter()
        .zip(gpus)
        .map(|(label, g)| {
            if first.iter().filter(|l| *l == label).count() > 1 && !label.ends_with(&g.pci) {
                format!("{label} · {}", g.pci)
            } else {
                label.clone()
            }
        })
        .collect()
}

fn gpu_row(state: &State, products: &HashMap<(u16, u16), String>) -> Option<GpuRow> {
    let gpus: &[Gpu] = state.gpus.as_deref().unwrap_or(&[]);
    if gpus.len() < 2 {
        return None;
    }
    let labels = gpu_labels(gpus, products);
    let note = |g: &Gpu| {
        if !g.has_rule {
            Some("no readings")
        } else if g.kind == GpuKind::QueryFailed {
            Some("driver couldn't be asked")
        } else {
            None
        }
    };
    let mut order: Vec<usize> = (0..gpus.len()).filter(|&i| gpus[i].has_rule).collect();
    order.extend((0..gpus.len()).filter(|&i| !gpus[i].has_rule));
    let mut entries = vec![GpuEntry {
        pci: String::new(),
        label: "Automatic".to_owned(),
        note: None,
    }];
    entries.extend(order.iter().map(|&i| GpuEntry {
        pci: gpus[i].pci.clone(),
        label: labels[i].clone(),
        note: note(&gpus[i]),
    }));
    let chosen_pci = state.chosen_gpu.as_deref().unwrap_or("");
    // The service lists every saved GPU (it clears the choice at start); one it doesn't reads
    // as Automatic.
    let chosen = if chosen_pci.is_empty() {
        0
    } else {
        entries
            .iter()
            .position(|e| e.pci == chosen_pci)
            .unwrap_or(0)
    };
    let in_use = state
        .gpu_in_use
        .as_deref()
        .and_then(|pci| gpus.iter().position(|g| g.pci == pci));
    let automatic = chosen == 0;
    let in_use_line = in_use
        .filter(|_| automatic)
        .map(|i| format!("Using {}.", labels[i]));
    let in_use_has_no_rule = in_use.is_some_and(|i| !gpus[i].has_rule);
    let chosen_failed = !automatic
        && gpus
            .iter()
            .find(|g| g.pci == entries[chosen].pci)
            .is_some_and(|g| g.kind == GpuKind::QueryFailed);
    let description = [
        in_use
            .filter(|_| automatic)
            .map(|i| format!("Using {}", labels[i])),
        in_use_has_no_rule.then(|| "no readings".to_owned()),
        chosen_failed.then(|| "driver couldn't be asked".to_owned()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("; ");
    Some(GpuRow {
        entries,
        chosen,
        in_use: in_use_line,
        hint: in_use_has_no_rule.then_some(NO_GPU_RULE_HINT),
        description,
    })
}

/// One of the cooler's inputs, the cooler fans' or the pump's.
struct Fan {
    chosen: Option<Input>,
    present: bool,
    rpm: Option<FanSpeed>,
}

/// `<chip> · <label or fanN>`.
fn input_label(chip: &str, label: &str, input: u32) -> String {
    if label.is_empty() {
        format!("{chip} · fan{input}")
    } else {
        format!("{chip} · {label}")
    }
}

fn rpm_text(speed: FanSpeed) -> String {
    match speed {
        FanSpeed::Rpm(rpm) => format!("{} RPM", grouped(rpm)),
        FanSpeed::Unavailable => "no reading".to_owned(),
    }
}

fn same(input: &Input, listed: &FanInput) -> bool {
    input.chip == listed.chip && input.device == listed.device && input.input == listed.input
}

impl Fan {
    fn speed(&self, inputs: &[FanInput]) -> Speed {
        if inputs.is_empty() {
            return Speed::NoSensors;
        }
        if self.chosen.is_none() {
            return Speed::ChooseInput;
        }
        if !self.present {
            return Speed::NotFound;
        }
        match self.rpm {
            Some(FanSpeed::Rpm(rpm)) => Speed::Rpm(rpm),
            Some(FanSpeed::Unavailable) | None => Speed::NoReading,
        }
    }

    fn row(&self, inputs: &[FanInput]) -> InputRow {
        if inputs.is_empty() {
            return InputRow {
                entries: Vec::new(),
                chosen: None,
                button: "No fan sensors found".to_owned(),
                sensitive: false,
                missing: false,
            };
        }
        let missing = self.chosen.is_some() && !self.present;
        let listed = self
            .chosen
            .as_ref()
            .and_then(|c| inputs.iter().position(|i| same(c, i)));
        let mut entries = vec![InputEntry {
            input: None,
            label: "None".to_owned(),
            rpm: String::new(),
        }];
        entries.extend(inputs.iter().enumerate().map(|(i, f)| InputEntry {
            input: Some(Input {
                chip: f.chip.clone(),
                device: f.device.clone(),
                input: f.input,
            }),
            label: input_label(&f.chip, &f.label, f.input),
            rpm: if missing && listed == Some(i) {
                "not found".to_owned()
            } else {
                rpm_text(f.rpm)
            },
        }));
        let (chosen, button) = match (&self.chosen, listed) {
            (None, _) => (Some(0), "None".to_owned()),
            (Some(_), Some(i)) => (Some(i + 1), entries[i + 1].label.clone()),
            (Some(c), None) => (None, input_label(&c.chip, "", c.input)),
        };
        InputRow {
            entries,
            chosen,
            button: if missing {
                format!("{button} — not found")
            } else {
                button
            },
            sensitive: true,
            missing,
        }
    }
}

#[cfg(test)]
mod tests;
