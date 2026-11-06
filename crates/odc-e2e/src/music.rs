//! The Music checks, `--only music` (Phase 9 plan, Task 5): the session's helper capturing only in
//! a Music mode, the real capture of a test tone through a virtual output, and the delay from a
//! sound to the frame that shows it. The tone plays into a null sink of the user's PipeWire,
//! `odc-e2e`, which the dev helper captures in place of the owner's helper; the owner's own sound
//! keeps playing on their output. Everything the run changes is set back however it ends, and a
//! run killed before that is repaired at the next start (solution review P9-10). Without PipeWire
//! or the dev service, the capture runs through the engine in process, and the delay isn't run.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use odc_device::monotonic_ns;
use odc_protocol::{Frame, Message};
use odc_render::{Mode as RenderMode, WINDOW, draws};
use odc_service::audio::{AudioSource, FakeAudio};
use odc_service::engine::{Drawn, Engine};
use odc_service::settings::Settings;
use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};
use odc_testkit::spectrum::{Rows, expected, signal};

use crate::plays::{FLUSHED_WITHIN, message, recording_lines};
use crate::service::{Restore, state};
use crate::session::Session;

/// The virtual output's name: a null sink of the user's PipeWire. It's a single node with that
/// name, whose monitor `pw-record --target odc-e2e` captures with `stream.capture.sink` (read on
/// the development PC, 2026-11-05).
pub const SINK: &str = "odc-e2e";

/// The session's helper, its unit and its dev build (Phase 9 plan, Task 4).
pub const HELPER_UNIT: &str = "opendeepcool-audio.service";
pub const HELPER_NODE: &str = "opendeepcool-audio";
pub const DEV_HELPER: &str = "/usr/local/libexec/opendeepcool-dev/opendeepcool-audio";

/// The test tone: column 7's centre at −20 dBFS, as `spectrum/signals.txt` has it.
const TONE: &str = "column 7 centre, -20 dBFS";
const TONE_HZ: f64 = 645.891_059;
const TONE_AMPLITUDE: f64 = 0.1;

/// The delay target (owner, 2026-10-25; `music.md` point 9): at most 150 ms for 95 % of frames.
const DELAY_TARGET_MS: f64 = 150.0;
/// 200 bursts of 100 ms, 405 ms apart (Phase 9 plan, Task 5): each starts 5 ms later in the
/// service's 100 ms tick than the one before, at 20 points of it 10 times each, so the delays show
/// their whole spread, not one point of the tick (owner, 2026-11-06). A burst starts on a 5 ms
/// chunk, whose write time is its stamp.
const BURSTS: usize = 200;
const BURST: usize = 4800;
const BURST_EVERY: usize = 19_440;

/// Samples per second, and per 5 ms chunk written to `pw-play`.
const RATE: usize = 48_000;
const CHUNK: usize = 240;

// A burst starts on a chunk, so it has a stamp, and never at the same point of every 100 ms tick.
const _: () = assert!(BURST_EVERY.is_multiple_of(CHUNK) && !BURST_EVERY.is_multiple_of(RATE / 10));

/// How long a change on the bus or in PipeWire may take.
const WITHIN: Duration = Duration::from_secs(1);
const CONNECT_WITHIN: Duration = Duration::from_secs(3);

/// The tools the checks start, from `<bin>`: the system's, or a test's fakes.
pub struct Tools {
    bin: PathBuf,
}

impl Tools {
    pub fn new(roots: &Roots) -> Tools {
        Tools {
            bin: roots.bin.clone(),
        }
    }

    fn command(&self, program: &str) -> Command {
        Command::new(self.bin.join(program))
    }

    /// The program's standard output; a failure says the command and its status.
    pub fn output(&self, program: &str, args: &[&str]) -> Result<String, String> {
        let out = self
            .command(program)
            .args(args)
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(format!("{program} {}: {}", args.join(" "), out.status))
        }
    }

    /// Whether the program exits with status 0, as `systemctl is-active` answers.
    pub fn succeeds(&self, program: &str, args: &[&str]) -> Result<bool, String> {
        self.command(program)
            .args(args)
            .output()
            .map(|out| out.status.success())
            .map_err(|e| format!("{program}: {e}"))
    }
}

/// `pactl load-module`'s output: the module's index (pactl(1)).
pub fn module_index(output: &str) -> Option<u32> {
    output.trim().parse().ok()
}

/// The modules of `pactl list short modules` that are this run's null sink, a killed run's
/// leftovers: index, `module-null-sink`, then its arguments, tab-separated.
pub fn leftover_sinks(list: &str) -> Vec<u32> {
    list.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let index = fields.next()?.parse().ok()?;
            (fields.next()? == "module-null-sink"
                && fields
                    .next()?
                    .split_whitespace()
                    .any(|arg| arg == format!("sink_name={SINK}")))
            .then_some(index)
        })
        .collect()
}

/// Whether `pw-dump`'s objects hold a node called `name`.
pub fn has_node(dump: &str, name: &str) -> bool {
    dump.contains(&format!("\"node.name\": \"{name}\""))
}

/// Whether the helper's capture stream, in `pactl list source-outputs`, is at 100 % on every
/// channel: a volume WirePlumber restored for it would scale every level. `None` while the stream
/// isn't listed, or its volume isn't known yet: the service says the sound is connected as soon as
/// the helper attaches, before `pw-record` has joined PipeWire (seen on the development PC,
/// 2026-11-05).
pub fn capture_at_full_volume(list: &str) -> Option<bool> {
    let block = list
        .split("Source Output #")
        .find(|block| block.contains(&format!("node.name = \"{HELPER_NODE}\"")))?;
    let volume = block
        .lines()
        .find_map(|line| line.trim().strip_prefix("Volume:"))?;
    let percents: Vec<&str> = volume
        .split('/')
        .map(str::trim)
        .filter(|part| part.ends_with('%'))
        .collect();
    if percents.is_empty() {
        return None;
    }
    Some(percents.iter().all(|p| *p == "100%"))
}

/// Before the checks: a null sink a killed run left is unloaded, and the owner's helper, which a
/// killed run left stopped, is started again (solution review P9-10). Returns what was repaired.
///
/// # Errors
///
/// When a tool can't run or refuses.
pub fn repair(tools: &Tools) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    for index in leftover_sinks(&tools.output("pactl", &["list", "short", "modules"])?) {
        tools.output("pactl", &["unload-module", &index.to_string()])?;
        notes.push(format!(
            "unloaded the null sink {SINK} a killed run left (module {index})"
        ));
    }
    let loaded = tools
        .output(
            "systemctl",
            &[
                "--user",
                "show",
                "--property=LoadState",
                "--value",
                HELPER_UNIT,
            ],
        )?
        .trim()
        == "loaded";
    if loaded
        && !tools.succeeds(
            "systemctl",
            &["--user", "--quiet", "is-active", HELPER_UNIT],
        )?
    {
        tools.output("systemctl", &["--user", "start", HELPER_UNIT])?;
        notes.push(format!(
            "started {HELPER_UNIT}, which a killed run left stopped"
        ));
    }
    Ok(notes)
}

/// The virtual output and the dev helper capturing it, in place of the owner's helper; all set
/// back when dropped, a panic included.
pub struct Harness<'a> {
    tools: &'a Tools,
    module: u32,
    owners_helper: bool,
    helper: Option<Child>,
}

impl<'a> Harness<'a> {
    /// Loads the null sink, stops the owner's helper, and starts `helper --target odc-e2e`, its
    /// lines going to `log`.
    ///
    /// # Errors
    ///
    /// When a step fails; what was done before it is set back.
    pub fn start(tools: &'a Tools, helper: &Path, log: &Path) -> Result<Harness<'a>, String> {
        let output = tools.output(
            "pactl",
            &[
                "load-module",
                "module-null-sink",
                &format!("sink_name={SINK}"),
            ],
        )?;
        let module = module_index(&output)
            .ok_or_else(|| format!("pactl load-module printed {output:?}, not an index"))?;
        let mut harness = Harness {
            tools,
            module,
            owners_helper: false,
            helper: None,
        };
        harness.owners_helper = tools.succeeds(
            "systemctl",
            &["--user", "--quiet", "is-active", HELPER_UNIT],
        )?;
        if harness.owners_helper {
            tools.output("systemctl", &["--user", "stop", HELPER_UNIT])?;
        }
        let lines = File::create(log).map_err(|e| format!("{}: {e}", log.display()))?;
        harness.helper = Some(
            Command::new(helper)
                .args(["--target", SINK])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(lines)
                .spawn()
                .map_err(|e| format!("{}: {e}", helper.display()))?,
        );
        Ok(harness)
    }
}

impl Drop for Harness<'_> {
    fn drop(&mut self) {
        if let Some(mut helper) = self.helper.take() {
            let _ = helper.kill();
            let _ = helper.wait();
        }
        let mut said = Vec::new();
        if let Err(e) = self
            .tools
            .output("pactl", &["unload-module", &self.module.to_string()])
        {
            said.push(e);
        }
        if self.owners_helper
            && let Err(e) = self
                .tools
                .output("systemctl", &["--user", "start", HELPER_UNIT])
        {
            said.push(e);
        }
        for line in said {
            eprintln!("odc-e2e: the sound wasn't all set back: {line}");
        }
    }
}

/// Runs the Music checks. They need mode 13, which Task 6 makes drawable.
pub fn run(roots: &Roots, out_dir: &Path, summary: &mut Summary) {
    let thirteen = RenderMode::new(13).expect("mode 13");
    if !draws(thirteen) {
        for name in ["music_capture", "music_delay"] {
            summary.skipped(name, "mode 13 isn't drawn by this build yet");
        }
        return;
    }
    match (
        probe(Capability::DevService, roots),
        probe(Capability::PipeWire, roots),
    ) {
        (Probe::Real, Probe::Real) => {
            summary.mode(Capability::DevService, Mode::Real);
            summary.mode(Capability::PipeWire, Mode::Real);
            real(roots, out_dir, summary);
        }
        (Probe::Unusable { reason, fix }, _) | (_, Probe::Unusable { reason, fix }) => {
            summary.check("music", Err(format!("{reason}; fix: {fix}")));
        }
        (service, pipewire) => {
            for (capability, found) in [
                (Capability::DevService, service),
                (Capability::PipeWire, pipewire),
            ] {
                let found = match found {
                    Probe::Real => Mode::Real,
                    Probe::Absent { reason } => Mode::Fake { reason },
                    Probe::Unusable { reason, .. } => Mode::Failed { reason },
                };
                summary.mode(capability, found);
            }
            summary.check("music_capture", fake_capture(draws));
            summary.skipped(
                "music_delay",
                "it needs the dev service and PipeWire on this PC",
            );
        }
    }
}

fn real(roots: &Roots, out_dir: &Path, summary: &mut Summary) {
    let tools = Tools::new(roots);
    match repair(&tools) {
        Ok(notes) => {
            for note in notes {
                println!("{note}");
                summary.check("music_repair", Ok(note));
            }
        }
        Err(e) => return summary.check("music_repair", Err(e)),
    }
    if let Err(e) = fs::create_dir_all(out_dir) {
        return summary.check("music", Err(format!("{}: {e}", out_dir.display())));
    }
    // Dropped last: the owner's settings come back first, which leaves Music and closes the pipe.
    let _harness = match Harness::start(&tools, Path::new(DEV_HELPER), &out_dir.join("helper.log"))
    {
        Ok(harness) => harness,
        Err(e) => return summary.check("music", Err(e)),
    };
    let session = match Session::begin() {
        Ok(session) => session,
        Err(e) => return summary.check("music", Err(e)),
    };
    let found = match state(&session) {
        Ok(state) => state,
        Err(e) => return summary.check("music", Err(e)),
    };
    let _restore = Restore {
        session: &session,
        state: found,
    };
    summary.check("music_only_in_music", only_in_music(&tools, &session));
    summary.check("music_capture", capture(&tools, &session));
    summary.check("music_delay", delay(&tools, &session));
}

fn set_mode(session: &Session, mode: u32) -> Result<(), String> {
    zbus::block_on(session.main().set_mode(mode)).map_err(|e| format!("SetMode({mode}): {e}"))
}

/// Waits until `check` holds, at most `limit`.
fn soon(limit: Duration, mut check: impl FnMut() -> Result<bool, String>) -> Result<bool, String> {
    let until = Instant::now() + limit;
    loop {
        if check()? {
            return Ok(true);
        }
        if Instant::now() >= until {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// In mode 1 no capture node exists; mode 13 makes one within a second, and mode 1 again removes
/// it within a second (owner, 2026-10-26: sound only in the Music modes).
fn only_in_music(tools: &Tools, session: &Session) -> Result<String, String> {
    let node =
        || -> Result<bool, String> { Ok(has_node(&tools.output("pw-dump", &[])?, HELPER_NODE)) };
    set_mode(session, 1)?;
    if !soon(WITHIN, || node().map(|n| !n))? {
        return Err(format!("in mode 1 a {HELPER_NODE} node still exists"));
    }
    set_mode(session, 13)?;
    if !soon(WITHIN, node)? {
        return Err(format!(
            "mode 13 made no {HELPER_NODE} node within a second"
        ));
    }
    set_mode(session, 1)?;
    if !soon(WITHIN, || node().map(|n| !n))? {
        return Err(format!(
            "back in mode 1 the {HELPER_NODE} node stayed for more than a second"
        ));
    }
    Ok(format!(
        "no {HELPER_NODE} node in mode 1; mode 13 made one and mode 1 removed it, each within a \
         second"
    ))
}

/// A sample of the test tone, its phase from the start of what's played.
fn tone(n: usize) -> f32 {
    (TONE_AMPLITUDE * (2.0 * std::f64::consts::PI * TONE_HZ * n as f64 / RATE as f64).sin()) as f32
}

/// Plays `samples` into the virtual output through `pw-play`, 5 ms at a time on the monotonic
/// clock, so at most 10 ms is queued ahead; returns the `CLOCK_MONOTONIC` time each of `marks`,
/// sample indices at a chunk's start, was written. Writing ahead can only make a measured delay
/// longer than the true one.
fn play(tools: &Tools, samples: &[f32], marks: &[usize]) -> Result<Vec<u64>, String> {
    let mut player = tools
        .command("pw-play")
        .args([
            "--raw",
            "--format",
            "f32",
            "--rate",
            "48000",
            "--channels",
            "2",
            "--latency",
            "10ms",
            // WirePlumber would give the stream the volume it keeps for pw-play (91 % on the
            // development PC, 2026-11-05).
            "-P",
            r#"{ "state.restore-props": false }"#,
            "--target",
            SINK,
            "-",
        ])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("pw-play: {e}"))?;
    let mut input = player.stdin.take().expect("a piped stdin");
    let mut stamps = vec![0; marks.len()];
    let start = Instant::now();
    for (k, chunk) in samples.chunks(CHUNK).enumerate() {
        let due = start + Duration::from_micros(5000 * k as u64);
        thread::sleep(due.saturating_duration_since(Instant::now()));
        let at = k * CHUNK;
        let now = monotonic_ns();
        for (stamp, _) in stamps.iter_mut().zip(marks).filter(|(_, m)| **m == at) {
            *stamp = now;
        }
        let bytes: Vec<u8> = chunk
            .iter()
            .flat_map(|s| [s.to_ne_bytes(), s.to_ne_bytes()].concat())
            .collect();
        input
            .write_all(&bytes)
            .map_err(|e| format!("pw-play: {e}"))?;
    }
    drop(input);
    let status = player.wait().map_err(|e| format!("pw-play: {e}"))?;
    if !status.success() {
        return Err(format!("pw-play: {status}"));
    }
    Ok(stamps)
}

/// Each column's lit heights from the floor in mode 13's spectrum area, rows 3–14, column 1
/// first: a dot right above its bar looks like a taller bar, so columns compare by these.
pub fn heights(frame: &Frame) -> Vec<Vec<u8>> {
    (1..=14)
        .map(|c| {
            (1..=12)
                .filter(|&h| frame.get(15 - usize::from(h), c))
                .collect()
        })
        .collect()
}

/// The lit heights `expected.txt`'s rows give each column.
pub fn expected_heights(rows: &Rows) -> Vec<Vec<u8>> {
    let dots = rows.dots.unwrap_or([None; 14]);
    rows.bars
        .iter()
        .zip(dots)
        .map(|(&bar, dot)| (1..=bar).chain(dot).collect())
        .collect()
}

/// The steady tone's rows at H 12 with the default settings: `expected.txt`'s middle tick of the
/// tone, the same as the tick after it.
fn steady_rows() -> Result<Vec<Vec<u8>>, String> {
    let heading = format!("{TONE} · H 12 · peaks on · bars moderate · peaks slow");
    let section = expected()
        .into_iter()
        .find(|s| s.heading == heading)
        .ok_or_else(|| format!("expected.txt has no section {heading:?}"))?;
    let (middle, next) = (&section.ticks[1], &section.ticks[2]);
    if middle != next {
        return Err(format!(
            "{heading}: ticks 1 and 2 differ, so the tone isn't steady"
        ));
    }
    Ok(expected_heights(middle))
}

/// The floor line: every column 1 row high, no dot.
fn floor() -> Vec<Vec<u8>> {
    vec![vec![1]; 14]
}

/// The frames the dev recording kept from `from` on, with their write times.
fn frames_since(from: u64) -> Result<Vec<(u64, Frame)>, String> {
    let mut frames = Vec::new();
    for report in recording_lines(None)? {
        if report.ns < from {
            continue;
        }
        if let Message::Frame(frame) = message(&report)? {
            frames.push((report.ns, frame));
        }
    }
    Ok(frames)
}

/// Mode 13, upright, with the sound connected and the capture stream at full volume.
fn ready(tools: &Tools, session: &Session) -> Result<(), String> {
    zbus::block_on(session.main().set_rotation(0)).map_err(|e| format!("SetRotation: {e}"))?;
    set_mode(session, 13)?;
    let connected = soon(CONNECT_WITHIN, || {
        Ok(state(session)?.audio_connected == Some(true))
    })?;
    if !connected {
        return Err("in mode 13 the dev helper's sound didn't connect within 3 s".into());
    }
    let mut full = None;
    soon(CONNECT_WITHIN, || {
        full = capture_at_full_volume(&tools.output("pactl", &["list", "source-outputs"])?);
        Ok(full.is_some())
    })?;
    match full {
        Some(true) => Ok(()),
        Some(false) => Err(format!(
            "the {HELPER_NODE} capture stream isn't at 100 %: set it back in a mixer"
        )),
        None => Err(format!(
            "within 3 s pactl lists no {HELPER_NODE} capture stream with a volume"
        )),
    }
}

/// The real capture (Phase 9 plan, Task 5): two seconds of a steady tone at column 7's centre,
/// after a second of silence and before three. Every frame while it plays shows `expected.txt`'s rows for it, and the
/// floor line before it and once the bars and the dots have fallen after it.
fn capture(tools: &Tools, session: &Session) -> Result<String, String> {
    ready(tools, session)?;
    let second = RATE;
    let samples: Vec<f32> = (0..6 * second)
        .map(|n| {
            if (second..3 * second).contains(&n) {
                tone(n)
            } else {
                0.0
            }
        })
        .collect();
    let begin = monotonic_ns();
    let stamps = play(tools, &samples, &[second, 3 * second])?;
    thread::sleep(FLUSHED_WITHIN);
    let (on, off) = (stamps[0], stamps[1]);
    let ms = |n: u64| n * 1_000_000;
    let steady = steady_rows()?;
    let frames = frames_since(begin)?;
    let (mut toned, mut floored) = (0, 0);
    for (ns, frame) in &frames {
        let shown = heights(frame);
        let rows_1_2_off = (1..=2).all(|r| (1..=14).all(|c| !frame.get(r, c)));
        if !rows_1_2_off {
            return Err(format!("a frame at {ns} ns lights rows 1–2"));
        }
        let want = if *ns >= on + ms(300) && *ns <= off {
            toned += 1;
            &steady
        } else if *ns + ms(200) < on || *ns >= off + ms(1800) {
            floored += 1;
            &floor()
        } else {
            continue;
        };
        if shown != *want {
            return Err(format!(
                "a frame {} ms after the tone began shows {shown:?}, not {want:?}",
                (*ns as i64 - on as i64) / 1_000_000
            ));
        }
    }
    if toned < 10 || floored < 5 {
        return Err(format!(
            "too few frames recorded: {toned} during the tone, {floored} in silence"
        ));
    }
    Ok(format!(
        "{toned} frames during the tone show its expected rows, {floored} in silence the floor line"
    ))
}

/// The value at quantile `q` of sorted values, nearest-rank.
pub fn quantile(sorted: &[f64], q: f64) -> f64 {
    let rank = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    sorted[rank - 1]
}

/// The delay (owner, 2026-10-25; solution review P9-06): 200 bursts, each a frame's write time
/// minus the time its first sample was written; fails when the 95th percentile is above 150 ms.
fn delay(tools: &Tools, session: &Session) -> Result<String, String> {
    ready(tools, session)?;
    let lead = RATE;
    let total = lead + BURSTS * BURST_EVERY;
    let samples: Vec<f32> = (0..total)
        .map(|n| {
            let into = n.checked_sub(lead).map(|i| i % BURST_EVERY);
            if into.is_some_and(|i| i < BURST) {
                tone(n)
            } else {
                0.0
            }
        })
        .collect();
    let marks: Vec<usize> = (0..BURSTS).map(|b| lead + b * BURST_EVERY).collect();
    let begin = monotonic_ns();
    let stamps = play(tools, &samples, &marks)?;
    thread::sleep(FLUSHED_WITHIN);
    let frames = frames_since(begin)?;
    let delays = delays(&frames, &stamps)?;
    let mut sorted = delays.clone();
    sorted.sort_by(f64::total_cmp);
    let (p50, p95, p99, max) = (
        quantile(&sorted, 0.5),
        quantile(&sorted, 0.95),
        quantile(&sorted, 0.99),
        sorted[sorted.len() - 1],
    );
    let report = format!(
        "{} bursts: median {p50:.1} ms, 95th percentile {p95:.1} ms, 99th {p99:.1} ms, maximum \
         {max:.1} ms; written 5 ms at a time, at most 10 ms queued",
        delays.len()
    );
    if p95 > DELAY_TARGET_MS {
        Err(format!(
            "{report}: the 95th percentile is above {DELAY_TARGET_MS} ms"
        ))
    } else {
        Ok(report)
    }
}

/// Each burst's delay in ms: the first frame at or after its first sample whose column 7 rises
/// above the floor, before the next burst.
///
/// # Errors
///
/// When a burst shows in no frame.
pub fn delays(frames: &[(u64, Frame)], stamps: &[u64]) -> Result<Vec<f64>, String> {
    stamps
        .iter()
        .enumerate()
        .map(|(b, &at)| {
            let until = stamps.get(b + 1).copied().unwrap_or(u64::MAX);
            frames
                .iter()
                .find(|(ns, frame)| *ns >= at && *ns < until && heights(frame)[6].len() > 1)
                .map(|(ns, _)| (ns - at) as f64 / 1e6)
                .ok_or_else(|| format!("burst {} shows in no frame before the next", b + 1))
        })
        .collect()
}

/// Without PipeWire or the dev service: the test tone's signal through the engine in process, with
/// `drawn` saying which modes it draws, its frames against `expected.txt` tick for tick.
pub fn fake_capture(drawn: Drawn) -> Result<String, String> {
    let tone = signal(TONE);
    let heading = format!("{TONE} · H 12 · peaks on · bars moderate · peaks slow");
    let section = expected()
        .into_iter()
        .find(|s| s.heading == heading)
        .ok_or_else(|| format!("expected.txt has no section {heading:?}"))?;
    let audio = std::sync::Arc::new(FakeAudio::new());
    let settings = Settings {
        mode: 13,
        ..Settings::default()
    };
    let mut engine = Engine::with_audio(
        settings,
        odc_sensors::Snapshot::new(
            |_| odc_sensors::Reading::Unavailable(odc_sensors::Unavailable::NotYet),
            odc_sensors::Fan::NotChosen,
            odc_sensors::Fan::NotChosen,
        ),
        std::sync::Arc::clone(&audio) as std::sync::Arc<dyn AudioSource>,
        drawn,
    );
    let windows: Vec<[f32; WINDOW]> = tone.windows();
    for (n, (window, rows)) in windows.iter().zip(&section.ticks).enumerate() {
        audio.set(*window);
        let frame = engine
            .tick(None, None, false)
            .frame
            .ok_or("the engine drew no frame")?;
        if heights(&frame) != expected_heights(rows) {
            return Err(format!(
                "{TONE}, tick {n}: the spectrum differs from expected.txt"
            ));
        }
    }
    Ok(format!(
        "in process: {TONE}, {} ticks as expected",
        windows.len()
    ))
}
