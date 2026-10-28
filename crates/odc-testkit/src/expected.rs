//! The expected frames of `docs/spec/frames/`, loaded into typed cases (`docs/spec/frames/README.md`,
//! Format). A file that doesn't parse panics, naming its file and line: a mis-read oracle must
//! never pass silently.

use std::fmt;
use std::fs;
use std::path::Path;
use std::time::Duration;

use odc_protocol::{Frame, Rotation};

const FRAMES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/spec/frames");
const SIZE: usize = Frame::SIZE;

/// One expected-frame file: every case of one mode.
#[derive(Debug)]
pub struct ModeFrames {
    pub title: String,
    pub cases: Vec<Case>,
}

/// One `##` section: a rotation, a unit and readings, and what the cooler must show for them.
#[derive(Debug)]
pub struct Case {
    /// The heading as written, for messages.
    pub heading: String,
    pub rotation: Rotation,
    /// °C or °F, when the heading names one.
    pub unit: Option<Unit>,
    /// The readings, with the metric names exactly as written (`CPU Usage`, `Gpu temperature`).
    pub readings: Vec<(String, u32)>,
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
        cases.push(parse_case(&mut lines, n, heading));
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

fn parse_case(lines: &mut Lines<'_>, n: usize, heading: &str) -> Case {
    let Some(rest) = heading.strip_prefix("## ") else {
        lines.fail(n, format!("expected a `##` heading, found {heading:?}"));
    };
    let mut parts = rest.split(", ");
    let rotation = parts
        .next()
        .and_then(|p| p.strip_prefix("rotation "))
        .and_then(|r| r.parse::<u8>().ok())
        .and_then(Rotation::from_setting)
        .unwrap_or_else(|| lines.fail(n, "the heading doesn't start with `rotation 0` to `3`"));
    let mut unit = None;
    let mut readings = Vec::new();
    for part in parts {
        match part {
            "°C" if unit.is_none() && readings.is_empty() => unit = Some(Unit::Celsius),
            "°F" if unit.is_none() && readings.is_empty() => unit = Some(Unit::Fahrenheit),
            _ => match part.split_once(" = ").map(|(k, v)| (k, v.parse::<u32>())) {
                Some((name, Ok(value))) if !name.is_empty() => {
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
    Case {
        heading: heading.to_owned(),
        rotation,
        unit,
        readings,
        kind,
    }
}

/// Reads the picture blocks up to the next heading: 14 rows each, several pictures to a row
/// separated by two spaces.
fn parse_pictures(lines: &mut Lines<'_>) -> Vec<Picture> {
    let mut pictures = Vec::new();
    loop {
        lines.skip_blank();
        match lines.peek() {
            None => break,
            Some(line) if line.starts_with("## ") => break,
            Some(_) => {}
        }
        let mut block: Vec<Picture> = Vec::new();
        for r in 0..SIZE {
            let (n, line) = match lines.next() {
                Some((n, line)) if !line.is_empty() && !line.starts_with("## ") => (n, line),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file loads into typed cases, as many as it has `##` headings (284 when Phase 5
    /// runs); cycles have at least 21 pictures, and `?` appears only in Music (modes 13–22).
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
            cases += frames.cases.len();
        }
        assert_eq!(cases, headings);
        assert_eq!(cases, 284);
    }

    #[test]
    fn mode_1_reads_as_written() {
        let frames = load_mode(1);
        assert_eq!(frames.title, "Load · CPU");
        let first = &frames.cases[0];
        assert_eq!(first.heading, "## rotation 0, CPU Usage = 3");
        assert_eq!(first.rotation, Rotation::default());
        assert_eq!(first.unit, None);
        assert_eq!(first.readings, [("CPU Usage".to_owned(), 3)]);
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
