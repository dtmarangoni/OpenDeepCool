//! The expected frames of `docs/spec/frames/`, loaded into typed cases (`docs/spec/frames/README.md`,
//! Format), the dash cases and the scenarios (README, Dash cases and scenarios). A file that
//! doesn't parse panics, naming its file and line: a mis-read oracle must never pass silently.

use std::fmt;
use std::fs;
use std::path::Path;
use std::time::Duration;

use odc_protocol::{Frame, Rotation};
use odc_sensors::{Exact, Metric};

const FRAMES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/spec/frames");
const SIZE: usize = Frame::SIZE;

/// One expected-frame file: every case of one mode.
#[derive(Debug)]
pub struct ModeFrames {
    pub title: String,
    pub cases: Vec<Case>,
}

/// One `##` section: a rotation, a unit and readings, or the Dancer's pose, and what the cooler
/// must show for them.
#[derive(Debug)]
pub struct Case {
    /// The heading as written, for messages.
    pub heading: String,
    pub rotation: Rotation,
    /// °C or °F, when the heading names one.
    pub unit: Option<Unit>,
    /// The Dancer's pose, 0 to 10, in mode 23's cases.
    pub pose: Option<u8>,
    /// The readings, with the metric names exactly as written (`CPU Usage`, `Gpu temperature`):
    /// an exact value, or `None` for `unavailable`.
    pub readings: Vec<(String, Option<Exact>)>,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Celsius,
    Fahrenheit,
}

#[derive(Debug)]
pub enum Kind {
    /// One picture, sent unchanged while the readings stay the same.
    Static(Picture),
    /// One cycle in time order, one picture per 100 ms tick; the first is the home position.
    Scrolling(Vec<Picture>),
    /// One loop of pictures, each with its duration.
    Loop(Vec<(Duration, Picture)>),
}

impl Kind {
    /// Every picture of the case, in order.
    pub fn pictures(&self) -> Vec<&Picture> {
        match self {
            Kind::Static(picture) => vec![picture],
            Kind::Scrolling(pictures) => pictures.iter().collect(),
            Kind::Loop(steps) => steps.iter().map(|(_, picture)| picture).collect(),
        }
    }
}

/// One pixel of an expected picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pixel {
    Off,
    On,
    /// `?`: a spectrum pixel. It depends on the sound, so the spectrum's layout rules check it.
    Any,
}

/// A 14 × 14 expected picture, row 1 at the top.
#[derive(Clone, PartialEq, Eq)]
pub struct Picture {
    rows: [[Pixel; SIZE]; SIZE],
}

impl Picture {
    /// A picture from 14 rows of 14 characters: `#` on, `.` off, `?` either.
    ///
    /// # Panics
    ///
    /// When a row isn't 14 of those characters.
    pub fn from_rows(rows: &[&str; SIZE]) -> Self {
        let mut picture = Picture {
            rows: [[Pixel::Off; SIZE]; SIZE],
        };
        for (r, line) in rows.iter().enumerate() {
            picture.rows[r] = parse_row(line).unwrap_or_else(|e| panic!("row {}: {e}", r + 1));
        }
        picture
    }

    /// Pixel (`row`, `col`), numbered from 1.
    pub fn get(&self, row: usize, col: usize) -> Pixel {
        self.rows[row - 1][col - 1]
    }

    /// Whether `frame` shows this picture. `Any` matches either value.
    pub fn matches(&self, frame: &Frame) -> bool {
        (1..=SIZE).all(|r| (1..=SIZE).all(|c| pixel_matches(self.get(r, c), frame.get(r, c))))
    }

    /// The picture as a frame, or `None` when it holds spectrum pixels.
    pub fn to_frame(&self) -> Option<Frame> {
        let mut frame = Frame::blank();
        for r in 1..=SIZE {
            for c in 1..=SIZE {
                match self.get(r, c) {
                    Pixel::On => frame.set(r, c, true),
                    Pixel::Off => {}
                    Pixel::Any => return None,
                }
            }
        }
        Some(frame)
    }

    /// Whether any pixel is a spectrum pixel.
    pub fn has_any(&self) -> bool {
        self.rows.iter().flatten().any(|&p| p == Pixel::Any)
    }
}

pub(crate) fn pixel_matches(expected: Pixel, on: bool) -> bool {
    match expected {
        Pixel::Any => true,
        Pixel::On => on,
        Pixel::Off => !on,
    }
}

pub(crate) fn pixel_char(pixel: Pixel) -> char {
    match pixel {
        Pixel::On => '#',
        Pixel::Off => '.',
        Pixel::Any => '?',
    }
}

/// Prints the picture as the files write it.
impl fmt::Debug for Picture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Picture [")?;
        for row in &self.rows {
            let line: String = row.iter().map(|&p| pixel_char(p)).collect();
            writeln!(f, "    {line}")?;
        }
        write!(f, "]")
    }
}

/// Each metric's own value where a case or a scenario names none (AGENTS.md rule 5): distinct
/// from each other and from every reading the files use, so drawing the wrong metric shows.
const FILLERS: [(Metric, i64); 7] = [
    (Metric::CpuTemperature, 11),
    (Metric::CpuUsage, 23),
    (Metric::CpuPower, 37),
    (Metric::GpuTemperature, 51),
    (Metric::GpuUsage, 73),
    (Metric::GpuPower, 83),
    (Metric::MemoryLoad, 97),
];

/// Every metric's reading: those named, by their names in the files, and a filler of its own for
/// every other metric.
///
/// # Panics
///
/// When a name isn't a metric's.
pub fn every_metric(readings: &[(String, Option<Exact>)]) -> Vec<(Metric, Option<Exact>)> {
    for (name, _) in readings {
        assert!(
            Metric::from_spec_name(name).is_some(),
            "{name:?} isn't a metric"
        );
    }
    FILLERS
        .iter()
        .map(|&(metric, filler)| {
            let named = readings
                .iter()
                .rev()
                .find(|(name, _)| Metric::from_spec_name(name) == Some(metric));
            (
                metric,
                named.map_or(Some(Exact::whole(filler)), |&(_, value)| value),
            )
        })
        .collect()
}

/// A reading as the files write it: an exact decimal, negative ones included, or `unavailable`
/// (`None`). Never through floating point.
pub fn reading(text: &str) -> Option<Option<Exact>> {
    if text == "unavailable" {
        return Some(None);
    }
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    if whole.is_empty() || !all_digits(whole) || !all_digits(fraction) || digits.ends_with('.') {
        return None;
    }
    let num: i64 = format!("{whole}{fraction}").parse().ok()?;
    let den = 10_u64.checked_pow(u32::try_from(fraction.len()).ok()?)?;
    Exact::new(if negative { -num } else { num }, den).map(Some)
}

/// Loads `docs/spec/frames/mode-NNN.txt`.
///
/// # Panics
///
/// When the file can't be read or doesn't parse, naming the file and line.
pub fn load_mode(mode: u16) -> ModeFrames {
    let name = format!("mode-{mode:03}.txt");
    let path = Path::new(FRAMES_DIR).join(&name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    parse(&name, mode, &text)
}

/// Loads `docs/spec/frames/unavailable.txt`: each dash case with the mode its heading names.
///
/// # Panics
///
/// When the file can't be read or doesn't parse, naming the file and line.
pub fn load_unavailable() -> Vec<(u16, Case)> {
    let name = "unavailable.txt";
    let path = Path::new(FRAMES_DIR).join(name);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    parse_dash_cases(name, &text)
}

fn parse_dash_cases(file: &str, text: &str) -> Vec<(u16, Case)> {
    let mut lines = Lines {
        file,
        lines: text.lines().collect(),
        at: 0,
    };
    match lines.next() {
        Some((_, line)) if line.starts_with("# ") => {}
        Some((n, line)) => lines.fail(n, format!("expected a `#` title, found {line:?}")),
        None => lines.fail(1, "the file is empty"),
    }
    let mut cases = Vec::new();
    loop {
        lines.skip_blank();
        let Some((n, heading)) = lines.next() else {
            break;
        };
        let (mode, case) = parse_case(&mut lines, n, heading, true);
        cases.push((mode.expect("a dash case names its mode"), case));
    }
    if cases.is_empty() {
        lines.fail(1, "the file has no case");
    }
    cases
}

fn parse_row(line: &str) -> Result<[Pixel; SIZE], String> {
    let mut row = [Pixel::Off; SIZE];
    let mut count = 0;
    for (i, ch) in line.chars().enumerate() {
        let pixel = match ch {
            '#' => Pixel::On,
            '.' => Pixel::Off,
            '?' => Pixel::Any,
            _ => return Err(format!("{ch:?} isn't '#', '.' or '?' in {line:?}")),
        };
        if i < SIZE {
            row[i] = pixel;
        }
        count += 1;
    }
    if count != SIZE {
        return Err(format!("{line:?} isn't 14 pixels wide"));
    }
    Ok(row)
}

/// The text of one file, line by line, numbered from 1.
struct Lines<'a> {
    file: &'a str,
    lines: Vec<&'a str>,
    at: usize,
}

impl<'a> Lines<'a> {
    fn peek(&self) -> Option<&'a str> {
        self.lines.get(self.at).copied()
    }

    /// The next line and its number.
    fn next(&mut self) -> Option<(usize, &'a str)> {
        let line = self.peek()?;
        self.at += 1;
        Some((self.at, line))
    }

    fn skip_blank(&mut self) {
        while self.peek().is_some_and(str::is_empty) {
            self.at += 1;
        }
    }

    fn fail(&self, line: usize, message: impl fmt::Display) -> ! {
        panic!("{}:{line}: {message}", self.file);
    }
}

const SCROLL_NOTE: &str = " frames in time order";
const LOOP_NOTE: &str = "one loop, picture durations in order: ";

fn parse(file: &str, mode: u16, text: &str) -> ModeFrames {
    let mut lines = Lines {
        file,
        lines: text.lines().collect(),
        at: 0,
    };
    let title_prefix = format!("# Mode {mode}: ");
    let title = match lines.next() {
        Some((_, line)) if line.starts_with(&title_prefix) => line[title_prefix.len()..].to_owned(),
        Some((n, line)) => lines.fail(n, format!("expected {title_prefix:?}…, found {line:?}")),
        None => lines.fail(1, "the file is empty"),
    };
    let mut cases = Vec::new();
    loop {
        lines.skip_blank();
        let Some((n, heading)) = lines.next() else {
            break;
        };
        cases.push(parse_case(&mut lines, n, heading, false).1);
    }
    if cases.is_empty() {
        lines.fail(1, "the file has no case");
    }
    ModeFrames { title, cases }
}

enum Expect {
    Static,
    Scrolling(usize),
    Loop(Vec<Duration>),
}

/// One case; with `with_mode`, its heading starts with `mode <N>`, which comes back with it.
fn parse_case(
    lines: &mut Lines<'_>,
    n: usize,
    heading: &str,
    with_mode: bool,
) -> (Option<u16>, Case) {
    let Some(rest) = heading.strip_prefix("## ") else {
        lines.fail(n, format!("expected a `##` heading, found {heading:?}"));
    };
    let mut parts = rest.split(", ");
    let mode = with_mode.then(|| {
        parts
            .next()
            .and_then(|p| p.strip_prefix("mode "))
            .and_then(|m| m.parse::<u16>().ok())
            .unwrap_or_else(|| lines.fail(n, "the heading doesn't start with `mode <N>`"))
    });
    let rotation = parts
        .next()
        .and_then(|p| p.strip_prefix("rotation "))
        .and_then(|r| r.parse::<u8>().ok())
        .and_then(Rotation::from_setting)
        .unwrap_or_else(|| lines.fail(n, "the heading doesn't start with `rotation 0` to `3`"));
    let mut unit = None;
    let mut pose = None;
    let mut readings = Vec::new();
    for part in parts {
        let alone = unit.is_none() && pose.is_none() && readings.is_empty();
        match part {
            "°C" if alone => unit = Some(Unit::Celsius),
            "°F" if alone => unit = Some(Unit::Fahrenheit),
            _ if alone && part.starts_with("pose ") => {
                pose = part["pose ".len()..]
                    .parse::<u8>()
                    .ok()
                    .filter(|k| *k <= 10)
                    .or_else(|| lines.fail(n, format!("{part:?} isn't `pose 0` to `10`")));
            }
            _ => match part.split_once(" = ").map(|(k, v)| (k, reading(v))) {
                Some((name, Some(value))) if !name.is_empty() => {
                    readings.push((name.to_owned(), value))
                }
                _ => lines.fail(n, format!("{part:?} is neither a unit nor `Name = value`")),
            },
        }
    }
    lines.skip_blank();
    let expect = match lines.peek() {
        Some(note) if note.contains(SCROLL_NOTE) => {
            let (m, note) = lines.next().expect("a line");
            let count = note
                .split_once(SCROLL_NOTE)
                .and_then(|(count, _)| count.parse::<usize>().ok())
                .unwrap_or_else(|| lines.fail(m, "no frame count before `frames in time order`"));
            Expect::Scrolling(count)
        }
        Some(note) if note.starts_with(LOOP_NOTE) => {
            let (m, note) = lines.next().expect("a line");
            let durations = note[LOOP_NOTE.len()..]
                .split(", ")
                .map(|d| d.strip_suffix(" ms").and_then(|ms| ms.parse::<u64>().ok()))
                .map(|ms| ms.map(Duration::from_millis))
                .collect::<Option<Vec<_>>>()
                .unwrap_or_else(|| lines.fail(m, "a duration isn't `<n> ms`"));
            Expect::Loop(durations)
        }
        _ => Expect::Static,
    };
    let pictures = parse_pictures(lines);
    let kind = match expect {
        Expect::Static if pictures.len() == 1 => {
            Kind::Static(pictures.into_iter().next().expect("one picture"))
        }
        Expect::Static => lines.fail(
            n,
            format!("a static case has one picture, not {}", pictures.len()),
        ),
        Expect::Scrolling(count) if count >= 21 && pictures.len() == count => {
            Kind::Scrolling(pictures)
        }
        Expect::Scrolling(count) => lines.fail(
            n,
            format!(
                "a cycle of {count} frames (at least 21) has {} pictures",
                pictures.len()
            ),
        ),
        Expect::Loop(durations) if durations.len() == pictures.len() => {
            Kind::Loop(durations.into_iter().zip(pictures).collect())
        }
        Expect::Loop(durations) => lines.fail(
            n,
            format!(
                "a loop of {} durations has {} pictures",
                durations.len(),
                pictures.len()
            ),
        ),
    };
    let case = Case {
        heading: heading.to_owned(),
        rotation,
        unit,
        pose,
        readings,
        kind,
    };
    (mode, case)
}

/// Whether a line is a heading, `#` to `###` and a space. A picture row can't be one: its first
/// 14 characters hold no space.
fn is_heading(line: &str) -> bool {
    ["# ", "## ", "### "].iter().any(|h| line.starts_with(h))
}

/// Reads the picture blocks up to the next heading: 14 rows each, several pictures to a row
/// separated by two spaces.
fn parse_pictures(lines: &mut Lines<'_>) -> Vec<Picture> {
    let mut pictures = Vec::new();
    loop {
        lines.skip_blank();
        match lines.peek() {
            None => break,
            Some(line) if is_heading(line) => break,
            Some(_) => {}
        }
        let mut block: Vec<Picture> = Vec::new();
        for r in 0..SIZE {
            let (n, line) = match lines.next() {
                Some((n, line)) if !line.is_empty() && !is_heading(line) => (n, line),
                Some((n, _)) => lines.fail(n, format!("a picture block ends after {r} of 14 rows")),
                None => lines.fail(
                    lines.at,
                    format!("the file ends after {r} of 14 picture rows"),
                ),
            };
            let cells: Vec<&str> = line.split("  ").collect();
            if r == 0 {
                block = vec![
                    Picture {
                        rows: [[Pixel::Off; SIZE]; SIZE],
                    };
                    cells.len()
                ];
            } else if cells.len() != block.len() {
                lines.fail(
                    n,
                    format!(
                        "{} pictures in this row, {} in the first",
                        cells.len(),
                        block.len()
                    ),
                );
            }
            for (picture, cell) in block.iter_mut().zip(cells) {
                picture.rows[r] = parse_row(cell).unwrap_or_else(|e| lines.fail(n, e));
            }
        }
        pictures.extend(block);
    }
    pictures
}

/// One scenario of `docs/spec/frames/scenarios/`: changes at chosen ticks, and the frame expected
/// at every tick, in rotation 0 (README, Dash cases and scenarios).
#[derive(Debug)]
pub struct Scenario {
    /// The heading as written, for messages.
    pub heading: String,
    /// The scenario's number in `display.md`, `S1` to `S26`.
    pub id: String,
    /// The mode it starts in.
    pub mode: u16,
    /// The unit it starts in: the one tick 0 names, °C when it names none (README, Dash cases
    /// and scenarios). Tick 0's changes don't repeat it.
    pub unit: Unit,
    /// Ticks 0, 1, 2 … in order.
    pub ticks: Vec<Tick>,
}

/// One tick of a scenario.
#[derive(Debug)]
pub struct Tick {
    pub n: u32,
    /// In the order written. On one tick the readings change first, then a reset applies.
    pub changes: Vec<Change>,
    /// The frame drawn; `None` when the tick's deadline is missed.
    pub frame: Option<Picture>,
}

/// What changes at a tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Reset(Reset),
    /// A metric's new reading, `None` for `unavailable`.
    Reading {
        metric: String,
        value: Option<Exact>,
    },
    /// A unit change in a mode without a temperature: only saved.
    UnitSaved(Unit),
    /// Choosing what's already set: nothing changes.
    AlreadySet(String),
    /// A change the service refuses: nothing changes.
    Refused {
        mode: u16,
        reason: String,
    },
    /// The cooler disconnects; frames are drawn but don't reach it.
    CoolerAway,
    /// The cooler is back: `03 08` before this tick's frame.
    CoolerBack,
    /// Back from standby: `03 08` before this tick's frame.
    Wake,
    /// The deadline is missed: no frame, and the strip doesn't move.
    Missed,
}

/// Why the picture restarts at home.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reset {
    StartUp,
    Mode(u16),
    /// Quarter turns added to the rotation.
    Rotation(u8),
    /// A unit change in a mode that shows a temperature.
    Unit(Unit),
    AnotherGpu,
    Refresh,
}

/// Loads `docs/spec/frames/scenarios/<name>`, for example `mode-001.txt`.
///
/// # Panics
///
/// When the file can't be read or doesn't parse, naming the file and line; when a tick names a
/// frame the file doesn't define, or the file defines a frame no tick names.
pub fn load_scenarios(name: &str) -> Vec<Scenario> {
    let file = format!("scenarios/{name}");
    let path = Path::new(FRAMES_DIR).join(&file);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    parse_scenarios(&file, &text)
}

/// Loads mode `mode`'s scenarios, `docs/spec/frames/scenarios/mode-NNN.txt`: every scenario
/// that starts in it (Phase 8 plan, Task 1: one file per mode).
///
/// # Panics
///
/// As [`load_scenarios`].
pub fn mode_scenarios(mode: u16) -> Vec<Scenario> {
    load_scenarios(&format!("mode-{mode:03}.txt"))
}

const ARROW: &str = " → ";

/// The frame each tick names, and the line it's on; `None` for a missed tick.
type FrameRefs = Vec<Option<(usize, String)>>;

fn parse_scenarios(file: &str, text: &str) -> Vec<Scenario> {
    let mut lines = Lines {
        file,
        lines: text.lines().collect(),
        at: 0,
    };
    match lines.next() {
        Some((_, line)) if line.starts_with("# ") && line != "# Frames" => {}
        Some((n, line)) => lines.fail(n, format!("expected a `#` title, found {line:?}")),
        None => lines.fail(1, "the file is empty"),
    }
    // The scenarios, each tick naming its frame and the line it's on.
    let mut parsed: Vec<(Scenario, FrameRefs)> = Vec::new();
    loop {
        lines.skip_blank();
        let Some((n, heading)) = lines.next() else {
            lines.fail(lines.at, "the file has no `# Frames` section");
        };
        if heading == "# Frames" {
            break;
        }
        parsed.push(parse_scenario(&mut lines, n, heading));
    }
    let mut frames: Vec<(String, Picture, bool)> = Vec::new();
    loop {
        lines.skip_blank();
        let Some((n, heading)) = lines.next() else {
            break;
        };
        let Some(id) = heading.strip_prefix("### ") else {
            lines.fail(n, format!("expected a `### f<n>` frame, found {heading:?}"));
        };
        if frames.iter().any(|(defined, _, _)| defined == id) {
            lines.fail(n, format!("{id} is defined twice"));
        }
        let pictures = parse_pictures(&mut lines);
        if pictures.len() != 1 {
            lines.fail(n, format!("{id} has {} pictures, not one", pictures.len()));
        }
        let picture = pictures.into_iter().next().expect("one picture");
        frames.push((id.to_owned(), picture, false));
    }
    if parsed.is_empty() {
        lines.fail(1, "the file has no scenario");
    }
    let mut scenarios = Vec::new();
    for (mut scenario, refs) in parsed {
        for (tick, frame) in scenario.ticks.iter_mut().zip(refs) {
            if let Some((n, id)) = frame {
                let Some(found) = frames.iter_mut().find(|(defined, _, _)| *defined == id) else {
                    lines.fail(n, format!("{id} isn't defined under `# Frames`"));
                };
                found.2 = true;
                tick.frame = Some(found.1.clone());
            }
        }
        scenarios.push(scenario);
    }
    if let Some((id, _, _)) = frames.iter().find(|(_, _, used)| !used) {
        panic!("{file}: {id} is defined but no tick names it");
    }
    scenarios
}

fn parse_scenario(lines: &mut Lines<'_>, n: usize, heading: &str) -> (Scenario, FrameRefs) {
    let Some(rest) = heading.strip_prefix("## ") else {
        lines.fail(
            n,
            format!("expected a `##` scenario heading, found {heading:?}"),
        );
    };
    let mut parts = rest.split(" · ");
    let id = parts
        .next()
        .filter(|id| {
            id.strip_prefix('S').is_some_and(|number| {
                let digits = number.trim_end_matches(|c: char| c.is_ascii_lowercase());
                !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
            })
        })
        .unwrap_or_else(|| lines.fail(n, "the heading doesn't start with a scenario, `S1` …"));
    let mode = parts
        .next()
        .and_then(|p| p.strip_prefix("mode "))
        .and_then(|m| m.parse::<u16>().ok())
        .unwrap_or_else(|| lines.fail(n, "the heading's second part isn't `mode <N>`"));
    lines.skip_blank();
    let mut ticks = Vec::new();
    let mut refs = Vec::new();
    let mut unit = None;
    while let Some(line) = lines.peek() {
        if line.is_empty() || line.starts_with('#') {
            break;
        }
        let (m, line) = lines.next().expect("a line");
        let Some((left, frame)) = line.rsplit_once(ARROW) else {
            lines.fail(
                m,
                format!("a tick is `<n>: <changes> → <frame>`, not {line:?}"),
            );
        };
        let Some((number, changes)) = left.split_once(": ") else {
            lines.fail(m, format!("a tick starts with `<n>: `, not {line:?}"));
        };
        let tick: u32 = number
            .parse()
            .unwrap_or_else(|_| lines.fail(m, format!("{number:?} isn't a tick number")));
        if tick as usize != ticks.len() {
            lines.fail(
                m,
                format!("tick {tick} comes after tick {}", ticks.len() as i64 - 1),
            );
        }
        let mut named = Vec::new();
        if changes != "-" {
            for c in changes.split("; ") {
                // The unit the scenario starts in: tick 0 only, once.
                let start = match c {
                    "unit celsius" => Some(Unit::Celsius),
                    "unit fahrenheit" => Some(Unit::Fahrenheit),
                    _ => None,
                };
                match start {
                    Some(_) if tick != 0 => lines.fail(
                        m,
                        format!("{c:?} without `reset` or `only saved` comes at tick 0 only"),
                    ),
                    Some(_) if unit.is_some() => lines.fail(m, "tick 0 names the unit twice"),
                    Some(_) => unit = start,
                    None => named.push(
                        change(c).unwrap_or_else(|| lines.fail(m, format!("{c:?} isn't a change"))),
                    ),
                }
            }
        }
        let changes = named;
        let missed = changes.contains(&Change::Missed);
        let frame = match frame {
            "no frame" if missed => None,
            "no frame" => lines.fail(m, "only a missed tick has no frame"),
            _ if missed => lines.fail(m, "a missed tick has no frame"),
            id if id
                .strip_prefix('f')
                .is_some_and(|k| k.parse::<u32>().is_ok()) =>
            {
                Some((m, id.to_owned()))
            }
            other => lines.fail(m, format!("{other:?} isn't `f<n>` or `no frame`")),
        };
        ticks.push(Tick {
            n: tick,
            changes,
            frame: None,
        });
        refs.push(frame);
    }
    if !ticks
        .first()
        .is_some_and(|t| t.changes.contains(&Change::Reset(Reset::StartUp)))
    {
        lines.fail(n, "a scenario starts with tick 0 and `reset start-up`");
    }
    let scenario = Scenario {
        heading: heading.to_owned(),
        id: id.to_owned(),
        mode,
        unit: unit.unwrap_or(Unit::Celsius),
        ticks,
    };
    (scenario, refs)
}

/// One change as the scenario files write it.
fn change(text: &str) -> Option<Change> {
    let reset = |r: Reset| Some(Change::Reset(r));
    match text {
        "reset start-up" => return reset(Reset::StartUp),
        "reset another GPU" => return reset(Reset::AnotherGpu),
        "reset Refresh" => return reset(Reset::Refresh),
        "reset unit celsius" => return reset(Reset::Unit(Unit::Celsius)),
        "reset unit fahrenheit" => return reset(Reset::Unit(Unit::Fahrenheit)),
        "unit celsius, only saved" => return Some(Change::UnitSaved(Unit::Celsius)),
        "unit fahrenheit, only saved" => return Some(Change::UnitSaved(Unit::Fahrenheit)),
        "cooler away" => return Some(Change::CoolerAway),
        "cooler back" => return Some(Change::CoolerBack),
        "wake" => return Some(Change::Wake),
        "missed" => return Some(Change::Missed),
        _ => {}
    }
    if let Some(mode) = text.strip_prefix("reset mode ") {
        return reset(Reset::Mode(mode.parse().ok()?));
    }
    if let Some(turns) = text.strip_prefix("reset rotation +") {
        let turns: u8 = turns.parse().ok()?;
        return (1..=3)
            .contains(&turns)
            .then_some(Change::Reset(Reset::Rotation(turns)));
    }
    if let Some(what) = text.strip_prefix("already set: ") {
        return (!what.is_empty()).then(|| Change::AlreadySet(what.to_owned()));
    }
    if let Some(rest) = text.strip_prefix("refused: mode ") {
        let (mode, reason) = rest.split_once(", ")?;
        return Some(Change::Refused {
            mode: mode.parse().ok()?,
            reason: reason.to_owned(),
        });
    }
    let (metric, value) = text.split_once(" = ")?;
    Some(Change::Reading {
        metric: (!metric.is_empty()).then(|| metric.to_owned())?,
        value: reading(value)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file loads into typed cases, as many as it has `##` headings (284 when Phase 5
    /// ran, 328 with Phase 9's Dancer); cycles have at least 21 pictures, `?` appears only in the
    /// spectrum's modes 13–22, and mode 23 has each of the Dancer's eleven poses in each
    /// rotation.
    #[test]
    fn every_expected_frame_file_loads() {
        let mut modes: Vec<u16> = fs::read_dir(FRAMES_DIR)
            .expect("docs/spec/frames/ is readable")
            .map(|entry| entry.expect("a directory entry").file_name())
            .filter_map(|name| {
                let name = name.into_string().ok()?;
                name.strip_prefix("mode-")?
                    .strip_suffix(".txt")?
                    .parse()
                    .ok()
            })
            .collect();
        modes.sort_unstable();
        assert!(!modes.is_empty());
        let (mut cases, mut headings) = (0, 0);
        let mut dancer = Vec::new();
        for mode in modes {
            let text =
                fs::read_to_string(Path::new(FRAMES_DIR).join(format!("mode-{mode:03}.txt")))
                    .expect("a mode file");
            headings += text.lines().filter(|l| l.starts_with("## ")).count();
            let frames = load_mode(mode);
            assert!(!frames.title.is_empty());
            for case in &frames.cases {
                if let Kind::Scrolling(pictures) = &case.kind {
                    assert!(pictures.len() >= 21, "mode {mode}, {}", case.heading);
                }
                if !(13..=22).contains(&mode) {
                    let spectrum = case.kind.pictures().iter().any(|p| p.has_any());
                    assert!(
                        !spectrum,
                        "mode {mode}, {}: `?` outside Music",
                        case.heading
                    );
                }
            }
            for case in &frames.cases {
                assert_eq!(
                    case.pose.is_some(),
                    mode == 23,
                    "mode {mode}, {}",
                    case.heading
                );
                if let Some(pose) = case.pose {
                    dancer.push((case.rotation.setting(), pose));
                }
            }
            cases += frames.cases.len();
        }
        assert_eq!(cases, headings);
        assert_eq!(cases, 328);
        dancer.sort_unstable();
        dancer.dedup();
        assert_eq!(dancer.len(), 4 * 11, "each pose in each rotation");
    }

    /// The dash cases and the scenarios load too: every heading becomes a case or a scenario.
    #[test]
    fn the_dash_cases_and_the_scenarios_load() {
        let dashes = load_unavailable();
        assert_eq!(dashes.len(), 44);
        assert!(
            dashes
                .iter()
                .all(|(mode, _)| [1, 4, 6, 7, 9, 10, 11, 12].contains(mode))
        );
        // A dash case of a mode with a temperature names its unit.
        for (mode, case) in &dashes {
            assert_eq!(
                case.unit.is_some(),
                [7, 9].contains(mode),
                "{}",
                case.heading
            );
        }
        for mode in 1..=12 {
            let name = format!("mode-{mode:03}.txt");
            let text = fs::read_to_string(Path::new(FRAMES_DIR).join("scenarios").join(&name))
                .expect("a scenario file");
            let scenarios = mode_scenarios(mode);
            let headings = text.lines().filter(|l| l.starts_with("## ")).count();
            assert_eq!(scenarios.len(), headings, "{name}");
            for scenario in &scenarios {
                assert_eq!(scenario.mode, mode, "{name}: {}", scenario.heading);
                check_ticks(scenario);
            }
            // A temperature mode's scenarios run in each unit, and their headings say which.
            if (7..=9).contains(&mode) {
                for (unit, label) in [(Unit::Celsius, " · °C"), (Unit::Fahrenheit, " · °F")] {
                    let mine = scenarios.iter().filter(|s| s.unit == unit);
                    assert!(mine.clone().count() > 0, "{name}: none in {unit:?}");
                    assert!(mine.into_iter().all(|s| s.heading.contains(label)));
                }
            }
        }
    }

    /// A missed tick, and only a missed tick, has no frame.
    fn check_ticks(scenario: &Scenario) {
        for tick in &scenario.ticks {
            let missed = tick.changes.contains(&Change::Missed);
            assert_eq!(
                tick.frame.is_none(),
                missed,
                "{}, tick {}",
                scenario.heading,
                tick.n
            );
        }
    }

    // Phase 8 plan, Task 1: tick 0's unit is the scenario's, and a unit change in a mode with a
    // temperature is a reset.
    #[test]
    fn a_scenario_reads_its_unit_and_its_unit_resets() {
        let text = scenario_file(
            "0: reset start-up; unit fahrenheit; CPU Usage = 100 → f1\n\
             1: reset unit celsius → f1\n\
             2: unit fahrenheit, only saved → f1\n",
        );
        let scenario = &parse_scenarios("test.txt", &text)[0];
        assert_eq!(scenario.unit, Unit::Fahrenheit);
        assert_eq!(
            scenario.ticks[0].changes.len(),
            2,
            "the unit isn't a change"
        );
        assert_eq!(
            scenario.ticks[1].changes,
            [Change::Reset(Reset::Unit(Unit::Celsius))]
        );
        assert_eq!(
            scenario.ticks[2].changes,
            [Change::UnitSaved(Unit::Fahrenheit)]
        );
        let none = &parse_scenarios("test.txt", &scenario_file(START))[0];
        assert_eq!(none.unit, Unit::Celsius, "no unit named runs in °C");
    }

    #[test]
    #[should_panic(
        expected = "test.txt:6: \"unit celsius\" without `reset` or `only saved` comes at tick 0 only"
    )]
    fn a_bare_unit_after_tick_0_is_refused() {
        parse_scenarios(
            "test.txt",
            &scenario_file(&format!("{START}1: unit celsius → f1\n")),
        );
    }

    #[test]
    #[should_panic(expected = "test.txt:5: tick 0 names the unit twice")]
    fn a_unit_named_twice_is_refused() {
        parse_scenarios(
            "test.txt",
            &scenario_file("0: reset start-up; unit celsius; unit fahrenheit → f1\n"),
        );
    }

    #[test]
    fn every_metric_gets_the_named_reading_or_its_own_filler() {
        let all = every_metric(&[
            ("CPU Usage".to_owned(), Some(Exact::whole(42))),
            ("Memory Load".to_owned(), None),
        ]);
        assert_eq!(all.len(), 7);
        assert!(all.contains(&(Metric::CpuUsage, Some(Exact::whole(42)))));
        assert!(all.contains(&(Metric::MemoryLoad, None)));
        assert!(all.contains(&(Metric::GpuUsage, Some(Exact::whole(73)))));
        let mut fillers: Vec<i64> = FILLERS.iter().map(|&(_, f)| f).collect();
        fillers.dedup();
        assert_eq!(fillers.len(), 7, "every filler its own");
    }

    #[test]
    #[should_panic(expected = "\"Cpu usage\" isn't a metric")]
    fn an_unknown_metric_name_is_refused() {
        every_metric(&[("Cpu usage".to_owned(), None)]);
    }

    #[test]
    fn readings_are_exact_decimals_or_unavailable() {
        assert_eq!(reading("42"), Some(Some(Exact::whole(42))));
        assert_eq!(reading("99.49"), Exact::new(9949, 100).map(Some));
        assert_eq!(reading("-0.5"), Exact::new(-5, 10).map(Some));
        assert_eq!(reading("unavailable"), Some(None));
        for bad in ["", "-", ".5", "5.", "1e3", "+1", "4 2", "NaN", "inf"] {
            assert_eq!(reading(bad), None, "{bad:?}");
        }
    }

    // display.md S9 (owner, 2026-10-29): `9%` centred, `10%` re-centred, and back.
    #[test]
    fn the_s9_scenario_of_mode_1_reads_as_written() {
        let scenarios = mode_scenarios(1);
        let s9 = scenarios
            .iter()
            .find(|s| s.heading == "## S9 · mode 1")
            .expect("S9 in mode 1");
        assert_eq!((s9.id.as_str(), s9.mode, s9.ticks.len()), ("S9", 1, 6));
        let cpu = |value: i64| Change::Reading {
            metric: "CPU Usage".to_owned(),
            value: Some(Exact::whole(value)),
        };
        assert_eq!(s9.ticks[0].changes[0], Change::Reset(Reset::StartUp));
        assert_eq!(s9.ticks[0].changes[1], cpu(9));
        assert_eq!(s9.ticks[2].changes, [cpu(10)]);
        let nine = s9.ticks[0].frame.as_ref().expect("a frame");
        let ten = s9.ticks[2].frame.as_ref().expect("a frame");
        // `9%` takes columns 4–12, `10%` columns 2–14: the 1's foot starts at column 2.
        assert_eq!((nine.get(6, 4), nine.get(10, 2)), (Pixel::On, Pixel::Off));
        assert_eq!(ten.get(10, 2), Pixel::On);
        assert_eq!(s9.ticks[4].frame, s9.ticks[0].frame);
    }

    fn scenario_file(ticks: &str) -> String {
        format!(
            "# Test scenarios\n\n## S1 · mode 1\n\n{ticks}\n# Frames\n\n### f1\n\n{}",
            BLANK_ROWS.repeat(14)
        )
    }

    const START: &str = "0: reset start-up; CPU Usage = 100 → f1\n";

    #[test]
    fn a_scenario_file_reads_its_ticks_and_frames() {
        let text = scenario_file(&format!(
            "{START}1: missed → no frame\n2: reset rotation +2 → f1\n"
        ));
        let scenarios = parse_scenarios("test.txt", &text);
        let ticks = &scenarios[0].ticks;
        assert_eq!(ticks.len(), 3);
        assert_eq!(ticks[1].frame, None);
        assert_eq!(ticks[2].changes, [Change::Reset(Reset::Rotation(2))]);
        assert_eq!(ticks[2].frame, ticks[0].frame);
    }

    #[test]
    #[should_panic(expected = "test.txt:6: \"reset rotation +4\" isn't a change")]
    fn an_unknown_change_is_refused() {
        parse_scenarios(
            "test.txt",
            &scenario_file(&format!("{START}1: reset rotation +4 → f1\n")),
        );
    }

    #[test]
    #[should_panic(expected = "test.txt:6: f2 isn't defined under `# Frames`")]
    fn a_missing_frame_is_refused() {
        parse_scenarios("test.txt", &scenario_file(&format!("{START}1: - → f2\n")));
    }

    #[test]
    #[should_panic(expected = "test.txt:6: a missed tick has no frame")]
    fn a_missed_tick_with_a_frame_is_refused() {
        parse_scenarios(
            "test.txt",
            &scenario_file(&format!("{START}1: missed → f1\n")),
        );
    }

    #[test]
    #[should_panic(expected = "test.txt:6: tick 2 comes after tick 0")]
    fn a_skipped_tick_number_is_refused() {
        parse_scenarios("test.txt", &scenario_file(&format!("{START}2: - → f1\n")));
    }

    #[test]
    #[should_panic(expected = "test.txt:3: a scenario starts with tick 0 and `reset start-up`")]
    fn a_scenario_without_a_start_is_refused() {
        parse_scenarios("test.txt", &scenario_file("0: CPU Usage = 100 → f1\n"));
    }

    #[test]
    #[should_panic(expected = "test.txt: f2 is defined but no tick names it")]
    fn an_unused_frame_is_refused() {
        let text = format!(
            "{}\n### f2\n\n{}",
            scenario_file(START),
            BLANK_ROWS.repeat(14)
        );
        parse_scenarios("test.txt", &text);
    }

    #[test]
    fn mode_1_reads_as_written() {
        let frames = load_mode(1);
        assert_eq!(frames.title, "Load · CPU");
        let first = &frames.cases[0];
        assert_eq!(first.heading, "## rotation 0, CPU Usage = 3");
        assert_eq!(first.rotation, Rotation::default());
        assert_eq!(first.unit, None);
        assert_eq!(
            first.readings,
            [("CPU Usage".to_owned(), Some(Exact::whole(3)))]
        );
        let Kind::Static(picture) = &first.kind else {
            panic!("3% is static");
        };
        // display.md: the digit 3's top row, centred, and the % sign.
        assert_eq!(picture.get(6, 4), Pixel::On);
        assert_eq!(picture.get(6, 7), Pixel::Off);
        assert_eq!(picture.get(1, 1), Pixel::Off);
        assert!(matches!(&frames.cases[2].kind, Kind::Scrolling(p) if p.len() == 21));
    }

    #[test]
    fn units_and_loops_read_as_written() {
        let temperature = load_mode(7);
        assert_eq!(temperature.cases[0].unit, Some(Unit::Celsius));
        assert!(
            temperature
                .cases
                .iter()
                .any(|c| c.unit == Some(Unit::Fahrenheit))
        );
        let emoji = load_mode(14);
        let Kind::Loop(steps) = &emoji.cases[0].kind else {
            panic!("an emoji loop");
        };
        let durations: Vec<u64> = steps.iter().map(|(d, _)| d.as_millis() as u64).collect();
        assert_eq!(durations, [400, 100, 100, 100, 100, 100]);
        assert!(steps[0].1.has_any());
    }

    const BLANK_ROWS: &str = "..............\n";

    fn file(head: &str, rows: &str) -> String {
        format!("# Mode 1: Test\n\n{head}\n\n{rows}")
    }

    #[test]
    #[should_panic(expected = "test.txt:1: expected \"# Mode 1: \"")]
    fn a_wrong_title_is_refused() {
        parse("test.txt", 1, "# Mode 2: Test\n");
    }

    #[test]
    #[should_panic(expected = "test.txt:3: the heading doesn't start with `rotation 0` to `3`")]
    fn rotation_4_is_refused() {
        parse(
            "test.txt",
            1,
            &file("## rotation 4", &BLANK_ROWS.repeat(14)),
        );
    }

    #[test]
    #[should_panic(expected = "test.txt:3: \"CPU Usage: 3\" is neither a unit nor `Name = value`")]
    fn a_malformed_reading_is_refused() {
        parse(
            "test.txt",
            1,
            &file("## rotation 0, CPU Usage: 3", &BLANK_ROWS.repeat(14)),
        );
    }

    #[test]
    #[should_panic(expected = "test.txt:7: 'x' isn't '#', '.' or '?'")]
    fn a_foreign_pixel_is_refused() {
        let rows = format!(
            "{}......x.......\n{}",
            BLANK_ROWS.repeat(2),
            BLANK_ROWS.repeat(11)
        );
        parse("test.txt", 1, &file("## rotation 0", &rows));
    }

    #[test]
    #[should_panic(expected = "isn't 14 pixels wide")]
    fn a_short_row_is_refused() {
        let rows = format!(".............\n{}", BLANK_ROWS.repeat(13));
        parse("test.txt", 1, &file("## rotation 0", &rows));
    }

    #[test]
    #[should_panic(expected = "test.txt:17: the file ends after 13 of 14 picture rows")]
    fn a_short_block_is_refused() {
        parse(
            "test.txt",
            1,
            &file("## rotation 0", &BLANK_ROWS.repeat(13)),
        );
    }

    #[test]
    #[should_panic(expected = "test.txt:3: a static case has one picture, not 2")]
    fn two_pictures_without_a_note_are_refused() {
        let rows = "..............  ..............\n".repeat(14);
        parse("test.txt", 1, &file("## rotation 0", &rows));
    }

    #[test]
    #[should_panic(expected = "test.txt:3: a cycle of 21 frames (at least 21) has 1 pictures")]
    fn a_cycle_with_missing_frames_is_refused() {
        let rows = format!(
            "21 frames in time order, one per 100 ms tick\n\n{}",
            BLANK_ROWS.repeat(14)
        );
        parse("test.txt", 1, &file("## rotation 0", &rows));
    }

    #[test]
    #[should_panic(expected = "test.txt:3: a loop of 2 durations has 1 pictures")]
    fn a_loop_with_a_missing_picture_is_refused() {
        let rows = format!("{LOOP_NOTE}400 ms, 100 ms\n\n{}", BLANK_ROWS.repeat(14));
        parse("test.txt", 1, &file("## rotation 0", &rows));
    }

    #[test]
    #[should_panic(expected = "test.txt:5: a duration isn't `<n> ms`")]
    fn a_malformed_duration_is_refused() {
        let rows = format!("{LOOP_NOTE}400 ms, 0.1 s\n\n{}", BLANK_ROWS.repeat(14));
        parse("test.txt", 1, &file("## rotation 0", &rows));
    }

    #[test]
    #[should_panic(expected = "test.txt:1: the file has no case")]
    fn a_file_without_cases_is_refused() {
        parse("test.txt", 1, "# Mode 1: Test\n\n");
    }

    #[test]
    fn spectrum_pixels_match_either_value() {
        let mut rows = [".............."; SIZE];
        rows[0] = "?.............";
        let picture = Picture::from_rows(&rows);
        let mut frame = Frame::blank();
        assert!(picture.matches(&frame));
        frame.set(1, 1, true);
        assert!(picture.matches(&frame));
        frame.set(1, 2, true);
        assert!(!picture.matches(&frame));
        assert_eq!(picture.to_frame(), None);
    }
}
