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
use crate::drawing::{BLANK, Frame, Kind, Pixel, SIZE, emoji, is_emoji, row, value_mode};
use crate::frames::{TITLES, emoji_loop, text};
use crate::movement::Movement;
use crate::rules::{Ratio, strip_of};
use crate::spectrum::{COLUMNS, STOPS, Spectrum};

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

/// S1–S13, S19, S20 and S22: values and movement.
fn movement_scenarios(mode: u16, unit: Option<bool>) -> Vec<Scenario> {
    use Level::{Base, Four, High, Max, OneDigit};
    let f = fahrenheit(unit);
    let mut out = Vec::new();
    let pair = shown(mode).len() == 2;
    for &v in Variant::of(mode) {
        let roles = v.roles(mode);
        let s = |id: &str, rest: &[&str], start: Level| {
            Script::new(id, mode, unit, &detail(v, rest), &roles, start)
        };
        let sv = |id: &str, rest: &[&str], value: &str| {
            Script::literal(id, mode, unit, &detail(v, rest), &roles, value)
        };
        let set = |metric: &'static str, value: &str| Step::Set(metric, value.to_owned());
        out.push(s("S1", &[], High).end(End::Total(43)));
        for (from, label) in [(Base, "from two digits"), (OneDigit, "from one digit")] {
            if f && label == "from one digit" {
                continue;
            }
            out.push(
                s("S2", &[label], from)
                    .set(3, &roles, High)
                    .end(End::Total(27)),
            );
        }
        out.push(
            s("S3", &[], High)
                .set(5, &roles, Base)
                .end(End::UntilStopped(3)),
        );
        out.push(
            s("S4", &[], High)
                .set(5, &roles, Base)
                .set(10, &roles, High)
                .end(End::Total(53)),
        );
        out.push(
            s("S5", &[], Base)
                .set(2, &roles, High)
                .set(6, &roles, Base)
                .set(10, &roles, High)
                .set(14, &roles, Base)
                .end(End::UntilStopped(3)),
        );
        for k in 0..21 {
            out.push(
                s("S6", &[&format!("k {k}")], High)
                    .set(tick_at(k, 21), &roles, Base)
                    .end(End::UntilStopped(2)),
            );
        }
        let near = |above: bool| -> Vec<Step> {
            roles
                .iter()
                .map(|&m| set(m, near_limit(m, f, above)))
                .collect()
        };
        out.push(
            Script::with(
                "S7",
                mode,
                unit,
                &detail(v, &[]),
                &roles
                    .iter()
                    .map(|&m| (m, near_limit(m, f, false).to_owned()))
                    .collect::<Vec<_>>(),
            )
            .at(2, near(true))
            .at(25, near(false))
            .end(End::UntilStopped(2)),
        );
        let all_at = |value: &str| -> Vec<Step> { roles.iter().map(|&m| set(m, value)).collect() };
        let all_drawing = |n: u32| -> Vec<Step> {
            roles
                .iter()
                .map(|&m| Step::Set(m, drawing(m, f, n)))
                .collect()
        };
        out.push(
            Script::with(
                "S8",
                mode,
                unit,
                &detail(v, &["999 and 1000"]),
                &roles
                    .iter()
                    .map(|&m| (m, drawing(m, f, 999)))
                    .collect::<Vec<_>>(),
            )
            .set(5, &roles, Four)
            .at(15, all_drawing(999))
            .set(25, &roles, Base)
            .end(End::UntilStopped(2)),
        );
        out.push(
            s("S8", &["past the cap"], Max)
                .at(5, all_at("10000"))
                .set(10, &roles, Max)
                .set(25, &roles, Base)
                .end(End::UntilStopped(2)),
        );
        // S9: the other value one-digit too, so two values re-align when the width changes. No
        // reading draws one digit in °F.
        if !f {
            let others: Vec<(&'static str, String)> = shown(mode)
                .iter()
                .map(|&m| {
                    let value = if roles.contains(&m) {
                        "9".to_owned()
                    } else {
                        level(m, f, OneDigit)
                    };
                    (m, value)
                })
                .collect();
            out.push(
                Script::with("S9", mode, unit, &detail(v, &[]), &others)
                    .at(2, all_at("10"))
                    .at(4, all_at("9"))
                    .end(End::Total(6)),
            );
        }
        out.push(
            sv("S11", &["while static"], "unavailable")
                .set(2, &roles, OneDigit)
                .at(4, all_at("unavailable"))
                .set(6, &roles, Base)
                .end(End::Total(8)),
        );
        out.push(
            s("S11", &["while scrolling"], High)
                .at(5, all_at("unavailable"))
                .set(10, &roles, High)
                .at(20, all_at("unavailable"))
                .end(End::UntilStopped(2)),
        );
        out.push(
            sv("S11", &["to 100 or more"], "unavailable")
                .set(3, &roles, High)
                .end(End::Total(25)),
        );
        out.push(
            s("S12", &[], Max)
                .at(26, all_at("123456"))
                .at(30, all_at("1000000000"))
                .end(End::Total(52)),
        );
        let mut every_tenth = s("S13", &["three digits"], High);
        for i in 1..=21 {
            let steps: Vec<Step> = roles
                .iter()
                .map(|&m| Step::Set(m, offset(m, f, High, 11, i64::from(i))))
                .collect();
            every_tenth = every_tenth.at(10 * i, steps);
        }
        out.push(every_tenth.end(End::Total(212)));
        for phase in 0..5 {
            let mut every_tenth = s("S13", &["four digits", &format!("phase {phase}")], Four);
            for i in 1..=5 {
                let steps: Vec<Step> = roles
                    .iter()
                    .map(|&m| Step::Set(m, offset(m, f, Four, 111, i64::from(i))))
                    .collect();
                every_tenth = every_tenth.at(10 * i + phase, steps);
            }
            out.push(every_tenth.end(End::Total(52 + phase)));
        }
        out.push(
            sv("S19", &["while static"], "-1")
                .at(2, all_at("-0.5"))
                .set(4, &roles, Base)
                .end(End::Total(6)),
        );
        out.push(
            s("S19", &["while scrolling"], High)
                .at(5, all_at("-7"))
                .set(10, &roles, High)
                .at(20, all_at("-7"))
                .end(End::UntilStopped(2)),
        );
        let points = boundaries(roles[0], f);
        let mut s20 = sv("S20", &[], points[0]);
        for (tick, value) in (1..).zip(&points[1..]) {
            s20 = s20.at(tick, all_at(value));
        }
        let after = u32::try_from(points.len()).expect("a few boundaries");
        out.push(s20.set(after, &roles, Base).end(End::UntilStopped(2)));
        out.push(
            s("S22", &[], High)
                .at(5, [Step::Missed])
                .at(6, [Step::Missed])
                .at(10, [Step::Missed])
                .end(End::Total(25)),
        );
    }
    if pair {
        let names = shown(mode);
        let (top, bottom) = (&names[..1], &names[1..]);
        out.push(
            Script::new("S10", mode, unit, &[], top, High)
                .set(6, bottom, High)
                .set(12, top, Base)
                .set(18, top, High)
                .set(18, bottom, Base)
                .set(24, top, Base)
                .end(End::UntilStopped(2)),
        );
        out.push(
            Script::new("S10b", mode, unit, &[], top, Four)
                .at(5, [Step::Set(names[0], drawing(names[0], f, 999))])
                .set(10, top, Four)
                .at(15, [Step::Set(names[0], drawing(names[0], f, 99))])
                .set(15, bottom, Four)
                .set(25, names, Base)
                .end(End::UntilStopped(2)),
        );
    }
    out
}

/// S14–S18, S21 and S23–S26: settings, mode switches and the device.
fn settings_scenarios(mode: u16, unit: Option<bool>) -> Vec<Scenario> {
    use Level::{Base, High};
    let f = fahrenheit(unit);
    let mut out = Vec::new();
    out.extend(mode_switches(mode, unit));
    for &v in Variant::of(mode) {
        let roles = v.roles(mode);
        let s = |id: &str, rest: &[&str], start: Level| {
            Script::new(id, mode, unit, &detail(v, rest), &roles, start)
        };
        out.extend(unit_changes(mode, unit, v));
        for (start, label) in [(Base, "static"), (High, "scrolling")] {
            let mut turns = s("S16", &[label], start);
            for (tick, d) in [(2, 1), (4, 1), (6, 1), (8, 1), (10, 2), (13, 3), (16, 1)] {
                turns = turns.at(tick, [Step::Reset(Cause::Rotation(d))]);
            }
            out.push(turns.end(End::Total(20)));
        }
        out.push(s("S17", &["99 or less"], Base).end(End::Total(3)));
        out.push(s("S17", &["100 or more"], High).end(End::Total(23)));
        let rates_away = || {
            RATES
                .iter()
                .map(|&m| Step::Set(m, "unavailable".to_owned()))
                .collect::<Vec<_>>()
        };
        let rates_back = |start: Level| {
            RATES
                .iter()
                .map(|&m| Step::Set(m, start_value(m, f, &roles, start)))
                .collect::<Vec<_>>()
        };
        out.push(
            s("S18", &["the cooler away"], High)
                .at(5, [Step::CoolerAway])
                .at(10, [Step::CoolerBack])
                .end(End::Total(25)),
        );
        out.push(
            s("S18", &["wake-up, node still there"], High)
                .at(7, [Step::Wake])
                .at(7, rates_away())
                .at(17, rates_back(High))
                .end(End::Total(30)),
        );
        out.push(
            s("S18", &["wake-up, node gone"], High)
                .at(7, [Step::Wake, Step::CoolerAway])
                .at(7, rates_away())
                .at(12, [Step::CoolerBack])
                .at(17, rates_back(High))
                .end(End::Total(30)),
        );
        out.push(
            s("S21", &["rotation"], High)
                .at(6, [Step::Reset(Cause::Rotation(1))])
                .set(6, &roles, Base)
                .end(End::Total(10)),
        );
        let next = next_in_kind(mode);
        out.push(
            s("S21", &[&format!("into mode {next}")], High)
                .at(6, [Step::Reset(Cause::Mode(next))])
                .set(6, &roles, Base)
                .end(End::Total(10)),
        );
        for (start, label) in [(Base, "static"), (High, "scrolling")] {
            // A mode outside the table (owner, 2026-11-05): from Phase 9 every mode is drawn, so
            // none stays "not yet available".
            out.push(
                s("S23", &[label], start)
                    .at(3, [Step::Refused(24, "not in the mode table")])
                    .at(6, [Step::Refused(refused_unavailable(mode), "unavailable")])
                    .end(End::Total(10)),
            );
        }
        let already = || {
            ["mode", "unit", "rotation", "GPU"]
                .into_iter()
                .map(Step::AlreadySet)
                .collect::<Vec<_>>()
        };
        out.push(
            s("S24", &["static"], Base)
                .at(3, already())
                .end(End::Total(6)),
        );
        // Another GPU brings its own readings for every GPU metric.
        let another_gpu = |scrolling: bool| -> Vec<Step> {
            let mut steps = vec![Step::Reset(Cause::AnotherGpu)];
            for (metric, static_, high) in ANOTHER_GPU {
                let value = if scrolling && roles.contains(&metric) {
                    high
                } else {
                    static_
                };
                steps.push(Step::Set(metric, value.to_owned()));
            }
            steps
        };
        out.push(
            s("S25", &["static"], Base)
                .at(3, another_gpu(false))
                .end(End::Total(6)),
        );
        let fresh = |base: Level| -> Vec<Step> {
            roles
                .iter()
                .map(|&m| Step::Set(m, offset(m, f, base, 2, 1)))
                .collect()
        };
        out.push(
            s("S26", &["static"], Base)
                .at(3, [Step::Reset(Cause::Refresh)])
                .at(3, fresh(Base))
                .end(End::Total(6)),
        );
        out.push(
            s("S26", &["scrolling, then 99 or less"], High)
                .at(5, [Step::Reset(Cause::Refresh)])
                .set(5, &roles, Base)
                .end(End::Total(9)),
        );
        for k in 0..21 {
            let t = tick_at(k, 21);
            let at_k = format!("k {k}");
            out.push(
                s("S24", &["scrolling", &at_k], High)
                    .at(t, already())
                    .end(End::Total(t + 4)),
            );
            out.push(
                s("S25", &["scrolling", &at_k], High)
                    .at(t, another_gpu(true))
                    .end(End::Total(t + 4)),
            );
            out.push(
                s("S26", &["scrolling", &at_k], High)
                    .at(t, [Step::Reset(Cause::Refresh)])
                    .at(t, fresh(High))
                    .end(End::Total(t + 4)),
            );
        }
    }
    // S17's first second: a reading averaged over the interval shows dashes until its second
    // sample (display.md, Unavailable values).
    for rate in RATES.into_iter().filter(|m| shown(mode).contains(m)) {
        out.push(
            Script::with(
                "S17",
                mode,
                unit,
                &["the first second's dashes, then 100 or more".to_owned()],
                &[(rate, "unavailable".to_owned())],
            )
            .set(10, &[rate], Level::High)
            .end(End::Total(32)),
        );
    }
    out
}

/// S15: a unit change. In a mode that shows a temperature it resets, like a mode switch: static
/// and scrolling, `57°C` ↔ `135°F`, and at every k of a scrolling strip. In any other mode it's
/// only saved: static and scrolling, and in Power at every k too.
fn unit_changes(mode: u16, unit: Option<bool>, v: Variant) -> Vec<Scenario> {
    use Level::{Base, High};
    let roles = v.roles(mode);
    let s = |rest: &[&str], start: Level| {
        Script::new("S15", mode, unit, &detail(v, rest), &roles, start)
    };
    let mut out = Vec::new();
    let (there, back) = if shows_temperature(mode) {
        let f = fahrenheit(unit);
        (Step::Reset(Cause::Unit(!f)), Step::Reset(Cause::Unit(f)))
    } else {
        (Step::UnitSaved(true), Step::UnitSaved(false))
    };
    for (start, label) in [(Base, "static"), (High, "scrolling")] {
        out.push(
            s(&[label], start)
                .at(3, [there.clone()])
                .at(8, [back.clone()])
                .end(End::Total(12)),
        );
    }
    if shows_temperature(mode) {
        // 57 °C is static and 135 °F scrolls; the other value of mode 9 at 56 °C, 133 °F.
        let readings: Vec<(&'static str, String)> = roles
            .iter()
            .zip(["57", "56"])
            .map(|(&m, value)| (m, value.to_owned()))
            .collect();
        out.push(
            Script::with("S15", mode, unit, &detail(v, &["57 °C, 135 °F"]), &readings)
                .at(3, [there.clone()])
                .at(8, [back.clone()])
                .end(End::Total(30)),
        );
    }
    if kind(mode) != Kind::Usage {
        for k in 0..21 {
            let t = tick_at(k, 21);
            out.push(
                s(&["scrolling", &format!("k {k}")], High)
                    .at(t, [there.clone()])
                    .end(End::Total(t + 5)),
            );
        }
    }
    out
}

/// The reading `metric` starts at in a scenario whose `roles` start at `start`.
fn start_value(
    metric: &'static str,
    fahrenheit: bool,
    roles: &[&'static str],
    start: Level,
) -> String {
    if roles.contains(&metric) {
        level(metric, fahrenheit, start)
    } else {
        level(metric, fahrenheit, Level::Base)
    }
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

/// S14: a switch from `mode` into every other mode, at every k of a scrolling strip, with values
/// below 100, above it, and mixed. A switch into a mode that shows a temperature runs in both
/// units, from a mode without one too. Modes 1–12 switch into modes 1–12, whose files stay as
/// Phase 8 wrote them; a Music mode switches into every mode 1–23, and into a Music mode while
/// the sound plays, so the spectrum's levels carry on.
fn mode_switches(mode: u16, unit: Option<bool>) -> Vec<Scenario> {
    let last = if mode <= 12 { 12 } else { 23 };
    let mut out = Vec::new();
    for target in (1..=last).filter(|&t| t != mode) {
        let into_units: Vec<Option<bool>> = match unit {
            Some(_) => vec![unit],
            None => units(target).to_vec(),
        };
        let audio = if mode >= 13 {
            music_audio(target)
        } else {
            None
        };
        for u in into_units {
            out.extend(switches(mode, target, u, sets_for(target), audio));
        }
    }
    out
}

/// S14 into a Music mode from each mode 1–12, kept in the Music mode's file: its sound plays from
/// tick 0, and its levels start at 0 when it's entered (music.md point 8).
fn switches_into(target: u16) -> Vec<Scenario> {
    let mut out = Vec::new();
    for source in 1..=12 {
        for &unit in units(source) {
            let into_units: Vec<Option<bool>> = match unit {
                Some(_) => vec![unit],
                None => units(target).to_vec(),
            };
            for u in into_units {
                out.extend(switches(
                    source,
                    target,
                    u,
                    sets_for(target),
                    music_audio(target),
                ));
            }
        }
    }
    out
}

/// The readings a switch into `target` needs: all three sets where it shows values; where it
/// shows none, only what keeps the mode left static or scrolling.
fn sets_for(target: u16) -> &'static [SwitchSet] {
    if value_mode(target).is_some() {
        &SWITCH_SETS
    } else {
        &SWITCH_SETS[..2]
    }
}

/// The switches from `mode`, which has values, into `target` in unit `u`, one set after another:
/// at every k when the strip scrolls, at tick 3 when it's static, with `audio` from tick 0.
fn switches(
    mode: u16,
    target: u16,
    u: Option<bool>,
    sets: &[SwitchSet],
    audio: Option<&'static str>,
) -> Vec<Scenario> {
    let mut out = Vec::new();
    for &(label, high) in sets {
        let readings: Vec<(&'static str, String)> = METRICS
            .iter()
            .map(|&m| {
                let at = if high(m) { Level::High } else { Level::Base };
                (m, level(m, fahrenheit(u), at))
            })
            .collect();
        let into = format!("into mode {target}");
        let switch = |detail: &[String], t: u32, end: u32| {
            Script::with("S14", mode, u, detail, &readings)
                .at(0, audio.map(Step::Audio))
                .at(t, [Step::Reset(Cause::Mode(target))])
                .end(End::Total(end))
        };
        if shown(mode).iter().any(|m| high(m)) {
            for k in 0..21 {
                let t = tick_at(k, 21);
                let detail = [into.clone(), label.to_owned(), format!("k {k}")];
                out.push(switch(&detail, t, t + 5));
            }
        } else {
            out.push(switch(&[into, label.to_owned()], 3, 8));
        }
    }
    out
}

/// The signals the Music scenarios play: the burst train under the spectrum, and for the Dancer
/// a beat train that brings sways and big moves (`spectrum/signals.txt`).
const BURST: &str = "burst train, column 7, -20 dBFS, 100 ms on, 400 ms off";
const BEATS: &str = "beat train, bass rising 0.13 a tick from 0.38 to 0.90, then 2 silent";

/// The sound a scenario plays in a Music mode: none elsewhere.
fn music_audio(mode: u16) -> Option<&'static str> {
    match mode {
        13..=22 => Some(BURST),
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

/// Whether a mode draws the spectrum: 13–22.
fn has_spectrum(mode: u16) -> bool {
    (13..=22).contains(&mode)
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

/// An emoji loop as the picture of each 100 ms tick (music.md, Emoji animations).
fn emoji_ticks(mode: u16) -> Vec<usize> {
    emoji_loop(mode)
        .iter()
        .flat_map(|&(picture, ms)| std::iter::repeat_n(picture, (ms / 100) as usize))
        .collect()
}

/// The ticks at which a change comes in a Music mode without values: every tick of a burst's
/// period in 13, every loop position in 14–17, the first ten ticks of the beat train in 23.
fn positions(mode: u16) -> Vec<u32> {
    let cycle = match mode {
        13 => 5,
        14..=17 => u32::try_from(emoji_ticks(mode).len()).expect("a short loop"),
        _ => return (1..=10).collect(),
    };
    (0..cycle).map(|p| tick_at(p, cycle)).collect()
}

/// S14–S18, S25 and S26 for a Music mode without values (display.md, Which modes): switches into
/// every other mode, rotation changes, another GPU and Refresh at every position, a unit change
/// (only saved), start-up and the device, while its sound plays.
fn still_scenarios(mode: u16) -> Vec<Scenario> {
    let own = music_audio(mode).expect("a Music mode");
    let s = |id: &str, detail: &[String]| {
        Script::with(id, mode, None, detail, &[]).at(0, [Step::Audio(own)])
    };
    let at = |t: u32| format!("at tick {t}");
    let mut out = Vec::new();
    for target in (1..=23).filter(|&t| t != mode) {
        let audio = if target == 23 { BEATS } else { own };
        let sets: &[SwitchSet] = if value_mode(target).is_some() {
            &SWITCH_SETS
        } else {
            &SWITCH_SETS[..1]
        };
        for &u in units(target) {
            for &(label, high) in sets {
                let readings: Vec<(&'static str, String)> = METRICS
                    .iter()
                    .map(|&m| {
                        let level_ = if high(m) { Level::High } else { Level::Base };
                        (m, level(m, fahrenheit(u), level_))
                    })
                    .collect();
                for t in positions(mode) {
                    let detail = [format!("into mode {target}"), label.to_owned(), at(t)];
                    out.push(
                        Script::with("S14", mode, u, &detail, &readings)
                            .at(0, [Step::Audio(audio)])
                            .at(t, [Step::Reset(Cause::Mode(target))])
                            .end(End::Total(t + 5)),
                    );
                }
            }
        }
    }
    out.push(
        s("S15", &[])
            .at(3, [Step::UnitSaved(true)])
            .at(8, [Step::UnitSaved(false)])
            .end(End::Total(12)),
    );
    for (i, t) in positions(mode).into_iter().enumerate() {
        let turns = u8::try_from(i % 3 + 1).expect("1 to 3");
        out.push(
            s("S16", &[at(t)])
                .at(t, [Step::Reset(Cause::Rotation(turns))])
                .end(End::Total(t + 5)),
        );
    }
    let quiet = Script::with("S17", mode, None, &["in silence".to_owned()], &[]);
    out.push(quiet.end(End::Total(12)));
    out.push(s("S17", &["with sound".to_owned()]).end(End::Total(12)));
    let rates = |value: &str| -> Vec<Step> {
        RATES
            .iter()
            .map(|&m| Step::Set(m, value.to_owned()))
            .collect()
    };
    let base = |m: &'static str| level(m, false, Level::Base);
    let rates_back: Vec<Step> = RATES.iter().map(|&m| Step::Set(m, base(m))).collect();
    out.push(
        s("S18", &["the cooler away".to_owned()])
            .at(5, [Step::CoolerAway])
            .at(10, [Step::CoolerBack])
            .end(End::Total(25)),
    );
    out.push(
        s("S18", &["wake-up, node still there".to_owned()])
            .at(7, [Step::Wake])
            .at(7, rates("unavailable"))
            .at(17, rates_back.clone())
            .end(End::Total(30)),
    );
    out.push(
        s("S18", &["wake-up, node gone".to_owned()])
            .at(7, [Step::Wake, Step::CoolerAway])
            .at(7, rates("unavailable"))
            .at(12, [Step::CoolerBack])
            .at(17, rates_back)
            .end(End::Total(30)),
    );
    for t in positions(mode) {
        let mut another_gpu = vec![Step::Reset(Cause::AnotherGpu)];
        for (metric, value, _) in ANOTHER_GPU {
            another_gpu.push(Step::Set(metric, value.to_owned()));
        }
        out.push(s("S25", &[at(t)]).at(t, another_gpu).end(End::Total(t + 5)));
    }
    for t in positions(mode) {
        out.push(
            s("S26", &[at(t)])
                .at(t, [Step::Reset(Cause::Refresh)])
                .end(End::Total(t + 5)),
        );
    }
    out
}

/// S15b: the Music settings, each stop and the peak dots, changed while the sound plays: nothing
/// resets, and the spectrum shows the change on the next frame (music.md point 7). Where nothing
/// draws a spectrum they're only saved, and the next spectrum draws them.
fn music_settings(mode: u16) -> Vec<Scenario> {
    use Step::{MusicBarFall as Bar, MusicPeakFall as Peak, MusicPeaks as Peaks};
    let mut out = Vec::new();
    if has_spectrum(mode) {
        let mut changes = Script::with("S15b", mode, units(mode)[0], &[], &[])
            .at(0, music_audio(mode).map(Step::Audio))
            .at(3, [Peaks(false)])
            .at(8, [Peaks(true)]);
        for (tick, bar, peak) in [
            (11, "slower", "slower"),
            (16, "faster", "faster"),
            (21, "slow", "moderate"),
            (26, "fast", "fast"),
            (31, "moderate", "slow"),
        ] {
            changes = changes.at(tick, [Bar(bar), Peak(peak)]);
        }
        out.push(changes.end(End::Total(40)));
    }
    if mode == 13 || mode == 23 {
        let from = if mode == 13 { 1 } else { 23 };
        out.push(
            Script::with(
                "S15b",
                from,
                None,
                &["only saved, then into mode 13".to_owned()],
                &[],
            )
            .at(0, music_audio(from).or(Some(BURST)).map(Step::Audio))
            .at(2, [Peaks(false), Bar("faster"), Peak("faster")])
            .at(4, [Step::Reset(Cause::Mode(13))])
            .end(End::Total(15)),
        );
    }
    out
}

/// A reading as the scenario writes it.
fn reading(value: &str) -> Option<Ratio> {
    if value == "unavailable" {
        None
    } else {
        Some(Ratio::decimal(value).unwrap_or_else(|| panic!("{value:?} isn't a decimal")))
    }
}

/// Runs a scenario: the movement procedure for a value, and in Music modes the spectrum, the
/// emoji loop or the Dancer, from the sound the script plays.
fn run(scenario: &Scenario) -> Vec<Tick> {
    let last = *scenario.script.keys().last().expect("tick 0");
    let mut values: HashMap<&'static str, Option<Ratio>> = HashMap::new();
    let mut mode = scenario.mode;
    let mut in_fahrenheit = fahrenheit(scenario.unit);
    let mut movement: Option<Movement> = None;
    let mut music = MusicSettings::default();
    let mut audio: Option<(&'static [[f64; COLUMNS]], u32)> = None;
    let mut spectrum: Option<Spectrum> = None;
    let mut emoji_at = 0;
    let mut dancer: Option<Dancer> = None;
    let mut ticks = Vec::new();
    let mut stopped = 0;
    for n in 0.. {
        let steps = scenario.script.get(&n).cloned().unwrap_or_default();
        let mut reset = false;
        let mut changed = false;
        let mut missed = false;
        for step in &steps {
            match step {
                Step::Set(metric, value) => {
                    values.insert(metric, reading(value));
                    changed = true;
                }
                Step::Reset(Cause::Mode(m)) => {
                    mode = *m;
                    reset = true;
                }
                Step::Reset(Cause::Unit(f)) => {
                    in_fahrenheit = *f;
                    reset = true;
                }
                Step::Reset(_) => reset = true,
                // Saved for the next mode that shows a temperature; this one draws none.
                Step::UnitSaved(f) => in_fahrenheit = *f,
                Step::Missed => missed = true,
                Step::Audio(signal) => audio = Some((signal_levels(signal), n)),
                Step::MusicPeaks(on) => music.peaks = *on,
                Step::MusicBarFall(name) => music.bar = name,
                Step::MusicPeakFall(name) => music.peak = name,
                _ => {}
            }
        }
        assert!(
            !(missed && audio.is_some()),
            "{}: a missed tick comes only in silence",
            scenario.heading
        );
        // The tick's window: the signal's tick since it started, or silence.
        let levels = audio
            .and_then(|(levels, from)| levels.get((n - from) as usize))
            .copied()
            .unwrap_or([0.0; COLUMNS]);
        let mut placed = None;
        let mut moving = false;
        let mut strip_frame = None;
        if value_mode(mode).is_some() {
            let readings: Vec<Option<Ratio>> = shown(mode)
                .iter()
                .map(|m| {
                    *values
                        .get(m)
                        .expect("every metric has a reading from tick 0")
                })
                .collect();
            let strip = strip_of(mode, &readings, in_fahrenheit);
            let m = match &mut movement {
                Some(m) if !reset => {
                    if changed {
                        m.change(strip);
                    }
                    m
                }
                _ => movement.insert(Movement::reset(strip)),
            };
            placed = Some(Placed {
                x: m.x(),
                home: m.strip().home(),
                cycle: m.strip().cycle(),
            });
            strip_frame = (!missed).then(|| m.tick());
            moving = m.moving();
        } else {
            movement = None;
        }
        // The spectrum, in 13–22: entering it from any other mode, 23 included, starts its levels
        // at 0, and a switch between them keeps them (music.md point 8). A missed tick, which only
        // comes in silence, leaves it as it is.
        if has_spectrum(mode) {
            let s = spectrum.get_or_insert_with(Spectrum::enter);
            if !missed {
                s.update(&levels, stop(music.bar).0, stop(music.peak).1);
            }
        } else {
            spectrum = None;
        }
        // A reset restarts the emoji loop and the Dancer (music.md).
        if reset {
            emoji_at = 0;
        }
        let pose = if mode == 23 {
            if reset || dancer.is_none() {
                dancer = Some(Dancer::reset());
            }
            let d = dancer.as_mut().expect("the Dancer");
            (!missed).then(|| d.tick(&levels))
        } else {
            dancer = None;
            None
        };
        let frame = (!missed).then(|| {
            let mut g = BLANK;
            match mode {
                13 => {}
                14..=17 => {
                    let picture = emoji_ticks(mode)[emoji_at];
                    for (r, line) in emoji(mode)[picture].iter().enumerate() {
                        g[r] = row(line);
                    }
                }
                18..=22 => g[..8].copy_from_slice(&strip_frame.expect("a value's frame")[..8]),
                23 => return poses()[usize::from(pose.expect("a pose"))],
                _ => return strip_frame.expect("a value's frame"),
            }
            let h = if mode == 13 { 12 } else { 6 };
            let s = spectrum.as_ref().expect("the spectrum");
            draw_spectrum(&mut g, s, h, music.peaks);
            g
        });
        if is_emoji(mode) && !missed {
            emoji_at = (emoji_at + 1) % emoji_ticks(mode).len();
        }
        ticks.push(Tick {
            n,
            frame,
            reset,
            placed,
            moving,
            steps,
        });
        match scenario.end {
            End::Total(total) if n + 1 >= total => break,
            End::Total(_) => {}
            End::UntilStopped(more) => {
                if n >= last && !moving {
                    stopped += 1;
                    if stopped > more {
                        break;
                    }
                }
                assert!(n < last + 200, "{}: never stops", scenario.heading);
            }
        }
    }
    ticks
}

/// The scenario files, one per mode: each one's path under `docs/spec/frames/` and its text.
pub fn files() -> Vec<(String, String)> {
    (1..=23)
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
        TITLES[usize::from(mode) - 1]
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
mod tests {
    use super::*;

    /// Every scenario of the 23 files.
    fn all() -> Vec<Scenario> {
        (1..=23).flat_map(scenarios).collect()
    }

    /// Every scenario's heading, mode and run, file by file, in file order.
    fn runs() -> Vec<(String, u16, Vec<Tick>)> {
        all()
            .iter()
            .map(|s| (s.heading.clone(), s.mode, run(s)))
            .collect()
    }

    /// The first columns, from 1 to 14, of the copies of a strip drawn at `x` with cycle `cycle`.
    fn on_screen(x: i64, cycle: i64) -> impl Iterator<Item = i64> {
        (1..=14).filter(move |c| (c - x).rem_euclid(cycle) == 0)
    }

    // Phase 7 plan, Task 1: nothing ever moves by more than one column per frame except at a
    // reset, or when a stopped picture is re-centred at its new home (S9; owner, 2026-10-29).
    // Every copy whose first column is on the screen had it there, or one column to its right,
    // in the frame before.
    #[test]
    fn nothing_moves_more_than_one_column_per_frame_except_at_a_reset() {
        for (heading, _, ticks) in runs() {
            let mut previous: Option<Placed> = None;
            for tick in ticks.iter().filter(|t| t.frame.is_some()) {
                let Some(placed) = tick.placed else {
                    previous = None;
                    continue;
                };
                let re_centred = !tick.moving && placed.x == placed.home;
                if let Some(before) = previous
                    && !tick.reset
                    && !re_centred
                {
                    let was = |c: i64| (c - before.x).rem_euclid(before.cycle) == 0;
                    for c in on_screen(placed.x, placed.cycle) {
                        assert!(
                            was(c) || was(c + 1),
                            "{heading}, tick {}: a copy at column {c}",
                            tick.n
                        );
                    }
                }
                previous = Some(placed);
            }
        }
    }

    // S1, S12: a strip that must scroll moves one column per frame from home; a stopped strip
    // is at home.
    #[test]
    fn a_stopped_strip_is_always_at_home() {
        for (heading, _, ticks) in runs() {
            for tick in &ticks {
                if let Some(placed) = tick.placed
                    && !tick.moving
                {
                    assert_eq!(placed.x, placed.home, "{heading}, tick {}", tick.n);
                }
            }
        }
    }

    // Phase 8 plan, Task 1: tick 0 names all seven metrics, so a switch into another kind draws
    // named readings.
    #[test]
    fn every_scenario_names_a_mode_starts_with_a_reset_and_names_every_metric() {
        for (heading, mode, ticks) in runs() {
            assert!((1..=23).contains(&mode), "{heading}");
            assert!(ticks[0].reset, "{heading}");
            assert_eq!(ticks[0].n, 0, "{heading}");
            assert!(
                ticks.windows(2).all(|w| w[1].n == w[0].n + 1),
                "{heading}: ticks in order"
            );
            for metric in METRICS {
                assert!(
                    ticks[0]
                        .steps
                        .iter()
                        .any(|s| matches!(s, Step::Set(m, _) if *m == metric)),
                    "{heading}: tick 0 doesn't name {metric}"
                );
            }
        }
    }

    // AC4: all 132 ordered pairs among modes 1–12, and from Phase 9 all 506 among modes 1–23, a
    // pair with a mode that shows a temperature in both units.
    #[test]
    fn every_ordered_pair_of_modes_has_a_switch_in_each_unit_it_needs() {
        let mut switches: Vec<(u16, u16, Option<bool>)> = Vec::new();
        for scenario in all().iter().filter(|s| s.heading.starts_with("## S14 ")) {
            for step in scenario.script.values().flatten() {
                if let Step::Reset(Cause::Mode(target)) = step {
                    switches.push((scenario.mode, *target, scenario.unit));
                }
            }
        }
        for from in 1..=23 {
            for into in (1..=23).filter(|&t| t != from) {
                let units: &[Option<bool>] = if shows_temperature(from) || shows_temperature(into) {
                    &[Some(false), Some(true)]
                } else {
                    &[None]
                };
                for unit in units {
                    assert!(
                        switches.contains(&(from, into, *unit)),
                        "no switch from mode {from} into {into} in {unit:?}"
                    );
                }
            }
        }
    }

    // display.md, Mode switches: a unit change resets only a mode that shows a temperature, and
    // is only saved in any other.
    #[test]
    fn a_unit_change_resets_only_where_a_temperature_is_shown() {
        for scenario in all() {
            let mut mode = scenario.mode;
            for (tick, steps) in &scenario.script {
                for step in steps {
                    match step {
                        Step::Reset(Cause::Mode(m)) => mode = *m,
                        Step::Reset(Cause::Unit(_)) => assert!(
                            shows_temperature(mode),
                            "{}, tick {tick}: a reset in mode {mode}",
                            scenario.heading
                        ),
                        Step::UnitSaved(_) => assert!(
                            !shows_temperature(mode),
                            "{}, tick {tick}: only saved in mode {mode}",
                            scenario.heading
                        ),
                        Step::Unit(_) => assert_eq!(*tick, 0, "{}", scenario.heading),
                        _ => {}
                    }
                }
            }
        }
    }

    // S23: the mode refused as unavailable shows a metric the scenario's mode doesn't, so the
    // service can make it unavailable while the scenario's mode still draws its own.
    #[test]
    fn s23_refuses_a_mode_whose_metric_the_scenario_s_mode_doesn_t_show() {
        for mode in 1..=12 {
            let refused = refused_unavailable(mode);
            assert_ne!(refused, mode);
            assert!(
                shown(refused).iter().any(|m| !shown(mode).contains(m)),
                "mode {mode} refuses mode {refused}"
            );
        }
        // Load keeps Phase 7's choices; modes 9 and 12 reach outside their kind.
        let chosen: Vec<u16> = (1..=12).map(refused_unavailable).collect();
        assert_eq!(chosen, [2, 3, 4, 5, 6, 1, 8, 9, 1, 11, 12, 1]);
    }

    // Phase 8 plan, Task 1: a unit change in a Load or Power mode leaves every frame as it was.
    #[test]
    fn a_unit_change_only_saved_changes_no_frame() {
        let mut checked = 0;
        for scenario in all() {
            let saved = |s: &Step| matches!(s, Step::UnitSaved(_));
            if !scenario.script.values().flatten().any(saved) {
                continue;
            }
            let mut without = scenario.clone();
            for steps in without.script.values_mut() {
                steps.retain(|s| !saved(s));
            }
            let frames = |s: &Scenario| run(s).into_iter().map(|t| t.frame).collect::<Vec<_>>();
            assert!(
                frames(&scenario) == frames(&without),
                "{}: a frame differs",
                scenario.heading
            );
            checked += 1;
        }
        assert!(checked > 0);
    }

    // S7 and S20 in °F: the °C readings chosen draw the °F numbers they're chosen for.
    #[test]
    fn the_fahrenheit_readings_draw_the_numbers_they_stand_for() {
        let drawn = |c: &str| {
            crate::rules::number(Ratio::decimal(c), Kind::Temperature, true).expect("a number")
        };
        assert_eq!(
            boundaries("CPU Temperature", true)
                .iter()
                .map(|c| drawn(c))
                .collect::<Vec<_>>(),
            [32, 99, 100, 999, 1000, 9997, 9999, 9999, 9999]
        );
        for metric in ["CPU Temperature", "Gpu temperature"] {
            assert_eq!(drawn(near_limit(metric, true, false)), 99);
            assert_eq!(drawn(near_limit(metric, true, true)), 100);
            assert_eq!(drawn(&drawing(metric, true, 999)), 999);
            assert_eq!(drawn(&drawing(metric, true, 99)), 99);
        }
        assert_eq!(drawn(&level("CPU Temperature", true, Level::Max)), 9999);
        assert_eq!(drawn(&level("Gpu temperature", true, Level::Max)), 9997);
        assert_eq!(drawn("5537.5"), 9999, "10 000, capped");
    }

    /// The mode each tick draws, from the scenario's switches.
    fn modes(scenario: &Scenario, ticks: &[Tick]) -> Vec<u16> {
        let mut mode = scenario.mode;
        ticks
            .iter()
            .map(|tick| {
                for step in &tick.steps {
                    if let Step::Reset(Cause::Mode(m)) = step {
                        mode = *m;
                    }
                }
                mode
            })
            .collect()
    }

    // music.md, Emoji animations: a reset shows the loop's first picture on the next frame, and
    // otherwise the loop goes on, a unit change or a Music setting change included.
    #[test]
    fn an_emoji_loop_restarts_at_every_reset_and_only_then() {
        let mut checked = 0;
        for scenario in all() {
            let ticks = run(&scenario);
            let mut at = 0;
            for (tick, mode) in ticks.iter().zip(modes(&scenario, &ticks)) {
                if !is_emoji(mode) {
                    continue;
                }
                if tick.reset {
                    at = 0;
                    checked += 1;
                }
                let loop_ = emoji_ticks(mode);
                let want = emoji(mode)[loop_[at]].map(row);
                let frame = tick.frame.expect("no missed tick in an emoji mode");
                assert!(frame[..7] == want, "{}, tick {}", scenario.heading, tick.n);
                assert!(frame[7].iter().all(|&p| p == Pixel::Off));
                at = (at + 1) % loop_.len();
            }
        }
        assert!(checked > 0);
    }

    // music.md point 7: a Music setting change resets nothing. Without it, every frame of a mode
    // without a spectrum and every top area stays the same; the spectrum changes somewhere.
    #[test]
    fn a_music_setting_changes_only_the_spectrum() {
        let setting = |s: &Step| {
            matches!(
                s,
                Step::MusicPeaks(_) | Step::MusicBarFall(_) | Step::MusicPeakFall(_)
            )
        };
        let mut checked = 0;
        for scenario in all() {
            if !scenario.script.values().flatten().any(setting) {
                continue;
            }
            let mut without = scenario.clone();
            for steps in without.script.values_mut() {
                steps.retain(|s| !setting(s));
            }
            let (with_, plain) = (run(&scenario), run(&without));
            let mut differs = false;
            for ((a, b), mode) in with_.iter().zip(&plain).zip(modes(&scenario, &with_)) {
                let (a, b) = (a.frame.expect("a frame"), b.frame.expect("a frame"));
                let top = match mode {
                    13 => 2,
                    14..=22 => 8,
                    _ => SIZE,
                };
                assert!(a[..top] == b[..top], "{}", scenario.heading);
                differs |= a != b;
            }
            assert!(
                differs,
                "{}: the settings changed nothing",
                scenario.heading
            );
            checked += 1;
        }
        assert!(checked >= 12);
    }

    // music.md, Spectrum: layout: each column a solid bar from the lit floor, and at most one dot
    // above it with a gap, within the area; nothing between the area and what's above it.
    #[test]
    fn every_music_frame_meets_the_spectrum_layout() {
        for scenario in all() {
            let ticks = run(&scenario);
            for (tick, mode) in ticks.iter().zip(modes(&scenario, &ticks)) {
                // A missed tick draws nothing (S22).
                let Some(frame) = tick.frame.filter(|_| has_spectrum(mode)) else {
                    continue;
                };
                let first = if mode == 13 { 2 } else { 8 };
                assert!(frame[first - 1].iter().all(|&p| p == Pixel::Off));
                for c in 0..SIZE {
                    let column: Vec<Pixel> = frame.iter().map(|row| row[c]).collect();
                    let lit: Vec<usize> =
                        (first..SIZE).filter(|&r| column[r] == Pixel::On).collect();
                    // The bar: lit rows from the floor up, without a gap; the dot, above a gap.
                    let bar = lit
                        .iter()
                        .rev()
                        .zip((0..SIZE).rev())
                        .take_while(|(r, want)| **r == *want)
                        .count();
                    assert!(bar >= 1, "{}, tick {}: no floor", scenario.heading, tick.n);
                    assert!(
                        lit.len() <= bar + 1,
                        "{}, tick {}, column {c}",
                        scenario.heading,
                        tick.n
                    );
                }
            }
        }
    }

    // music.md point 8: entering a spectrum mode from a mode without one starts the levels at 0,
    // and a switch between Music modes keeps them. Two ticks after column 7's first burst, its bar
    // still stands above the floor after 14 → 13, and is the floor after 1 → 13.
    #[test]
    fn the_spectrum_starts_at_0_when_entered_and_carries_on_between_music_modes() {
        let column_7 = |from: u16| {
            let scenario = Script::with("S14", from, None, &[], &[])
                .at(0, [Step::Audio(BURST)])
                .at(2, [Step::Reset(Cause::Mode(13))])
                .end(End::Total(3));
            let frame = run(&scenario)[2].frame.expect("a frame");
            (2..SIZE).filter(|&r| frame[r][6] == Pixel::On).count()
        };
        assert!(column_7(14) > 2);
        assert_eq!(column_7(1), 1);
    }
}
