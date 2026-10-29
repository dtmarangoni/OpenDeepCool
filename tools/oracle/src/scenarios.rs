//! `docs/spec/frames/scenarios/load.txt`: the test scenarios of `docs/spec/display.md` (S1–S26)
//! that apply to Load, for each of modes 1–6, in rotation 0, with the frame expected at every
//! tick (Phase 7 plan, Task 1). The rotations 1–3 aren't written: their frames are these, turned.
//! Each scenario is a script of changes at chosen ticks, run through the movement procedure.

use std::collections::{BTreeMap, HashMap};

use crate::drawing::{Frame, value_mode};
use crate::frames::text;
use crate::movement::Movement;
use crate::rules::{Ratio, strip_of};

/// The metrics Load shows, in the order the scenarios list them.
const LOAD: [&str; 3] = ["CPU Usage", "Gpu usage", "Memory Load"];

/// Each metric's readings at the levels the scenarios use: every metric its own value.
fn level(metric: &str, level: Level) -> String {
    let column = level as usize;
    let row: [&str; 5] = match metric {
        "CPU Usage" => ["42", "3", "100", "1000", "9999"],
        "Gpu usage" => ["67", "6", "104", "1500", "9998"],
        "Memory Load" => ["85", "8", "109", "2048", "9997"],
        _ => panic!("{metric} isn't shown by Load"),
    };
    row[column].to_owned()
}

#[derive(Clone, Copy)]
enum Level {
    /// Two digits.
    Base,
    OneDigit,
    /// Three digits, 100 or more.
    High,
    /// Four digits.
    Four,
    /// The largest values, 4 digits each.
    Max,
}

/// A reading close to another, for the same metric: `base + step · i`.
fn offset(metric: &str, base: Level, step: i64, i: i64) -> String {
    let start: i64 = level(metric, base).parse().expect("a whole number");
    (start + step * i).to_string()
}

/// Readings on either side of 99.5, distinct for each metric (S7).
fn near_limit(metric: &str, above: bool) -> &'static str {
    match (metric, above) {
        ("CPU Usage", false) => "99.49",
        ("Gpu usage", false) => "99.48",
        ("Memory Load", false) => "99.47",
        ("CPU Usage", true) => "99.5",
        ("Gpu usage", true) => "99.51",
        ("Memory Load", true) => "99.52",
        _ => panic!("{metric} isn't shown by Load"),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Reset(Cause),
    /// A metric's new reading: a decimal, or `unavailable`.
    Set(&'static str, String),
    /// A unit change in a mode without a temperature: only saved.
    UnitSaved(bool),
    AlreadySet(&'static str),
    Refused(u16, &'static str),
    CoolerAway,
    CoolerBack,
    Wake,
    Missed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Cause {
    StartUp,
    Mode(u16),
    /// Quarter turns added to the rotation.
    Rotation(u8),
    AnotherGpu,
    Refresh,
}

impl Step {
    fn text(&self) -> String {
        match self {
            Step::Reset(Cause::StartUp) => "reset start-up".into(),
            Step::Reset(Cause::Mode(m)) => format!("reset mode {m}"),
            Step::Reset(Cause::Rotation(d)) => format!("reset rotation +{d}"),
            Step::Reset(Cause::AnotherGpu) => "reset another GPU".into(),
            Step::Reset(Cause::Refresh) => "reset Refresh".into(),
            Step::Set(metric, value) => format!("{metric} = {value}"),
            Step::UnitSaved(true) => "unit fahrenheit, only saved".into(),
            Step::UnitSaved(false) => "unit celsius, only saved".into(),
            Step::AlreadySet(what) => format!("already set: {what}"),
            Step::Refused(mode, why) => format!("refused: mode {mode}, {why}"),
            Step::CoolerAway => "cooler away".into(),
            Step::CoolerBack => "cooler back".into(),
            Step::Wake => "wake".into(),
            Step::Missed => "missed".into(),
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

struct Scenario {
    heading: String,
    mode: u16,
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
    /// Where the frame was drawn: the column of the strip's first column.
    pub x: i64,
    /// Whether the strip was moving when the frame was drawn.
    pub moving: bool,
    pub home: i64,
    pub cycle: i64,
    steps: Vec<Step>,
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
    value_mode(mode).expect("a Load mode").1
}

/// Builds one scenario's script.
struct Script {
    mode: u16,
    heading: String,
    script: BTreeMap<u32, Vec<Step>>,
}

impl Script {
    /// A scenario starting at tick 0 with every Load metric at `base`, the `roles` at `start`.
    fn new(id: &str, mode: u16, detail: &[String], roles: &[&'static str], start: Level) -> Self {
        let readings: Vec<(&'static str, String)> = roles
            .iter()
            .map(|&metric| (metric, level(metric, start)))
            .collect();
        Script::with(id, mode, detail, &readings)
    }

    /// A scenario starting at tick 0 with the `roles` at `value`, a decimal or `unavailable`.
    fn literal(
        id: &str,
        mode: u16,
        detail: &[String],
        roles: &[&'static str],
        value: &str,
    ) -> Self {
        let readings: Vec<(&'static str, String)> = roles
            .iter()
            .map(|&metric| (metric, value.to_owned()))
            .collect();
        Script::with(id, mode, detail, &readings)
    }

    /// A scenario starting at tick 0 with the given readings, every other Load metric at its
    /// two-digit base.
    fn with(id: &str, mode: u16, detail: &[String], readings: &[(&'static str, String)]) -> Self {
        let mut heading = format!("## {id} · mode {mode}");
        for part in detail {
            heading += " · ";
            heading += part;
        }
        let mut steps = vec![Step::Reset(Cause::StartUp)];
        for metric in LOAD {
            let value = readings
                .iter()
                .find(|(m, _)| *m == metric)
                .map_or_else(|| level(metric, Level::Base), |(_, v)| v.clone());
            steps.push(Step::Set(metric, value));
        }
        let mut script = BTreeMap::new();
        script.insert(0, steps);
        Script {
            mode,
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
        let steps: Vec<Step> = roles
            .iter()
            .map(|&metric| Step::Set(metric, level(metric, level_)))
            .collect();
        self.at(tick, steps)
    }

    fn end(self, end: End) -> Scenario {
        Scenario {
            heading: self.heading,
            mode: self.mode,
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

/// Every scenario, mode by mode.
fn scenarios() -> Vec<Scenario> {
    let mut all = Vec::new();
    for mode in 1..=6 {
        all.extend(movement_scenarios(mode));
        all.extend(settings_scenarios(mode));
    }
    all
}

/// S1–S13, S19, S20 and S22: values and movement.
fn movement_scenarios(mode: u16) -> Vec<Scenario> {
    use Level::{Base, Four, High, Max, OneDigit};
    let mut out = Vec::new();
    let pair = shown(mode).len() == 2;
    for &v in Variant::of(mode) {
        let roles = v.roles(mode);
        let s = |id: &str, rest: &[&str], start: Level| {
            Script::new(id, mode, &detail(v, rest), &roles, start)
        };
        let sv = |id: &str, rest: &[&str], value: &str| {
            Script::literal(id, mode, &detail(v, rest), &roles, value)
        };
        let set = |metric: &'static str, value: &str| Step::Set(metric, value.to_owned());
        out.push(s("S1", &[], High).end(End::Total(43)));
        for (from, label) in [(Base, "from two digits"), (OneDigit, "from one digit")] {
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
                .map(|&m| set(m, near_limit(m, above)))
                .collect()
        };
        out.push(
            Script::with(
                "S7",
                mode,
                &detail(v, &[]),
                &roles
                    .iter()
                    .map(|&m| (m, near_limit(m, false).to_owned()))
                    .collect::<Vec<_>>(),
            )
            .at(2, near(true))
            .at(25, near(false))
            .end(End::UntilStopped(2)),
        );
        let all_at = |value: &str| -> Vec<Step> { roles.iter().map(|&m| set(m, value)).collect() };
        out.push(
            Script::with(
                "S8",
                mode,
                &detail(v, &["999 and 1000"]),
                &roles
                    .iter()
                    .map(|&m| (m, "999".to_owned()))
                    .collect::<Vec<_>>(),
            )
            .set(5, &roles, Four)
            .at(15, all_at("999"))
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
        // S9: the other value one-digit too, so two values re-align when the width changes.
        let others: Vec<(&'static str, String)> = shown(mode)
            .iter()
            .map(|&m| {
                let value = if roles.contains(&m) {
                    "9".to_owned()
                } else {
                    level(m, OneDigit)
                };
                (m, value)
            })
            .collect();
        out.push(
            Script::with("S9", mode, &detail(v, &[]), &others)
                .at(2, all_at("10"))
                .at(4, all_at("9"))
                .end(End::Total(6)),
        );
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
                .map(|&m| Step::Set(m, offset(m, High, 11, i64::from(i))))
                .collect();
            every_tenth = every_tenth.at(10 * i, steps);
        }
        out.push(every_tenth.end(End::Total(212)));
        for phase in 0..5 {
            let mut every_tenth = s("S13", &["four digits", &format!("phase {phase}")], Four);
            for i in 1..=5 {
                let steps: Vec<Step> = roles
                    .iter()
                    .map(|&m| Step::Set(m, offset(m, Four, 111, i64::from(i))))
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
        let boundaries = [
            "0", "0.49", "0.5", "9.49", "9.5", "99.49", "99.5", "999.49", "999.5", "9999.49",
            "9999.5",
        ];
        let mut s20 = sv("S20", &[], boundaries[0]);
        for (tick, value) in (1..).zip(&boundaries[1..]) {
            s20 = s20.at(tick, all_at(value));
        }
        out.push(s20.set(11, &roles, Base).end(End::UntilStopped(2)));
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
            Script::new("S10", mode, &[], top, High)
                .set(6, bottom, High)
                .set(12, top, Base)
                .set(18, top, High)
                .set(18, bottom, Base)
                .set(24, top, Base)
                .end(End::UntilStopped(2)),
        );
        out.push(
            Script::new("S10b", mode, &[], top, Four)
                .at(5, [Step::Set(names[0], "999".to_owned())])
                .set(10, top, Four)
                .at(15, [Step::Set(names[0], "99".to_owned())])
                .set(15, bottom, Four)
                .set(25, names, Base)
                .end(End::UntilStopped(2)),
        );
    }
    out
}

/// S14–S18, S21 and S23–S26: settings, mode switches and the device.
fn settings_scenarios(mode: u16) -> Vec<Scenario> {
    use Level::{Base, High};
    let mut out = Vec::new();
    out.extend(mode_switches(mode));
    for &v in Variant::of(mode) {
        let roles = v.roles(mode);
        let s = |id: &str, rest: &[&str], start: Level| {
            Script::new(id, mode, &detail(v, rest), &roles, start)
        };
        for (start, label) in [(Base, "static"), (High, "scrolling")] {
            out.push(
                s("S15", &[label], start)
                    .at(3, [Step::UnitSaved(true)])
                    .at(8, [Step::UnitSaved(false)])
                    .end(End::Total(12)),
            );
            let mut turns = s("S16", &[label], start);
            for (tick, d) in [(2, 1), (4, 1), (6, 1), (8, 1), (10, 2), (13, 3), (16, 1)] {
                turns = turns.at(tick, [Step::Reset(Cause::Rotation(d))]);
            }
            out.push(turns.end(End::Total(20)));
        }
        out.push(s("S17", &["99 or less"], Base).end(End::Total(3)));
        out.push(s("S17", &["100 or more"], High).end(End::Total(23)));
        let cpu = |value: &str| Step::Set("CPU Usage", value.to_owned());
        let cpu_back =
            |start: Level| Step::Set("CPU Usage", start_value("CPU Usage", &roles, start));
        out.push(
            s("S18", &["the cooler away"], High)
                .at(5, [Step::CoolerAway])
                .at(10, [Step::CoolerBack])
                .end(End::Total(25)),
        );
        out.push(
            s("S18", &["wake-up, node still there"], High)
                .at(7, [Step::Wake, cpu("unavailable")])
                .at(17, [cpu_back(High)])
                .end(End::Total(30)),
        );
        out.push(
            s("S18", &["wake-up, node gone"], High)
                .at(7, [Step::Wake, Step::CoolerAway, cpu("unavailable")])
                .at(12, [Step::CoolerBack])
                .at(17, [cpu_back(High)])
                .end(End::Total(30)),
        );
        out.push(
            s("S21", &["rotation"], High)
                .at(6, [Step::Reset(Cause::Rotation(1))])
                .set(6, &roles, Base)
                .end(End::Total(10)),
        );
        let next = mode % 6 + 1;
        out.push(
            s("S21", &[&format!("into mode {next}")], High)
                .at(6, [Step::Reset(Cause::Mode(next))])
                .set(6, &roles, Base)
                .end(End::Total(10)),
        );
        for (start, label) in [(Base, "static"), (High, "scrolling")] {
            out.push(
                s("S23", &[label], start)
                    .at(3, [Step::Refused(9, "not yet available")])
                    .at(6, [Step::Refused(next, "unavailable")])
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
        let gpu = |value: &str| Step::Set("Gpu usage", value.to_owned());
        out.push(
            s("S25", &["static"], Base)
                .at(3, [Step::Reset(Cause::AnotherGpu), gpu("71")])
                .end(End::Total(6)),
        );
        let fresh = |base: Level| -> Vec<Step> {
            roles
                .iter()
                .map(|&m| Step::Set(m, offset(m, base, 2, 1)))
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
            let new_reading = if roles.contains(&"Gpu usage") {
                "116"
            } else {
                "71"
            };
            out.push(
                s("S25", &["scrolling", &at_k], High)
                    .at(t, [Step::Reset(Cause::AnotherGpu), gpu(new_reading)])
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
    // S17's first second: CPU Usage, a reading averaged over the interval, shows dashes until
    // its second sample (display.md, Unavailable values).
    if shown(mode).contains(&"CPU Usage") {
        out.push(
            Script::with(
                "S17",
                mode,
                &["the first second's dashes, then 100 or more".to_owned()],
                &[("CPU Usage", "unavailable".to_owned())],
            )
            .set(10, &["CPU Usage"], Level::High)
            .end(End::Total(32)),
        );
    }
    out
}

/// The reading `metric` starts at in a scenario whose `roles` start at `start`.
fn start_value(metric: &'static str, roles: &[&'static str], start: Level) -> String {
    if roles.contains(&metric) {
        level(metric, start)
    } else {
        level(metric, Level::Base)
    }
}

/// S14: a switch from `mode` into every other Load mode, at every k of a scrolling strip, with
/// values below 100, above it, and mixed.
fn mode_switches(mode: u16) -> Vec<Scenario> {
    let sets: [(&str, [&str; 3]); 3] = [
        ("values below 100", ["42", "67", "85"]),
        ("values above 100", ["100", "104", "109"]),
        ("CPU above 100", ["100", "67", "85"]),
    ];
    let mut out = Vec::new();
    for target in (1..=6).filter(|&t| t != mode) {
        for (label, values) in &sets {
            let readings: Vec<(&'static str, String)> = LOAD
                .iter()
                .zip(values)
                .map(|(&m, &v)| (m, v.to_owned()))
                .collect();
            let scrolls = shown(mode).iter().any(|m| {
                let i = LOAD.iter().position(|l| l == m).expect("a Load metric");
                values[i].len() >= 3
            });
            let into = format!("into mode {target}");
            if scrolls {
                for k in 0..21 {
                    let t = tick_at(k, 21);
                    let detail = [into.clone(), (*label).to_owned(), format!("k {k}")];
                    out.push(
                        Script::with("S14", mode, &detail, &readings)
                            .at(t, [Step::Reset(Cause::Mode(target))])
                            .end(End::Total(t + 5)),
                    );
                }
            } else {
                let detail = [into.clone(), (*label).to_owned()];
                out.push(
                    Script::with("S14", mode, &detail, &readings)
                        .at(3, [Step::Reset(Cause::Mode(target))])
                        .end(End::Total(8)),
                );
            }
        }
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

/// Runs a scenario through the movement procedure.
fn run(scenario: &Scenario) -> Vec<Tick> {
    let last = *scenario.script.keys().last().expect("tick 0");
    let mut values: HashMap<&'static str, Option<Ratio>> = HashMap::new();
    let mut mode = scenario.mode;
    let mut movement: Option<Movement> = None;
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
                Step::Reset(_) => reset = true,
                Step::Missed => missed = true,
                _ => {}
            }
        }
        let readings: Vec<Option<Ratio>> = shown(mode)
            .iter()
            .map(|m| {
                *values
                    .get(m)
                    .expect("every Load metric has a reading from tick 0")
            })
            .collect();
        let strip = strip_of(mode, &readings, false);
        let m = match &mut movement {
            Some(m) if !reset => {
                if changed {
                    m.change(strip);
                }
                m
            }
            _ => movement.insert(Movement::reset(strip)),
        };
        let (x, home, cycle) = (m.x(), m.strip().home(), m.strip().cycle());
        let frame = (!missed).then(|| m.tick());
        let moving = m.moving();
        ticks.push(Tick {
            n,
            frame,
            reset,
            x,
            moving,
            home,
            cycle,
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

/// Every scenario's heading and run, in file order.
pub fn runs() -> Vec<(String, u16, Vec<Tick>)> {
    scenarios()
        .iter()
        .map(|s| (s.heading.clone(), s.mode, run(s)))
        .collect()
}

/// The file: the scenarios, then every frame they name, once.
pub fn file() -> String {
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut frames: Vec<String> = Vec::new();
    let mut lines = vec![
        "# Load scenarios: modes 1–6, rotation 0".to_owned(),
        String::new(),
    ];
    for (heading, _, ticks) in runs() {
        lines.push(heading);
        lines.push(String::new());
        for tick in ticks {
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
            let mut previous: Option<(i64, i64)> = None;
            for tick in ticks.iter().filter(|t| t.frame.is_some()) {
                let re_centred = !tick.moving && tick.x == tick.home;
                if let Some((x, cycle)) = previous
                    && !tick.reset
                    && !re_centred
                {
                    let was = |c: i64| (c - x).rem_euclid(cycle) == 0;
                    for c in on_screen(tick.x, tick.cycle) {
                        assert!(
                            was(c) || was(c + 1),
                            "{heading}, tick {}: a copy at column {c}",
                            tick.n
                        );
                    }
                }
                previous = Some((tick.x, tick.cycle));
            }
        }
    }

    // S1, S12: a strip that must scroll moves one column per frame from home; a stopped strip
    // is at home.
    #[test]
    fn a_stopped_strip_is_always_at_home() {
        for (heading, _, ticks) in runs() {
            for tick in &ticks {
                if !tick.moving {
                    assert_eq!(tick.x, tick.home, "{heading}, tick {}", tick.n);
                }
            }
        }
    }

    #[test]
    fn every_scenario_names_a_load_mode_and_starts_with_a_reset() {
        for (heading, mode, ticks) in runs() {
            assert!((1..=6).contains(&mode), "{heading}");
            assert!(ticks[0].reset, "{heading}");
            assert_eq!(ticks[0].n, 0, "{heading}");
            assert!(
                ticks.windows(2).all(|w| w[1].n == w[0].n + 1),
                "{heading}: ticks in order"
            );
        }
    }
}
