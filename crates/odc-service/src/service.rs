//! The service, put together (Phase 7 plan, Recommendation): the sensor, device and engine
//! threads, and the D-Bus side on threads of its own. `opendeepcoold` (Task 8) starts it with
//! the cooler, the sensors and the system bus; the tests with fakes on a private bus.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, RwLock, RwLockWriteGuard};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use odc_device::{Connector, Link, Lp360, Reason, RecordingHandle};
use odc_protocol::Frame;
use odc_sensors::{Binding, FanInput, Gpu, Metric};

use crate::dbus::{self, Bus, Checks};
use crate::engine::{Apply, Engine, Published, Requests};
use crate::log::{Journal, Priority};
use crate::pulse::Pulse;
use crate::runtime::{
    Device, DeviceEvent, Outputs, Progress, SensorLoop, SensorRequest, Sensors, Slot, SystemTicker,
    forward, run_device, run_engine, run_sensors, sd_notify,
};
use crate::settings::{Settings, SettingsStore};
use crate::status::Status;

/// What the service starts with.
pub struct Config<C: Connector, S: Sensors> {
    /// The saved settings, as loaded, and their file.
    pub settings: Settings,
    pub store: SettingsStore,
    pub lp360: Lp360<C>,
    pub sensors: S,
    /// The GPUs and fan inputs found at start.
    pub gpus: Vec<Gpu>,
    pub fan_inputs: Vec<FanInput>,
    pub bus: Bus,
    pub checks: Checks,
    pub journal: Arc<dyn Journal>,
    /// With `--dev`: the service's group, and the recording `lp360` writes through.
    pub dev: Option<(u32, RecordingHandle)>,
}

/// The running service; dropping it stops it.
pub struct Running {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    dbus: Option<dbus::Side>,
}

fn write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn errno(errno: Option<i32>) -> String {
    errno.map_or_else(String::new, |e| {
        format!(": {}", io::Error::from_raw_os_error(e))
    })
}

/// The journal line for a new link, one per change (owner, 2026-10-27; solution review P5-02).
fn link_line(link: &Link) -> (Priority, String) {
    match link {
        Link::Connected => (Priority::Info, "the cooler is connected".into()),
        Link::Disconnected(Reason::NotFound) => (Priority::Warning, "no cooler is attached".into()),
        Link::Disconnected(Reason::NoAccess { path, errno: e }) => (
            Priority::Warning,
            format!(
                "the cooler at {} can't be opened{}",
                path.display(),
                errno(*e)
            ),
        ),
        Link::Disconnected(Reason::WriteFailed { errno: e }) => (
            Priority::Warning,
            format!("a write to the cooler failed{}", errno(*e)),
        ),
    }
}

/// Each metric's binding, one line each: at start and at each rebind (PRD, Diagnostics).
fn binding_lines(bindings: &[(Metric, Binding)]) -> String {
    bindings
        .iter()
        .map(|(metric, binding)| match binding {
            Binding::Bound(source) => format!("{}: {source}", metric.spec_name()),
            Binding::Unbound(reason) => format!("{}: no source: {reason}", metric.spec_name()),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What the engine's loop sends out.
struct EngineOutputs {
    frames: Arc<Slot<(u64, Frame)>>,
    sensors: Sender<SensorRequest>,
    status: Arc<RwLock<Status>>,
    pulse: Arc<Pulse>,
    journal: Arc<dyn Journal>,
}

impl Outputs for EngineOutputs {
    fn frame(&mut self, tick: u64, frame: Frame) {
        self.frames.post((tick, frame));
    }

    fn requests(&mut self, requests: Requests) {
        forward(requests, &self.sensors);
    }

    fn status(&mut self, engine: &Engine) {
        if write(&self.status).from_engine(engine) {
            self.pulse.send();
        }
    }

    fn notify(&mut self, message: &str) {
        // Outside systemd there's no one to tell.
        let _ = sd_notify(message);
    }

    fn skipped(&mut self, from: u64, to: u64) {
        self.journal.write(
            Priority::Warning,
            &format!("ticks {from} to {to} were skipped: the engine fell behind"),
        );
    }
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) -> io::Result<JoinHandle<()>> {
    thread::Builder::new().name(name.into()).spawn(work)
}

impl Running {
    /// Starts every thread, the D-Bus side last; returns once the bus name is owned.
    ///
    /// # Errors
    ///
    /// When a thread can't start, or the D-Bus side can't serve; the threads started are stopped.
    pub fn start<C, S>(config: Config<C, S>) -> io::Result<Running>
    where
        C: Connector + Send + 'static,
        C::Transport: Send,
        S: Sensors + 'static,
    {
        let journal = config.journal;
        let stop = Arc::new(AtomicBool::new(false));
        let pulse = Arc::new(Pulse::default());
        let progress = Arc::new(Progress::new(Instant::now()));
        let apply = Arc::new(Slot::<Apply>::default());
        let published = Arc::new(Slot::<Published>::default());
        let frames = Arc::new(Slot::<(u64, Frame)>::default());
        let mut sensors = SensorLoop::new(config.sensors);
        let first = sensors.snapshot();
        let bindings = sensors.sensors.bindings();
        journal.write(Priority::Info, &binding_lines(&bindings));
        let status = Arc::new(RwLock::new(Status {
            bindings,
            gpus: config.gpus,
            fan_inputs: config.fan_inputs,
            ..Status::default()
        }));
        let engine = Engine::new(config.settings.clone(), first.snapshot);
        let (requests_tx, requests) = mpsc::channel::<SensorRequest>();
        let (control_tx, control) = mpsc::channel();
        let mut threads = Vec::new();
        let started = (|| -> io::Result<()> {
            threads.push(spawn("sensors", {
                let (published, progress, stop) = (
                    Arc::clone(&published),
                    Arc::clone(&progress),
                    Arc::clone(&stop),
                );
                let (status, journal) = (Arc::clone(&status), Arc::clone(&journal));
                move || {
                    let mut on_bindings = |bindings: Vec<(Metric, Binding)>| {
                        let mut status = write(&status);
                        if status.bindings != bindings {
                            journal.write(Priority::Info, &binding_lines(&bindings));
                            status.bindings = bindings;
                        }
                    };
                    run_sensors(
                        &mut sensors,
                        &requests,
                        &published,
                        &progress,
                        &mut on_bindings,
                        &stop,
                    );
                }
            })?);
            threads.push(spawn("device", {
                let (frames, progress, stop) = (
                    Arc::clone(&frames),
                    Arc::clone(&progress),
                    Arc::clone(&stop),
                );
                let (status, pulse, journal) = (
                    Arc::clone(&status),
                    Arc::clone(&pulse),
                    Arc::clone(&journal),
                );
                let mut device = match &config.dev {
                    Some((_, recording)) => Device::recorded(config.lp360, recording.clone()),
                    None => Device::new(config.lp360),
                };
                move || {
                    let mut on_event = |event: DeviceEvent<'_>| {
                        match event {
                            DeviceEvent::Written { link, changed, .. } => {
                                if !changed {
                                    return;
                                }
                                let (priority, line) = link_line(link);
                                journal.write(priority, &line);
                                write(&status).link = Some(link.clone());
                            }
                            DeviceEvent::Released(released) => {
                                journal.write(
                                    Priority::Notice,
                                    if released {
                                        "the cooler is released to a test"
                                    } else {
                                        "the cooler is taken back from the test"
                                    },
                                );
                                write(&status).released = released;
                            }
                        }
                        pulse.send();
                    };
                    run_device(
                        &mut device,
                        &frames,
                        &control,
                        &progress,
                        &mut on_event,
                        &stop,
                    );
                }
            })?);
            threads.push(spawn("engine", {
                let (apply, published, progress, stop) = (
                    Arc::clone(&apply),
                    Arc::clone(&published),
                    Arc::clone(&progress),
                    Arc::clone(&stop),
                );
                let mut outputs = EngineOutputs {
                    frames: Arc::clone(&frames),
                    sensors: requests_tx,
                    status: Arc::clone(&status),
                    pulse: Arc::clone(&pulse),
                    journal: Arc::clone(&journal),
                };
                move || {
                    run_engine(
                        engine,
                        &SystemTicker,
                        &apply,
                        &published,
                        &progress,
                        &mut outputs,
                        &stop,
                        None,
                    );
                }
            })?);
            Ok(())
        })();
        let dbus = started.and_then(|()| {
            dbus::Side::start(dbus::Parts {
                bus: config.bus,
                checks: config.checks,
                store: config.store,
                settings: config.settings,
                status,
                apply,
                device: control_tx,
                pulse,
                journal,
                dev: config.dev,
            })
            .map_err(io::Error::other)
        });
        // On an error, dropping `running` stops the threads already started.
        let mut running = Running {
            stop,
            threads,
            dbus: None,
        };
        running.dbus = Some(dbus?);
        Ok(running)
    }

    /// Stops the service: the D-Bus side first, so no call arrives meanwhile, then the threads.
    /// Dropping it does the same.
    pub fn stop(self) {}
}

impl Drop for Running {
    fn drop(&mut self) {
        self.dbus.take();
        self.stop.store(true, Ordering::Relaxed);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}
