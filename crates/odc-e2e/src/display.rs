//! The display checks, `--only display` (Phase 7 plan, Task 3, as `--only load`; owner,
//! 2026-10-29: AGENTS.md rule 4 asks for the run on the cooler in each mode's commit; owner,
//! 2026-11-05: every drawn mode). Each listed mode's examples, drawn by the code just built, go
//! straight to the cooler at the tick, without the service: value set 2 upright and turned once,
//! value set 3, in each unit a temperature mode has, and the mode's dash cases; a Music mode
//! without values each case upright and its first turned once; a Music spectrum in silence.
//! Where no example scrolls, scenario S1 of the mode's `scenarios/mode-NNN.txt` brings one. Every
//! report written is recorded and compared with the expected frames; nobody looks at the cooler.
//! Without a cooler, the same runs through a fake one.

use std::path::Path;
use std::time::Duration;

use odc_device::{Connector, DisplayPort, Link, Lp360, Recording};
use odc_protocol::{Frame, Message};
use odc_render::{
    COLUMNS, Column, EmojiLoop, Marquee, Mode, Pose, Unit, Values, mode_info, picture,
    spectrum_height, with_spectrum,
};
use odc_testkit::diff;
use odc_testkit::expected::{
    self, Case, Change, Kind, Picture, every_metric, load_mode, load_unavailable, mode_scenarios,
};
use odc_testkit::hardware::{Capability, Mode as RunMode, Probe, Roots, Summary, probe};

use crate::device::{Pace, fake_lp360, hidraw, read_recording, recording_path};
use crate::session::{handed_back, release_for_direct};

/// Runs the examples of each of `modes`, which the flags checked are drawn by this build.
pub fn run(roots: &Roots, out_dir: &Path, tick: Duration, modes: &[u16], summary: &mut Summary) {
    for &n in modes {
        let mode = Mode::new(n).expect("a mode the flags checked");
        let result = check(mode, roots, out_dir, tick, summary);
        summary.check(&format!("display_mode_{n}"), result);
    }
}

/// The renderer's unit for a case's; a case that names none is in °C.
pub(crate) fn unit(unit: Option<expected::Unit>) -> Unit {
    match unit {
        Some(expected::Unit::Fahrenheit) => Unit::Fahrenheit,
        Some(expected::Unit::Celsius) | None => Unit::Celsius,
    }
}

fn check(
    mode: Mode,
    roots: &Roots,
    out_dir: &Path,
    tick: Duration,
    summary: &mut Summary,
) -> Result<String, String> {
    let examples = examples(mode)?;
    match probe(Capability::Lp360, roots) {
        Probe::Real => {
            summary.mode(Capability::Lp360, RunMode::Real);
            let session = release_for_direct(roots, summary)?;
            handed_back(session, show(hidraw(roots), mode, &examples, out_dir, tick))
        }
        Probe::Absent { reason } => {
            summary.mode(Capability::Lp360, RunMode::Fake { reason });
            let tree = fake_lp360(out_dir, &format!("fake-display-{}", mode.number()))?;
            show(hidraw(&tree.roots()), mode, &examples, out_dir, tick)
        }
        Probe::Unusable { reason, fix } => {
            summary.mode(
                Capability::Lp360,
                RunMode::Failed {
                    reason: reason.clone(),
                },
            );
            Err(format!("{reason}; fix: {fix}"))
        }
    }
}

/// Every example of `mode` against its expected frames in process, without a cooler; how many
/// frames matched. The tests check a Music mode's examples so before its commit makes it drawn.
///
/// # Errors
///
/// The first example frame that differs.
pub fn examples_match(mode: Mode) -> Result<usize, String> {
    let mut frames = 0;
    for example in examples(mode)? {
        for (i, (frame, picture)) in example.frames.iter().zip(&example.expected).enumerate() {
            if let Some(why) = diff::describe(picture, frame) {
                return Err(format!("{}, frame {}: {why}", example.name, i + 1));
            }
            frames += 1;
        }
    }
    Ok(frames)
}

/// One example: the frames the renderer draws for it, already turned, and those expected.
struct Example {
    name: String,
    frames: Vec<Frame>,
    expected: Vec<Picture>,
}

/// Silence: every bar 1 row high, the floor line.
const FLOOR: [Column; COLUMNS] = [Column { bar: 1, dot: None }; COLUMNS];

/// `frame` with the spectrum area of `mode` in silence: the floor line, when the mode has one.
fn in_silence(mode: Mode, frame: Frame) -> Frame {
    match spectrum_height(mode) {
        Some(h) => with_spectrum(&frame, &FLOOR, h),
        None => frame,
    }
}

/// The mode's examples, one of each test type, in each unit a temperature mode has (owner,
/// 2026-10-26: the full cross runs in process). A Music mode's spectrum is in silence (Phase 9
/// plan, Task 5).
fn examples(mode: Mode) -> Result<Vec<Example>, String> {
    let n = mode.number();
    if mode_info(mode).metrics.is_empty() {
        return music_examples(mode);
    }
    let file = load_mode(n);
    // The value sets in file order: 1, 2 and 3 (docs/spec/frames/README.md, Readings used).
    let set = |rotation: u8, unit: Option<expected::Unit>, index: usize| {
        file.cases
            .iter()
            .filter(|c| c.rotation.setting() == rotation && c.unit == unit)
            .nth(index)
            .ok_or_else(|| {
                format!(
                    "mode-{n:03}.txt: no value set {} in rotation {rotation}, {unit:?}",
                    index + 1
                )
            })
    };
    let mut units: Vec<Option<expected::Unit>> = Vec::new();
    for case in &file.cases {
        if !units.contains(&case.unit) {
            units.push(case.unit);
        }
    }
    let mut cases = Vec::new();
    for &unit in &units {
        cases.extend([set(0, unit, 1)?, set(1, unit, 1)?, set(0, unit, 2)?]);
    }
    let dashes = load_unavailable();
    cases.extend(
        dashes
            .iter()
            .filter(|(m, c)| *m == n && c.rotation.setting() == 0)
            .map(|(_, case)| case),
    );
    let scrolls = cases.iter().any(|c| matches!(c.kind, Kind::Scrolling(_)));
    let mut examples: Vec<Example> = cases
        .into_iter()
        .map(|case| draw(mode, case))
        .collect::<Result<_, _>>()?;
    if !scrolls {
        examples.push(scrolling(mode)?);
    }
    Ok(examples)
}

/// The mode's scenario S1, a value of 100 or more scrolling from home, as the oracle drew it: in a
/// Music mode over the spectrum's floor line, as its scenarios start in silence.
fn scrolling(mode: Mode) -> Result<Example, String> {
    let n = mode.number();
    let scenarios = mode_scenarios(n);
    let s1 = scenarios
        .iter()
        .find(|s| s.id == "S1")
        .ok_or_else(|| format!("scenarios/mode-{n:03}.txt: no S1"))?;
    let start: Vec<_> = s1
        .ticks
        .first()
        .map(|t| t.changes.as_slice())
        .unwrap_or_default()
        .iter()
        .filter_map(|c| match c {
            Change::Reading { metric, value } => Some((metric.clone(), *value)),
            _ => None,
        })
        .collect();
    let mut values = Values::new();
    for (metric, value) in every_metric(&start) {
        values.set(metric, value);
    }
    let p =
        picture(mode, &values, unit(Some(s1.unit))).map_err(|_| format!("mode {n} isn't drawn"))?;
    let mut marquee = Marquee::new(p);
    let (mut frames, mut expected) = (Vec::new(), Vec::new());
    for tick in &s1.ticks {
        if tick.n > 0 && !tick.changes.is_empty() {
            return Err(format!("{}: a change after tick 0", s1.heading));
        }
        let picture = tick
            .frame
            .clone()
            .ok_or_else(|| format!("{}: a missed tick", s1.heading))?;
        frames.push(in_silence(mode, marquee.tick()));
        expected.push(picture);
    }
    Ok(Example {
        name: s1.heading.trim_start_matches("## ").to_owned(),
        frames,
        expected,
    })
}

fn draw(mode: Mode, case: &Case) -> Result<Example, String> {
    let mut values = Values::new();
    for (metric, value) in every_metric(&case.readings) {
        values.set(metric, value);
    }
    let p = picture(mode, &values, unit(case.unit))
        .map_err(|_| format!("mode {} isn't drawn", mode.number()))?;
    let mut marquee = Marquee::new(p);
    let expected: Vec<Picture> = match &case.kind {
        Kind::Static(picture) => vec![picture.in_silence(); 3],
        Kind::Scrolling(cycle) => cycle.iter().map(Picture::in_silence).collect(),
        Kind::Loop(_) => return Err(format!("{}: a loop, not a value", case.heading)),
    };
    let frames = expected
        .iter()
        .map(|_| in_silence(mode, marquee.tick()).rotated(case.rotation))
        .collect();
    Ok(Example {
        name: case.heading.trim_start_matches("## ").to_owned(),
        frames,
        expected,
    })
}

/// A Music mode without values (Phase 9 plan, Task 5): each case upright and its first case turned
/// once, in silence. Mode 13 is the floor line, an emoji its whole loop from its first picture
/// and that again, and mode 23 each of the Dancer's poses as the renderer draws it.
fn music_examples(mode: Mode) -> Result<Vec<Example>, String> {
    let file = load_mode(mode.number());
    let cases = file.cases.iter().filter(|c| {
        c.rotation.setting() == 0 || (c.rotation.setting() == 1 && c.pose.is_none_or(|p| p == 0))
    });
    cases.map(|case| draw_music(mode, case)).collect()
}

fn draw_music(mode: Mode, case: &Case) -> Result<Example, String> {
    let turned = |frame: Frame| frame.rotated(case.rotation);
    let (frames, expected) = match &case.kind {
        Kind::Static(picture) => {
            let frame = match case.pose {
                Some(k) => Pose::new(k)
                    .ok_or_else(|| format!("{}: no pose {k}", case.heading))?
                    .frame(),
                None => in_silence(mode, Frame::blank()),
            };
            (vec![turned(frame); 3], vec![picture.in_silence(); 3])
        }
        Kind::Loop(steps) => {
            let mut expected: Vec<Picture> = steps
                .iter()
                .flat_map(|(duration, picture)| {
                    let n = usize::try_from(duration.as_millis() / 100).expect("a short duration");
                    std::iter::repeat_n(picture.in_silence(), n)
                })
                .collect();
            expected.extend(expected.first().cloned());
            let mut emoji = EmojiLoop::of(mode)
                .ok_or_else(|| format!("{}: mode {} has no loop", case.heading, mode.number()))?;
            let frames = expected
                .iter()
                .map(|_| turned(in_silence(mode, emoji.tick())))
                .collect();
            (frames, expected)
        }
        Kind::Scrolling(_) => {
            return Err(format!("{}: a value, in a mode without one", case.heading));
        }
    };
    Ok(Example {
        name: case.heading.trim_start_matches("## ").to_owned(),
        frames,
        expected,
    })
}

/// Writes every example's frames to the cooler at the tick, then compares the recording.
fn show<C: Connector>(
    connector: C,
    mode: Mode,
    examples: &[Example],
    out_dir: &Path,
    tick: Duration,
) -> Result<String, String> {
    let path = recording_path(out_dir, &format!("display-mode-{}", mode.number()))?;
    let recording =
        Recording::create(connector, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lp360 = Lp360::new(recording);
    let mut pace = Pace::new(tick);
    for example in examples {
        for (i, frame) in example.frames.iter().enumerate() {
            pace.wait();
            let link = lp360.show(frame);
            if link != Link::Connected {
                return Err(format!("{}, frame {}: {link:?}", example.name, i + 1));
            }
        }
    }
    if let Some(failure) = lp360.connector().failure() {
        return Err(failure);
    }
    let messages = read_recording(&path)?;
    let expected: Vec<(&str, &Picture)> = examples
        .iter()
        .flat_map(|e| e.expected.iter().map(move |p| (e.name.as_str(), p)))
        .collect();
    let startups = messages.iter().filter(|m| **m == Message::Startup).count();
    if messages.first() != Some(&Message::Startup)
        || startups != 1
        || messages.len() != expected.len() + 1
    {
        return Err(format!(
            "{}: {} messages, {startups} start-ups; expected 03 08 then {} frames",
            path.display(),
            messages.len(),
            expected.len()
        ));
    }
    for (i, (message, (name, picture))) in messages[1..].iter().zip(&expected).enumerate() {
        let Message::Frame(frame) = message else {
            return Err(format!(
                "{}: report {} isn't a frame",
                path.display(),
                i + 2
            ));
        };
        if let Some(why) = diff::describe(picture, frame) {
            return Err(format!("{name}, frame {}: {why}", i + 1));
        }
    }
    let names: Vec<&str> = examples.iter().map(|e| e.name.as_str()).collect();
    Ok(format!(
        "03 08, then {} frames as expected: {}",
        expected.len(),
        names.join("; ")
    ))
}
