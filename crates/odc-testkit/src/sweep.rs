//! The research sweeps of 2026-10-22 (`research/logs/`, local and git-ignored; Phase 3): each
//! step's mode, readings, unit, rotation and steady window, and the frames DeepCreative sent, by
//! their time, from `lp360-hidraw.log`. Only the local layer 2 tests read them (PRD, Testing
//! Strategy); they hold no DeepCreative code (AGENTS.md rule 6).

use std::fs;
use std::path::{Path, PathBuf};

use odc_protocol::{Frame, Message, Rotation, decode};
use odc_sensors::json::{self, Json};

use crate::capture;

/// `research/logs/` in the repository.
pub fn logs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../research/logs")
}

/// One sweep step.
#[derive(Clone, Debug)]
pub struct Step {
    /// The sweep file, for messages.
    pub file: String,
    pub label: String,
    /// `Load`, `Temperature`, `Transition` …, as the sweep named it.
    pub family: String,
    pub mode: u16,
    /// Each reading the step fed DeepCreative, its JSON number kept as text, never parsed as a
    /// float.
    pub readings: Vec<(String, String)>,
    pub fahrenheit: bool,
    pub rotation: Rotation,
    /// The steady window, in ms since the Unix epoch, both ends included.
    pub start_ms: u64,
    pub end_ms: u64,
}

/// Every step of every `sweep-*.json`, file by file in name order.
///
/// # Errors
///
/// When there's no sweep (on a computer without the research files), or one doesn't read.
pub fn steps() -> Result<Vec<Step>, String> {
    let dir = logs();
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("sweep-") && n.ends_with(".json"))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("{}: no sweep-*.json", dir.display()));
    }
    let mut steps = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        let sweep = json::parse(&text).map_err(|e| format!("{name}: {e}"))?;
        let Some(Json::Array(list)) = member(&sweep, "steps") else {
            return Err(format!("{name}: no `steps` list"));
        };
        for (i, step) in list.iter().enumerate() {
            steps.push(read_step(&name, step).map_err(|e| format!("{name}, step {}: {e}", i + 1))?);
        }
    }
    Ok(steps)
}

fn member<'a>(value: &'a Json, key: &str) -> Option<&'a Json> {
    match value {
        Json::Object(members) => members.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn text(value: &Json, key: &str) -> Result<String, String> {
    match member(value, key) {
        Some(Json::String(s)) => Ok(s.clone()),
        _ => Err(format!("no string `{key}`")),
    }
}

fn whole(value: &Json, key: &str) -> Result<Option<u64>, String> {
    match member(value, key) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::Number(n)) => n
            .parse()
            .map(Some)
            .map_err(|_| format!("`{key}` {n} isn't a whole number")),
        Some(_) => Err(format!("`{key}` isn't a number")),
    }
}

fn read_step(file: &str, step: &Json) -> Result<Step, String> {
    let required = |key: &str| whole(step, key)?.ok_or_else(|| format!("no `{key}`"));
    let Some(Json::Object(values)) = member(step, "values") else {
        return Err("no `values`".into());
    };
    let readings = values
        .iter()
        .map(|(name, value)| match value {
            Json::Number(n) => Ok((name.clone(), n.clone())),
            _ => Err(format!("`{name}` isn't a number")),
        })
        .collect::<Result<Vec<_>, String>>()?;
    let rotation = whole(step, "rot")?.unwrap_or(0);
    Ok(Step {
        file: file.to_owned(),
        label: text(step, "label")?,
        family: text(step, "family")?,
        mode: u16::try_from(required("mode")?).map_err(|_| "a mode past 65535")?,
        readings,
        fahrenheit: whole(step, "unit")?.unwrap_or(0) == 1,
        rotation: u8::try_from(rotation)
            .ok()
            .and_then(Rotation::from_setting)
            .ok_or_else(|| format!("rotation {rotation}"))?,
        start_ms: required("start")?,
        end_ms: required("end")?,
    })
}

/// The frames of the capture log, in order, each with its time in ms since the Unix epoch.
pub struct Sent {
    frames: Vec<(u64, Frame)>,
}

impl Sent {
    /// The frames sent from `start_ms` to `end_ms`, both included.
    pub fn within(&self, start_ms: u64, end_ms: u64) -> impl Iterator<Item = &Frame> {
        self.frames
            .iter()
            .filter(move |(t, _)| (start_ms..=end_ms).contains(t))
            .map(|(_, frame)| frame)
    }
}

/// Reads `research/logs/lp360-hidraw.log`, keeping the frames written to the cooler.
///
/// # Errors
///
/// When the log is missing or a message written to the cooler isn't one the spec documents.
pub fn sent() -> Result<Sent, String> {
    let path = logs().join("lp360-hidraw.log");
    if !path.is_file() {
        return Err(format!("{}: no capture log", path.display()));
    }
    let mut frames = Vec::new();
    for captured in capture::read(&path) {
        if captured.dir != capture::Direction::Out {
            continue;
        }
        let bytes: [u8; 64] = captured
            .bytes
            .as_slice()
            .try_into()
            .map_err(|_| format!("{}:{}: not 64 bytes", path.display(), captured.line))?;
        match decode(&bytes) {
            Ok(Message::Frame(frame)) => frames.push((captured.unix_ns / 1_000_000, frame)),
            Ok(Message::Startup) => {}
            Err(e) => return Err(format!("{}:{}: {e:?}", path.display(), captured.line)),
        }
    }
    Ok(Sent { frames })
}
