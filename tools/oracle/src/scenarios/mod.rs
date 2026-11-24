//! `docs/spec/frames/scenarios/`: the test scenarios of `docs/spec/display.md` (S1–S26) for every
//! mode, in rotation 0, with the frame expected at every tick (Phase 7 plan, Task 1; Phase 8
//! plan, Task 1; Phase 9 plan, Task 1): one file per mode, `mode-001.txt` to `mode-023.txt`, each
//! under the commit guard's 2 MiB limit (owner, 2026-11-05), the modes with a temperature in °C
//! and in °F. The rotations 1–3 aren't written: their frames are these, turned. Each scenario is
//! a script of changes at chosen ticks, run through the movement procedure, and in Music modes
//! through the spectrum, the emoji loop or the Dancer, from the sound the script plays.

use std::collections::{BTreeMap, HashMap};

use std::sync::OnceLock;

use crate::dancer::{Dancer, poses};

use crate::drawing::{
    BLANK, Frame, Kind, MODES, Pixel, SIZE, emoji, is_emoji, is_emoji_alone, row, value_mode,
};

use crate::frames::{emoji_loop, text, title};

use crate::movement::Movement;

use crate::rules::{Ratio, strip_of};

use crate::spectrum::{COLUMNS, STOPS, Spectrum};

mod movement;
mod music;
mod run;
mod settings;
mod switches;
use movement::movement_scenarios;
use music::{emoji_ticks, music_settings, still_scenarios};
use run::run;
use settings::settings_scenarios;
use switches::{mode_switches, switches_into};

/// Every metric modes 1–12 show, in the order of the mode table. Tick 0 names them all, so a
/// switch into another kind of mode draws named readings.
const METRICS: [&str; 7] = [
    "CPU Usage",
    "Gpu usage",
    "Memory Load",
    "CPU Temperature",
    "Gpu temperature",
    "CPU Power",
    "Gpu power",
];

/// The readings averaged over the interval on every PC: dashes in their first second, and again
/// after a wake-up (`display.md`, Unavailable values).
const RATES: [&str; 2] = ["CPU Usage", "CPU Power"];

/// The readings another GPU brings (S25): a static one, and one of 100 or more for a GPU metric
/// the scenario's change applies to.
const ANOTHER_GPU: [(&str, &str, &str); 3] = [
    ("Gpu usage", "71", "116"),
    ("Gpu temperature", "36", "107"),
    ("Gpu power", "52", "260"),
];

fn is_temperature(metric: &str) -> bool {
    metric.ends_with("emperature")
}

/// Each metric's readings at the levels the scenarios use, temperatures in °C: every metric its
/// own value. No reading draws one digit in °F (0 °C is 32 °F); there the temperatures' largest
/// readings draw 9999 and 9997 (9998.6 and 9996.8), so the two values of mode 9 still differ.
fn level(metric: &str, fahrenheit: bool, level: Level) -> String {
    let row: [&str; 5] = match metric {
        "CPU Usage" => ["42", "3", "100", "1000", "9999"],
        "Gpu usage" => ["67", "6", "104", "1500", "9998"],
        "Memory Load" => ["85", "8", "109", "2048", "9997"],
        "CPU Temperature" if fahrenheit => ["29", "4", "101", "1001", "5537"],
        "CPU Temperature" => ["29", "4", "101", "1001", "9996"],
        "Gpu temperature" if fahrenheit => ["34", "2", "105", "1501", "5536"],
        "Gpu temperature" => ["34", "2", "105", "1501", "9995"],
        "CPU Power" => ["65", "7", "120", "1234", "9994"],
        "Gpu power" => ["48", "5", "250", "1999", "9993"],
        _ => panic!("{metric} isn't shown by modes 1–12"),
    };
    row[level as usize].to_owned()
}

#[derive(Clone, Copy)]
enum Level {
    /// Two digits.
    Base,
    /// One digit; two in °F.
    OneDigit,
    /// Three digits, 100 or more.
    High,
    /// Four digits.
    Four,
    /// The largest values, 4 digits each.
    Max,
}

/// A reading close to another, for the same metric: `base + step · i`.
fn offset(metric: &str, fahrenheit: bool, base: Level, step: i64, i: i64) -> String {
    let start: i64 = level(metric, fahrenheit, base)
        .parse()
        .expect("a whole number");
    (start + step * i).to_string()
}

/// Readings on either side of the number 99.5, distinct for each metric of a mode (S7). In °F
/// the °C readings whose °F is 99.48, 99.46, 99.5 and 99.52.
fn near_limit(metric: &str, fahrenheit: bool, above: bool) -> &'static str {
    match (metric, fahrenheit && is_temperature(metric), above) {
        ("CPU Temperature", true, false) => "37.49",
        ("Gpu temperature", true, false) => "37.48",
        ("CPU Temperature", true, true) => "37.5",
        ("Gpu temperature", true, true) => "37.51",
        ("CPU Usage" | "CPU Temperature" | "CPU Power", _, false) => "99.49",
        ("Gpu usage" | "Gpu temperature" | "Gpu power", _, false) => "99.48",
        ("Memory Load", _, false) => "99.47",
        ("CPU Usage" | "CPU Temperature" | "CPU Power", _, true) => "99.5",
        ("Gpu usage" | "Gpu temperature" | "Gpu power", _, true) => "99.51",
        ("Memory Load", _, true) => "99.52",
        _ => panic!("{metric} isn't shown by modes 1–12"),
    }
}

/// A reading that draws `n`, 999 or 99: in °F, the °C reading that does (998.6 and 98.6).
fn drawing(metric: &str, fahrenheit: bool, n: u32) -> String {
    if !(fahrenheit && is_temperature(metric)) {
        return n.to_string();
    }
    match n {
        999 => "537",
        99 => "37",
        _ => panic!("no °C reading chosen for {n} °F"),
    }
    .to_owned()
}

/// The readings either side of each rounding point where a number gains a digit (S20): in °F,
/// the °C readings that draw 32, 99 ↔ 100, 999 ↔ 1000, 9997 ↔ 9999 and 9999 ↔ 10 000, capped.
fn boundaries(metric: &str, fahrenheit: bool) -> &'static [&'static str] {
    if fahrenheit && is_temperature(metric) {
        &[
            "0", "37.49", "37.5", "537.49", "537.5", "5536", "5537", "5537.49", "5537.5",
        ]
    } else {
        &[
            "0", "0.49", "0.5", "9.49", "9.5", "99.49", "99.5", "999.49", "999.5", "9999.49",
            "9999.5",
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Reset(Cause),
    /// A metric's new reading: a decimal, or `unavailable`.
    Set(&'static str, String),
    /// The unit a scenario starts in: tick 0 only.
    Unit(bool),
    /// A unit change in a mode without a temperature: only saved.
    UnitSaved(bool),
    AlreadySet(&'static str),
    Refused(u16, &'static str),
    CoolerAway,
    CoolerBack,
    Wake,
    Missed,
    /// A signal of `spectrum/signals.txt` plays from this tick.
    Audio(&'static str),
    /// The Music settings (music.md point 7): never a reset.
    MusicPeaks(bool),
    MusicBarFall(&'static str),
    MusicPeakFall(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Cause {
    StartUp,
    Mode(u16),
    /// Quarter turns added to the rotation.
    Rotation(u8),
    /// A unit change in a mode that shows a temperature.
    Unit(bool),
    AnotherGpu,
    Refresh,
}

fn unit_name(fahrenheit: bool) -> &'static str {
    if fahrenheit { "fahrenheit" } else { "celsius" }
}

impl Step {
    fn text(&self) -> String {
        match self {
            Step::Reset(Cause::StartUp) => "reset start-up".into(),
            Step::Reset(Cause::Mode(m)) => format!("reset mode {m}"),
            Step::Reset(Cause::Rotation(d)) => format!("reset rotation +{d}"),
            Step::Reset(Cause::Unit(f)) => format!("reset unit {}", unit_name(*f)),
            Step::Reset(Cause::AnotherGpu) => "reset another GPU".into(),
            Step::Reset(Cause::Refresh) => "reset Refresh".into(),
            Step::Set(metric, value) => format!("{metric} = {value}"),
            Step::Unit(f) => format!("unit {}", unit_name(*f)),
            Step::UnitSaved(f) => format!("unit {}, only saved", unit_name(*f)),
            Step::AlreadySet(what) => format!("already set: {what}"),
            Step::Refused(mode, why) => format!("refused: mode {mode}, {why}"),
            Step::CoolerAway => "cooler away".into(),
            Step::CoolerBack => "cooler back".into(),
            Step::Wake => "wake".into(),
            Step::Missed => "missed".into(),
            Step::Audio(signal) => format!("audio {signal}"),
            Step::MusicPeaks(on) => format!("music peaks {}", if *on { "on" } else { "off" }),
            Step::MusicBarFall(stop) => format!("music bar fall {stop}"),
            Step::MusicPeakFall(stop) => format!("music peak fall {stop}"),
        }
    }
}

/// How long a scenario runs.
#[derive(Clone, Copy)]
enum End {
    /// Ticks 0 to n − 1.
    Total(u32),
    /// After the last change, until the strip stops, then n more ticks.
    UntilStopped(u32),
}

#[derive(Clone)]
struct Scenario {
    heading: String,
    mode: u16,
    /// The unit tick 0 names: `Some(true)` for °F. Named where a mode it shows has a
    /// temperature; `None` runs in °C.
    unit: Option<bool>,
    script: BTreeMap<u32, Vec<Step>>,
    end: End,
}

/// One tick of a scenario, as run.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the movement checks read where each frame was drawn"
    )
)]
pub struct Tick {
    pub n: u32,
    pub frame: Option<Frame>,
    pub reset: bool,
    /// Where a value's strip was drawn, in a mode with values.
    pub placed: Option<Placed>,
    /// Whether the strip was moving when the frame was drawn; never without values.
    pub moving: bool,
    /// Whether the sound played this tick has any column above level 0: what runs an emoji.
    pub sound: bool,
    steps: Vec<Step>,
}

/// Where a value's strip was drawn: the column of its first column, its home and its cycle.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the movement checks read where each frame was drawn"
    )
)]
#[derive(Clone, Copy)]
pub struct Placed {
    pub x: i64,
    pub home: i64,
    pub cycle: i64,
}

/// The value levels of one scenario: which metrics a change applies to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Single,
    Top,
    Bottom,
    Both,
}

impl Variant {
    fn of(mode: u16) -> &'static [Variant] {
        if shown(mode).len() == 1 {
            &[Variant::Single]
        } else {
            &[Variant::Top, Variant::Bottom, Variant::Both]
        }
    }

    fn label(self) -> Option<&'static str> {
        match self {
            Variant::Single => None,
            Variant::Top => Some("top"),
            Variant::Bottom => Some("bottom"),
            Variant::Both => Some("both"),
        }
    }

    /// The metrics a change applies to, in `mode`.
    fn roles(self, mode: u16) -> Vec<&'static str> {
        let names = shown(mode);
        match self {
            Variant::Single | Variant::Top => vec![names[0]],
            Variant::Bottom => vec![names[1]],
            Variant::Both => names.to_vec(),
        }
    }
}

fn shown(mode: u16) -> &'static [&'static str] {
    value_mode(mode).expect("a mode with values").1
}

fn kind(mode: u16) -> Kind {
    value_mode(mode).expect("a mode with values").0
}

fn shows_temperature(mode: u16) -> bool {
    value_mode(mode).is_some_and(|(_, shown)| shown.iter().any(|m| is_temperature(m)))
}

fn fahrenheit(unit: Option<bool>) -> bool {
    unit == Some(true)
}

/// The next mode of the same kind, the first after the last (S21, S23).
fn next_in_kind(mode: u16) -> u16 {
    let (first, count) = match mode {
        1..=6 => (1, 6),
        7..=9 => (7, 3),
        10..=12 => (10, 3),
        18..=22 => (18, 5),
        _ => panic!("mode {mode} has no values"),
    };
    first + (mode - first + 1) % count
}

/// The mode S23 asks for and the service refuses as unavailable: one that shows a metric `mode`
/// doesn't, so that metric can be unavailable while `mode` still draws its own. The next of the
/// same kind first, as for Load, then the others from mode 1 (mode 9 shows mode 7's and mode
/// 8's metrics, mode 12 those of 10 and 11).
fn refused_unavailable(mode: u16) -> u16 {
    let mut same_kind = Vec::new();
    let mut next = next_in_kind(mode);
    while next != mode {
        same_kind.push(next);
        next = next_in_kind(next);
    }
    same_kind
        .into_iter()
        .chain((1..=12).filter(|&m| kind(m) != kind(mode)))
        .find(|&m| shown(m).iter().any(|metric| !shown(mode).contains(metric)))
        .expect("a mode with a metric of its own")
}

/// Builds one scenario's script.
struct Script {
    mode: u16,
    unit: Option<bool>,
    heading: String,
    script: BTreeMap<u32, Vec<Step>>,
}

impl Script {
    /// A scenario starting at tick 0 with every metric at `base`, the `roles` at `start`.
    fn new(
        id: &str,
        mode: u16,
        unit: Option<bool>,
        detail: &[String],
        roles: &[&'static str],
        start: Level,
    ) -> Self {
        let readings: Vec<(&'static str, String)> = roles
            .iter()
            .map(|&metric| (metric, level(metric, fahrenheit(unit), start)))
            .collect();
        Script::with(id, mode, unit, detail, &readings)
    }

    /// A scenario starting at tick 0 with the `roles` at `value`, a decimal or `unavailable`.
    fn literal(
        id: &str,
        mode: u16,
        unit: Option<bool>,
        detail: &[String],
        roles: &[&'static str],
        value: &str,
    ) -> Self {
        let readings: Vec<(&'static str, String)> = roles
            .iter()
            .map(|&metric| (metric, value.to_owned()))
            .collect();
        Script::with(id, mode, unit, detail, &readings)
    }

    /// A scenario starting at tick 0 with the given readings, every other metric at its
    /// two-digit base, in `unit` when it's named.
    fn with(
        id: &str,
        mode: u16,
        unit: Option<bool>,
        detail: &[String],
        readings: &[(&'static str, String)],
    ) -> Self {
        let mut heading = format!("## {id} · mode {mode}");
        if let Some(f) = unit {
            heading += if f { " · °F" } else { " · °C" };
        }
        for part in detail {
            heading += " · ";
            heading += part;
        }
        let mut steps = vec![Step::Reset(Cause::StartUp)];
        steps.extend(unit.map(Step::Unit));
        for metric in METRICS {
            let value = readings.iter().find(|(m, _)| *m == metric).map_or_else(
                || level(metric, fahrenheit(unit), Level::Base),
                |(_, v)| v.clone(),
            );
            steps.push(Step::Set(metric, value));
        }
        let mut script = BTreeMap::new();
        script.insert(0, steps);
        Script {
            mode,
            unit,
            heading,
            script,
        }
    }

    fn at(mut self, tick: u32, steps: impl IntoIterator<Item = Step>) -> Self {
        self.script.entry(tick).or_default().extend(steps);
        self
    }

    /// The `roles` at `level` from `tick` on.
    fn set(self, tick: u32, roles: &[&'static str], level_: Level) -> Self {
        let f = fahrenheit(self.unit);
        let steps: Vec<Step> = roles
            .iter()
            .map(|&metric| Step::Set(metric, level(metric, f, level_)))
            .collect();
        self.at(tick, steps)
    }

    fn end(self, end: End) -> Scenario {
        Scenario {
            heading: self.heading,
            mode: self.mode,
            unit: self.unit,
            script: self.script,
            end,
        }
    }
}

/// The variant's label, if any, followed by `rest`.
fn detail(variant: Variant, rest: &[&str]) -> Vec<String> {
    variant
        .label()
        .into_iter()
        .chain(rest.iter().copied())
        .map(str::to_owned)
        .collect()
}

/// The tick at which the strip is `k` columns past home, for a strip reset at tick 0: `k`, or
/// one cycle on for `k` = 0, where it's back at home and hasn't moved on yet.
fn tick_at(k: u32, cycle: u32) -> u32 {
    if k == 0 { cycle } else { k }
}

/// The units a mode's scenarios run in: °C and °F where it shows a temperature.
fn units(mode: u16) -> &'static [Option<bool>] {
    if shows_temperature(mode) {
        &[Some(false), Some(true)]
    } else {
        &[None]
    }
}

/// Every scenario of one mode's file, °C before °F; a Music mode's file adds the Music settings
/// and the switches into it from modes 1–12.
fn scenarios(mode: u16) -> Vec<Scenario> {
    let mut all = Vec::new();
    if value_mode(mode).is_some() {
        for &unit in units(mode) {
            all.extend(movement_scenarios(mode, unit));
            all.extend(settings_scenarios(mode, unit));
        }
    } else {
        all.extend(still_scenarios(mode));
    }
    if mode >= 13 {
        all.extend(music_settings(mode));
        all.extend(switches_into(mode));
    }
    all
}

/// One set of S14's readings: its label, and whether a metric is 100 or more in it.
type SwitchSet = (&'static str, fn(&str) -> bool);

/// The readings of S14's switches: every metric below 100, every metric 100 or more, and only
/// the CPU's above it.
const SWITCH_SETS: [SwitchSet; 3] = [
    ("values below 100", |_| false),
    ("values above 100", |_| true),
    ("CPU above 100", |metric| metric.starts_with("CPU ")),
];

/// The signals the Music scenarios play: the burst train under the spectrum, and for the Dancer
/// a beat train that brings sways and big moves (`spectrum/signals.txt`).
const BURST: &str = "burst train, column 7, -20 dBFS, 100 ms on, 400 ms off";

const BEATS: &str = "beat train, bass rising 0.13 a tick from 0.38 to 0.90, then 2 silent";

/// The sound a scenario plays in a Music mode: none elsewhere. The three emoji alone, 24–26, are
/// Music modes that take no sound, but the PC's sound plays in their scenarios all the same: the
/// frames of one never change with it, and a switch into a spectrum finds it playing.
fn music_audio(mode: u16) -> Option<&'static str> {
    match mode {
        13..=16 | 18..=22 | 24..=26 => Some(BURST),
        23 => Some(BEATS),
        _ => None,
    }
}

/// Each tick's column levels, 0–1, of a signal the scenarios play.
fn signal_levels(name: &str) -> &'static [[f64; COLUMNS]] {
    static LEVELS: OnceLock<Vec<(String, Vec<[f64; COLUMNS]>)>> = OnceLock::new();
    let all = LEVELS.get_or_init(|| {
        crate::spectrum::signals()
            .into_iter()
            .filter(|s| [BURST, BEATS].contains(&s.name.as_str()))
            .map(|s| {
                let levels = s.tick_levels();
                let clamped = levels.iter().map(|l| l.map(|x| x.clamp(0.0, 1.0)));
                (s.name, clamped.collect())
            })
            .collect()
    });
    &all.iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("{name} isn't a signal the scenarios play"))
        .1
}

/// The Music settings, by their stops' names (music.md point 7).
#[derive(Clone, Copy)]
struct MusicSettings {
    peaks: bool,
    bar: &'static str,
    peak: &'static str,
}

impl Default for MusicSettings {
    /// Peak dots on, bars moderate, peaks slow.
    fn default() -> Self {
        MusicSettings {
            peaks: true,
            bar: "moderate",
            peak: "slow",
        }
    }
}

/// A stop's bar and peak values (music.md point 4).
fn stop(name: &str) -> (f64, f64) {
    STOPS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|&(_, b, g)| (b, g))
        .unwrap_or_else(|| panic!("{name} isn't a stop"))
}

/// Whether a mode draws the spectrum: 13–16 and 18–22.
fn has_spectrum(mode: u16) -> bool {
    matches!(mode, 13..=16 | 18..=22)
}

/// The spectrum *H* rows high above the floor: each column's bar, and its dot (music.md).
fn draw_spectrum(frame: &mut Frame, spectrum: &Spectrum, h: u8, peaks: bool) {
    for (c, (bar, dot)) in spectrum.draw(h, peaks).into_iter().enumerate() {
        for row in &mut frame[SIZE - usize::from(bar)..] {
            row[c] = Pixel::On;
        }
        if let Some(dot) = dot {
            frame[SIZE - usize::from(dot)][c] = Pixel::On;
        }
    }
}

/// The quiet ticks that put an emoji back at rest: one second (music.md, Emoji animations).
const REST_TICKS: u32 = 10;

/// A reading as the scenario writes it.
fn reading(value: &str) -> Option<Ratio> {
    if value == "unavailable" {
        None
    } else {
        Some(Ratio::decimal(value).unwrap_or_else(|| panic!("{value:?} isn't a decimal")))
    }
}

/// The scenario files, one per mode: each one's path under `docs/spec/frames/` and its text.
pub fn files() -> Vec<(String, String)> {
    MODES
        .into_iter()
        .map(|mode| (format!("scenarios/mode-{mode:03}.txt"), file(mode)))
        .collect()
}

/// One mode's file: the scenarios, then every frame they name, once.
fn file(mode: u16) -> String {
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut frames: Vec<String> = Vec::new();
    let units = if shows_temperature(mode) {
        ", °C and °F"
    } else {
        ""
    };
    let title = format!(
        "# Mode {mode} scenarios: {}{units}, rotation 0",
        title(mode)
    );
    let mut lines = vec![title, String::new()];
    for scenario in scenarios(mode) {
        lines.push(scenario.heading.clone());
        lines.push(String::new());
        for tick in run(&scenario) {
            let changes = if tick.steps.is_empty() {
                "-".to_owned()
            } else {
                tick.steps
                    .iter()
                    .map(Step::text)
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            let frame = match &tick.frame {
                None => "no frame".to_owned(),
                Some(frame) => {
                    let rows = text(frame).join("\n");
                    let next = ids.len() + 1;
                    let id = *ids.entry(rows.clone()).or_insert_with(|| {
                        frames.push(rows);
                        next
                    });
                    format!("f{id}")
                }
            };
            lines.push(format!("{}: {changes} → {frame}", tick.n));
        }
        lines.push(String::new());
    }
    lines.push("# Frames".to_owned());
    lines.push(String::new());
    for (i, rows) in frames.iter().enumerate() {
        lines.push(format!("### f{}", i + 1));
        lines.push(String::new());
        lines.push(rows.clone());
        lines.push(String::new());
    }
    let mut text = lines.join("\n").trim_end().to_owned();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests;
