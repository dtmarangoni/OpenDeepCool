//! The settings app's end-to-end run (Phase 10 plan, Tasks 8 and 9): the real `opendeepcool`
//! binary in a headless weston, driven through AT-SPI by `odc-ui-e2e`, each scenario a check of
//! its own. Both binaries are built first, from the code being tested.
//!
//! - **Against the fake service** on a private bus, every time: every scenario of the design's
//!   coverage check and States table.
//! - **Against the real dev service**, on the system bus, when it runs here (decisions log D83):
//!   the app's own changes, which the service must confirm, with no test session held; then, in
//!   a test session at fixed readings, the frames the service wrote to the cooler against those
//!   the app received (its `--frame-log`). The owner's settings are set back afterwards, however
//!   the run ends.
//!
//! Nothing reaches the user's session: weston has a runtime folder of its own, the app's
//! accessibility bus and the fake's bus are private daemons, and the app runs with an
//! environment that holds only them, the Cairo renderer, GLib's in-memory settings, and home,
//! configuration, cache, data and state folders of its own (as the in-process tests, decisions
//! log D52); the real run's app reaches the system bus too. Everything, weston, the daemons and
//! the app, ends with this program.
//!
//! Without weston the area fails with its fix: a UI test without a display proves nothing (Phase
//! 10 plan, Task 8).

use std::ffi::OsStr;
use std::fs::{self, DirBuilder, File};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use odc_ipc::{OpenDeepCoolProxy, State, frame_pixels};
use odc_protocol::Message;
use odc_sensors::{Exact, Metric};
use odc_testkit::bus::PrivateBus;
use odc_testkit::fake_service::FakeService;
use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};
use odc_testkit::weston::Weston;

use crate::plays::{FLUSHED_WITHIN, message, override_args, recording_lines};
use crate::service::Restore;
use crate::session::Session;

/// The app and the driver, built next to this program.
struct Programs {
    app: PathBuf,
    driver: PathBuf,
}

/// Builds both with the profile this program was built with (decisions log D77).
fn build() -> Result<Programs, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new(cargo);
    command
        .args([
            "build",
            "--quiet",
            "-p",
            "odc-app",
            "-p",
            "odc-ui-e2e",
            "--bins",
        ])
        .current_dir(workspace);
    if !cfg!(debug_assertions) {
        command.arg("--release");
    }
    let status = command
        .status()
        .map_err(|e| format!("cargo can't start: {e}"))?;
    if !status.success() {
        return Err(format!("building the app and the driver failed: {status}"));
    }
    let bin = std::env::current_exe()
        .map_err(|e| format!("this program's path: {e}"))?
        .parent()
        .map(Path::to_owned)
        .ok_or("this program has no folder")?;
    Ok(Programs {
        app: bin.join("opendeepcool"),
        driver: bin.join("odc-ui-e2e"),
    })
}

/// A child that's killed when dropped.
struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn folder(path: &Path) -> Result<PathBuf, String> {
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path.to_owned())
}

/// What both runs share: the programs, a fresh folder, weston and the accessibility bus.
struct Stage {
    programs: Programs,
    dir: PathBuf,
    runtime: PathBuf,
    weston: Weston,
    a11y: PrivateBus,
}

impl Stage {
    fn start(out_dir: &Path) -> Result<Stage, String> {
        let programs = build()?;
        let dir = out_dir.join("app");
        match fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(format!("{}: {e}", dir.display()));
            }
            _ => {}
        }
        let dir = fs::canonicalize(folder(&dir)?).map_err(|e| format!("{}: {e}", dir.display()))?;
        let runtime = folder(&dir.join("runtime"))?;
        let weston = Weston::desktop(&runtime).map_err(|e| format!("weston: {e}"))?;
        let a11y = PrivateBus::accessibility(&dir.join("a11y"), &runtime)
            .map_err(|e| format!("the accessibility bus: {e}"))?;
        Ok(Stage {
            programs,
            dir,
            runtime,
            weston,
            a11y,
        })
    }

    /// The app in weston, its standard error in `<run>.app.log`, with `args`.
    fn app(&self, run: &str, args: &[&OsStr]) -> Result<Killed, String> {
        let log_path = self.dir.join(format!("{run}.app.log"));
        let log = File::create(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
        let own = |name: &str| folder(&self.dir.join(run).join(name));
        Command::new("setpriv")
            .args(["--pdeathsig", "KILL", "--"])
            .arg(&self.programs.app)
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "C.UTF-8")
            .env("HOME", own("home")?)
            .env("WAYLAND_DISPLAY", self.weston.display())
            .env("GDK_BACKEND", "wayland")
            .env("GSK_RENDERER", "cairo")
            .env("AT_SPI_BUS_ADDRESS", self.a11y.address())
            .env("GSETTINGS_BACKEND", "memory")
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("XDG_CONFIG_HOME", own("config")?)
            .env("XDG_CACHE_HOME", own("cache")?)
            .env("XDG_DATA_HOME", own("data")?)
            .env("XDG_STATE_HOME", own("state")?)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map(Killed)
            .map_err(|e| format!("the app can't start: {e}"))
    }

    /// The driver for `run`'s app, with `extra` flags; each scenario it reports becomes a check.
    fn driver(
        &self,
        run: &str,
        fake: &Path,
        extra: &[&str],
        summary: &mut Summary,
    ) -> Result<(), String> {
        let out = folder(&self.dir.join(run))?;
        let mut driver = Killed(
            Command::new(&self.programs.driver)
                .arg("--a11y-bus")
                .arg(self.a11y.address())
                .arg("--fake")
                .arg(fake)
                .arg("--app-log")
                .arg(self.dir.join(format!("{run}.app.log")))
                .arg("--frame-log")
                .arg(self.dir.join(format!("{run}.frames.log")))
                .arg("--omarchy-state")
                .arg(self.dir.join(run).join("omarchy"))
                .arg("--wayland-display")
                .arg(self.weston.display())
                .arg("--out")
                .arg(&out)
                .args(extra)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .spawn()
                .map_err(|e| format!("the driver can't start: {e}"))?,
        );
        let stdout = driver.0.stdout.take().ok_or("the driver's output")?;
        let mut reported = 0;
        let mut failed = false;
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            println!("app: {line}");
            if let Some((name, result)) = scenario(&line) {
                failed |= result.is_err();
                summary.check(&format!("app_{name}"), result);
                reported += 1;
            }
        }
        let status = driver.0.wait().map_err(|e| format!("the driver: {e}"))?;
        if reported == 0 || (!status.success() && !failed) {
            return Err(format!(
                "the driver ended ({status}) after {reported} scenarios"
            ));
        }
        Ok(())
    }
}

pub fn run(roots: &Roots, out_dir: &Path, summary: &mut Summary) {
    let missing = match probe(Capability::Weston, roots) {
        Probe::Real => None,
        Probe::Absent { reason } => Some(format!("{reason}; fix: install weston")),
        Probe::Unusable { reason, fix } => Some(format!("{reason}; fix: {fix}")),
    };
    if let Some(reason) = missing {
        summary.mode(
            Capability::Weston,
            Mode::Failed {
                reason: reason.clone(),
            },
        );
        summary.check("app", Err(reason));
        return;
    }
    summary.mode(Capability::Weston, Mode::Real);
    let stage = match Stage::start(out_dir) {
        Ok(stage) => stage,
        Err(e) => {
            summary.check("app", Err(e));
            return;
        }
    };
    if let Err(e) = against_the_fake(&stage, summary) {
        summary.check("app", Err(e));
    }
    against_the_service(&stage, roots, summary);
}

/// Every scenario against the fake service (Task 8).
fn against_the_fake(stage: &Stage, summary: &mut Summary) -> Result<(), String> {
    let bus = PrivateBus::start(&stage.dir.join("service-bus"))
        .map_err(|e| format!("the fake's bus: {e}"))?;
    let fake_dir = folder(&stage.dir.join("fake"))?;
    let _fake = FakeService::start(bus.address(), &fake_dir)
        .map_err(|e| format!("the fake service: {e}"))?;
    let omarchy = stage.dir.join("fake").join("omarchy");
    let frames = stage.dir.join("fake.frames.log");
    let _app = stage.app(
        "fake",
        &[
            OsStr::new("--service-address"),
            OsStr::new(bus.address()),
            OsStr::new("--omarchy-state"),
            omarchy.as_os_str(),
            OsStr::new("--frame-log"),
            frames.as_os_str(),
        ],
    )?;
    stage.driver("fake", &fake_dir, &[], summary)
}

/// What says the installed dev copy is older than this build: a state without the GPU in use,
/// which Phase 10 added (Task 2).
const STALE: &str = "the dev service is older than this build; fix: run \
                     `cargo run -p odc-dev -- setup`";

/// The real run, where the dev service runs (decisions log D83); else nothing, as every
/// scenario ran against the fake.
fn against_the_service(stage: &Stage, roots: &Roots, summary: &mut Summary) {
    match probe(Capability::DevService, roots) {
        Probe::Real => summary.mode(Capability::DevService, Mode::Real),
        Probe::Absent { reason } => {
            summary.mode(
                Capability::DevService,
                Mode::Fake {
                    reason: "the app's scenarios ran against the fake service".to_owned(),
                },
            );
            summary.skipped("app_real", &format!("no dev service: {reason}"));
            return;
        }
        Probe::Unusable { reason, fix } => {
            let reason = format!("{reason}; fix: {fix}");
            summary.mode(
                Capability::DevService,
                Mode::Failed {
                    reason: reason.clone(),
                },
            );
            summary.check("app_real", Err(reason));
            return;
        }
    }
    if let Err(e) = real_run(stage, summary) {
        summary.check("app_real", Err(e));
    }
}

/// The owner's settings, as the dev service has them.
fn saved_state() -> Result<State, String> {
    zbus::block_on(async {
        let conn = zbus::Connection::system()
            .await
            .map_err(|e| e.to_string())?;
        let service = OpenDeepCoolProxy::new(&conn)
            .await
            .map_err(|e| e.to_string())?;
        let dict = service
            .get_state()
            .await
            .map_err(|e| format!("GetState: {e}"))?;
        // A state this build can't read comes from an older dev copy.
        State::from_dict(&dict).map_err(|e| format!("{STALE} ({e})"))
    })
}

fn real_run(stage: &Stage, summary: &mut Summary) -> Result<(), String> {
    let saved = saved_state()?;
    if saved.gpu_in_use.is_none() {
        return Err(STALE.to_owned());
    }
    let omarchy = stage.dir.join("real").join("omarchy");
    let frames = stage.dir.join("real.frames.log");
    let app = stage.app(
        "real",
        &[
            OsStr::new("--omarchy-state"),
            omarchy.as_os_str(),
            OsStr::new("--frame-log"),
            frames.as_os_str(),
        ],
    )?;
    // The app's own changes, with no test session held: the service takes them.
    let changes = stage.driver("real", &stage.dir.join("real"), &["--real"], summary);
    // Then always the session, which also sets the owner's settings back.
    summary.check("app_real_preview", preview(&saved, &frames));
    drop(app);
    changes
}

/// The design's sample readings (Display page, Tile content), for the preview's check.
fn samples() -> Vec<(String, Option<Exact>)> {
    [
        (Metric::CpuUsage, 42),
        (Metric::GpuUsage, 63),
        (Metric::MemoryLoad, 51),
        (Metric::CpuTemperature, 57),
        (Metric::GpuTemperature, 38),
        (Metric::CpuPower, 65),
        (Metric::GpuPower, 88),
    ]
    .into_iter()
    .map(|(metric, value)| (metric.spec_name().to_owned(), Some(Exact::whole(value))))
    .collect()
}

/// The modes and rotations the preview's check shows, about 1.5 s each.
const SHOWN: [(u32, u8); 4] = [(1, 0), (9, 1), (12, 0), (13, 0)];

/// How far apart the cooler's write and the app's receipt of a frame may be.
const LATE: Duration = Duration::from_millis(500);

/// The preview is the cooler (Phase 10 plan, Task 9; AC3): in a test session at fixed readings,
/// the harness shows a few modes, one turned, and each frame the service wrote to the cooler,
/// each change once, must have reached the app, in order, within half a second. The owner's
/// settings are set back when the session's guard drops, before the session ends.
fn preview(saved: &State, frames: &Path) -> Result<String, String> {
    let session = Session::begin()?;
    let result = (|| {
        let _restore = Restore {
            session: &session,
            state: saved.clone(),
        };
        let (values, unavailable) = override_args(&samples(), 1, &[]);
        let first = zbus::block_on(session.dev().override_readings(values, unavailable))
            .map_err(|e| format!("OverrideReadings: {e}"))?;
        for (mode, rotation) in SHOWN {
            zbus::block_on(session.main().set_rotation(rotation))
                .map_err(|e| format!("SetRotation: {e}"))?;
            zbus::block_on(session.main().set_mode(mode)).map_err(|e| format!("SetMode: {e}"))?;
            thread::sleep(Duration::from_millis(1500));
        }
        thread::sleep(FLUSHED_WITHIN);
        compare(&written(first)?, &logged(frames)?)
    })();
    let ended = session.end();
    let note = result?;
    ended.map(|()| note)
}

/// A frame's time in ns and its 196 dots.
type Timed = (u64, Vec<u8>);

/// The frames the service wrote to the cooler from tick `first` on, each change once.
fn written(first: u64) -> Result<Vec<Timed>, String> {
    let mut frames: Vec<Timed> = Vec::new();
    for report in recording_lines(None)? {
        if report.tick < first {
            continue;
        }
        if let Message::Frame(frame) = message(&report)? {
            let pixels = frame_pixels(&frame);
            if frames.last().map(|(_, p)| p) != Some(&pixels) {
                frames.push((report.ns, pixels));
            }
        }
    }
    Ok(frames)
}

/// The frames the app logged: `<ns> <196 dots>` a line.
fn logged(path: &Path) -> Result<Vec<Timed>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    text.lines()
        .map(|line| {
            let (ns, dots) = line
                .split_once(' ')
                .ok_or_else(|| format!("the frame log: {line:?}"))?;
            let ns = ns
                .parse::<u64>()
                .map_err(|_| format!("the frame log: {line:?}"))?;
            let pixels: Vec<u8> = dots.bytes().map(|b| b.wrapping_sub(b'0')).collect();
            if pixels.len() != 196 || pixels.iter().any(|&p| p > 1) {
                return Err(format!("the frame log: {line:?}"));
            }
            Ok((ns, pixels))
        })
        .collect()
}

/// Each written frame, in order, among the app's, each within [`LATE`] of the write. The first
/// written frame may have reached the app before the run's first tick, unchanged since.
fn compare(written: &[Timed], logged: &[Timed]) -> Result<String, String> {
    let late = u64::try_from(LATE.as_nanos()).unwrap_or(u64::MAX);
    let start = |frame: &Timed| {
        logged
            .iter()
            .rposition(|(t, p)| p == &frame.1 && *t <= frame.0 + late)
    };
    let first = written.first().ok_or("the cooler got no frame")?;
    let at = start(first).ok_or("the app never got the cooler's first frame")?;
    let mut worst = 0_u64;
    for (i, (when, pixels)) in written.iter().enumerate() {
        let Some((got, received)) = logged.get(at + i) else {
            return Err(format!(
                "the app got {i} of the cooler's {} frames",
                written.len()
            ));
        };
        if received != pixels {
            return Err(format!(
                "frame {i} of {}: the app's differs from the cooler's",
                written.len()
            ));
        }
        // The first may have come before the run's ticks, unchanged since.
        if i > 0 {
            let apart = got.abs_diff(*when);
            if apart > late {
                return Err(format!(
                    "frame {i}: the app's {} ms from the cooler's",
                    apart / 1_000_000
                ));
            }
            worst = worst.max(apart);
        }
    }
    Ok(format!(
        "{} frames alike, at most {} ms apart",
        written.len(),
        worst / 1_000_000
    ))
}

/// A driver's line: `ok <name> <seconds> [<note>]` or `FAILED <name> <seconds> <why>`.
fn scenario(line: &str) -> Option<(String, Result<String, String>)> {
    let (verdict, rest) = line.split_once(' ')?;
    let mut parts = rest.splitn(3, ' ');
    let name = parts.next()?.to_owned();
    let seconds = parts.next()?;
    match verdict {
        "ok" => Some((
            name,
            Ok(match parts.next() {
                Some(note) => format!("{seconds} s, {note}"),
                None => format!("{seconds} s"),
            }),
        )),
        "FAILED" => Some((name, Err(parts.next().unwrap_or("").to_owned()))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: u64 = 1_000_000;

    fn frame(lit: usize) -> Vec<u8> {
        let mut pixels = vec![0; 196];
        pixels[lit] = 1;
        pixels
    }

    // Each written frame among the app's, in order, each within half a second; the first may
    // have reached the app long before, unchanged since.
    #[test]
    fn the_app_s_frames_must_be_the_cooler_s_in_order_and_on_time() {
        let written = vec![
            (10_000 * MS, frame(1)),
            (10_100 * MS, frame(2)),
            (10_200 * MS, frame(3)),
        ];
        let logged = vec![
            (2_000 * MS, frame(9)),
            (3_000 * MS, frame(1)),
            (10_103 * MS, frame(2)),
            (10_210 * MS, frame(3)),
            (11_000 * MS, frame(4)),
        ];
        assert_eq!(
            compare(&written, &logged),
            Ok("3 frames alike, at most 10 ms apart".to_owned())
        );
        let mut missing = logged.clone();
        missing.remove(2);
        assert!(compare(&written, &missing).is_err(), "a frame missing");
        let mut late = logged.clone();
        late[3].0 = 10_800 * MS;
        assert!(compare(&written, &late).is_err(), "a frame late");
        let mut other = logged.clone();
        other[3].1 = frame(5);
        assert!(compare(&written, &other).is_err(), "another frame");
        assert!(compare(&written, &logged[..2]).is_err(), "too few");
        assert!(compare(&[], &logged).is_err(), "nothing written");
    }

    #[test]
    fn the_frame_log_is_read_line_by_line() {
        let dir = std::env::temp_dir().join(format!("odc-e2e-frames-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("a folder");
        let path = dir.join("frames.log");
        let dots = |lit: usize| {
            frame(lit)
                .iter()
                .map(|&p| char::from(b'0' + p))
                .collect::<String>()
        };
        fs::write(&path, format!("5 {}\n7 {}\n", dots(0), dots(195))).expect("written");
        assert_eq!(logged(&path), Ok(vec![(5, frame(0)), (7, frame(195))]));
        for bad in ["5", "x 0101", &format!("5 {}2", &dots(0)[1..])] {
            fs::write(&path, format!("{bad}\n")).expect("written");
            assert!(logged(&path).is_err(), "{bad:?}");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_driver_s_lines_become_checks() {
        assert_eq!(
            scenario("ok tiles 4.2"),
            Some(("tiles".to_owned(), Ok("4.2 s".to_owned())))
        );
        assert_eq!(
            scenario("ok shortcuts 1.3 keys don't reach it"),
            Some((
                "shortcuts".to_owned(),
                Ok("1.3 s, keys don't reach it".to_owned())
            ))
        );
        assert_eq!(
            scenario("FAILED tiles 5.0 Load: CPU not selected (the tree: /x)"),
            Some((
                "tiles".to_owned(),
                Err("Load: CPU not selected (the tree: /x)".to_owned())
            ))
        );
        assert_eq!(scenario("something else"), None);
    }
}
