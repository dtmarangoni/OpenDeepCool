//! The settings app's end-to-end run (Phase 10 plan, Task 8): the real `opendeepcool` binary in
//! a headless weston, against the fake service on a private bus, driven through AT-SPI by
//! `odc-ui-e2e`, each scenario a check of its own. Both binaries are built first, from the code
//! being tested.
//!
//! Nothing reaches the user's session: weston has a runtime folder of its own, the app's
//! accessibility bus and the fake's bus are private daemons, and the app runs with an
//! environment that holds only them, the Cairo renderer, GLib's in-memory settings, and home,
//! configuration, cache, data and state folders of its own (as the in-process tests, decisions
//! log D52). Everything, weston, the daemons and the app, ends with this program.
//!
//! Without weston the area fails with its fix: a UI test without a display proves nothing (Phase
//! 10 plan, Task 8).

use std::fs::{self, DirBuilder, File};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use odc_testkit::bus::PrivateBus;
use odc_testkit::fake_service::FakeService;
use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};
use odc_testkit::weston::Weston;

/// The app and the driver, built next to this program.
struct Programs {
    app: PathBuf,
    driver: PathBuf,
}

/// Builds both with the profile this program was built with.
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
    if let Err(e) = scenarios(out_dir, summary) {
        summary.check("app", Err(e));
    }
}

/// Starts everything, runs the driver, and records each scenario it reports.
fn scenarios(out_dir: &Path, summary: &mut Summary) -> Result<(), String> {
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
    let service_bus =
        PrivateBus::start(&dir.join("service-bus")).map_err(|e| format!("the fake's bus: {e}"))?;
    let a11y = PrivateBus::accessibility(&dir.join("a11y"), &runtime)
        .map_err(|e| format!("the accessibility bus: {e}"))?;
    let fake_dir = folder(&dir.join("fake"))?;
    let _fake = FakeService::start(service_bus.address(), &fake_dir)
        .map_err(|e| format!("the fake service: {e}"))?;
    let omarchy = dir.join("omarchy");
    let app_log = dir.join("app.log");
    let log = File::create(&app_log).map_err(|e| format!("{}: {e}", app_log.display()))?;
    let _app = Killed(
        Command::new("setpriv")
            .args(["--pdeathsig", "KILL", "--"])
            .arg(&programs.app)
            .arg("--service-address")
            .arg(service_bus.address())
            .arg("--omarchy-state")
            .arg(&omarchy)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "C.UTF-8")
            .env("HOME", folder(&dir.join("home"))?)
            .env("WAYLAND_DISPLAY", weston.display())
            .env("GDK_BACKEND", "wayland")
            .env("GSK_RENDERER", "cairo")
            .env("AT_SPI_BUS_ADDRESS", a11y.address())
            .env("GSETTINGS_BACKEND", "memory")
            .env("XDG_RUNTIME_DIR", &runtime)
            .env("XDG_CONFIG_HOME", folder(&dir.join("config"))?)
            .env("XDG_CACHE_HOME", folder(&dir.join("cache"))?)
            .env("XDG_DATA_HOME", folder(&dir.join("data"))?)
            .env("XDG_STATE_HOME", folder(&dir.join("state"))?)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map_err(|e| format!("the app can't start: {e}"))?,
    );
    let mut driver = Killed(
        Command::new(&programs.driver)
            .arg("--a11y-bus")
            .arg(a11y.address())
            .arg("--fake")
            .arg(&fake_dir)
            .arg("--app-log")
            .arg(&app_log)
            .arg("--omarchy-state")
            .arg(&omarchy)
            .arg("--wayland-display")
            .arg(weston.display())
            .arg("--out")
            .arg(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("the driver can't start: {e}"))?,
    );
    let stdout = driver.0.stdout.take().ok_or("the driver's output")?;
    let mut reported = 0;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        println!("app: {line}");
        if let Some((name, result)) = scenario(&line) {
            summary.check(&format!("app_{name}"), result);
            reported += 1;
        }
    }
    let status = driver.0.wait().map_err(|e| format!("the driver: {e}"))?;
    if reported == 0 || (!status.success() && summary.passed()) {
        return Err(format!(
            "the driver ended ({status}) after {reported} scenarios"
        ));
    }
    Ok(())
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
