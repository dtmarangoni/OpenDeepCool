//! `opendeepcoold`, the display service (Phase 7 plan, Task 8): it drives the DeepCool LP360's
//! display from boot under systemd, and serves the settings app on the system bus. Its only
//! switch is `--dev`, which only the dev copy's drop-in passes: the dev interface and the frame
//! recording (`docs/spec/dbus.md`, Dev interface). Any other argument is an error, so a typo
//! can't start a different service.

use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use odc_device::{Connector, HidrawConnector, Lp360, ROTATE_AT, Recording, RecordingHandle};
use odc_sensors::{Registry, Roots, SystemClock, fan_inputs};
use odc_service::dbus::{Bus, Checks};
use odc_service::engine::Choices;
use odc_service::log::{Journal, Priority, Stderr};
use odc_service::runtime::{RegistrySensors, Sensors, abort_on_panic, sd_notify};
use odc_service::service::{Config, Running};
use odc_service::settings::{Found, Settings, SettingsStore};
use odc_service::status::gpus_at_start;
use odc_service::{RECORDING, STATE_DIR};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

const USAGE: &str = "usage: opendeepcoold [--dev]";

fn main() -> ExitCode {
    abort_on_panic();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dev = match args.as_slice() {
        [] => false,
        [flag] if flag == "--dev" => true,
        _ => {
            eprintln!("<3>{USAGE}");
            return ExitCode::from(2);
        }
    };
    let journal: Arc<dyn Journal> = Arc::new(Stderr);
    match run(dev, &journal) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            journal.write(Priority::Error, &e);
            ExitCode::FAILURE
        }
    }
}

/// The effective group ID: the second number of `/proc/self/status`'s `Gid:` line (real,
/// effective, saved, file system; proc(5)). The dev interface admits this group.
fn effective_gid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|l| l.strip_prefix("Gid:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

fn run(dev: bool, journal: &Arc<dyn Journal>) -> Result<(), String> {
    // Caught before anything starts, so a stop during the start isn't lost.
    let mut signals =
        Signals::new([SIGTERM, SIGINT]).map_err(|e| format!("the signal handlers: {e}"))?;
    let state = Path::new(STATE_DIR);
    let store = SettingsStore::new(state);
    let (mut settings, found) = store
        .load()
        .map_err(|e| format!("{}: {e}", store.path().display()))?;
    match found {
        Found::Read => {}
        Found::Created => journal.write(Priority::Info, "no settings yet: the defaults, saved"),
        Found::Replaced { moved_to, why } => journal.write(
            Priority::Warning,
            &format!(
                "the settings file was refused ({why}): kept as {}, and the defaults saved",
                moved_to.display()
            ),
        ),
    }
    let roots = Roots::system();
    let found = gpus_at_start(&roots.sys, state, &store, &mut settings);
    for (priority, line) in &found.lines {
        journal.write(*priority, line);
    }
    let inputs = fan_inputs(&roots.sys);
    let mut sensors = RegistrySensors {
        registry: Registry::from_rules(SystemClock, roots),
        gpus: found.gpus.clone(),
    };
    sensors.choose(&Choices::of(&settings));
    let connector = HidrawConnector::system();
    let parts = Parts {
        settings,
        store,
        sensors,
        gpus: found.gpus,
        fan_inputs: inputs,
        journal: Arc::clone(journal),
    };
    let (running, recording) = if dev {
        let recording = Recording::dev(connector, state.join(RECORDING), ROTATE_AT)
            .map_err(|e| format!("{}: {e}", state.join(RECORDING).display()))?;
        let handle = recording.handle();
        let status = std::fs::read_to_string("/proc/self/status")
            .map_err(|e| format!("/proc/self/status: {e}"))?;
        let group = effective_gid(&status).ok_or("no Gid: line in /proc/self/status")?;
        let running = start(parts, Lp360::new(recording), Some((group, handle.clone())));
        (running, Some(handle))
    } else {
        (start(parts, Lp360::new(connector), None), None)
    };
    let running = running.map_err(|e| format!("the service can't start: {e}"))?;
    // Outside systemd there's no one to tell.
    let _ = sd_notify("READY=1");
    journal.write(
        Priority::Info,
        if dev {
            "started, with the dev interface"
        } else {
            "started"
        },
    );
    let signal = signals.forever().next();
    let _ = sd_notify("STOPPING=1");
    running.stop();
    if let Some(recording) = recording {
        recording.flush();
    }
    let name = match signal {
        Some(SIGTERM) => "SIGTERM",
        Some(SIGINT) => "SIGINT",
        _ => "a signal",
    };
    journal.write(Priority::Info, &format!("stopped on {name}"));
    Ok(())
}

/// What the service starts with besides its cooler.
struct Parts {
    settings: Settings,
    store: SettingsStore,
    sensors: RegistrySensors<SystemClock>,
    gpus: Vec<odc_sensors::Gpu>,
    fan_inputs: Vec<odc_sensors::FanInput>,
    journal: Arc<dyn Journal>,
}

/// Starts the service on the system bus, checked with polkit.
fn start<C>(
    parts: Parts,
    lp360: Lp360<C>,
    dev: Option<(u32, RecordingHandle)>,
) -> io::Result<Running>
where
    C: Connector + Send + 'static,
    C::Transport: Send,
{
    Running::start(Config {
        settings: parts.settings,
        store: parts.store,
        lp360,
        sensors: parts.sensors,
        gpus: parts.gpus,
        fan_inputs: parts.fan_inputs,
        bus: Bus::System,
        checks: Checks::Polkit,
        journal: parts.journal,
        dev,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_effective_group_comes_from_proc_status() {
        assert_eq!(
            effective_gid("Name:\tx\nUid:\t961\t961\t961\t961\nGid:\t5\t960\t960\t960\n"),
            Some(960)
        );
        assert_eq!(effective_gid("Name:\tx\n"), None);
    }
}
