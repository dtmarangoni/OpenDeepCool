//! `odc-e2e`: layer 4 of the tests, unattended (PRD, Testing Strategy; AGENTS.md rule 3). It
//! probes the PC it runs on: a device present and usable is tested for real, an absent one
//! through fakes, and one present but unusable fails with its fix. Its options are flags given at
//! start, never environment variables (owner, 2026-10-24). It never runs in `cargo test` or CI,
//! because it touches real hardware when present.

pub mod access;
pub mod app;
pub mod attended;
pub mod baseline;
pub mod dev_interface;
pub mod device;
pub mod display;
pub mod gpu_rank;
pub mod music;
pub mod package;
pub mod plays;
pub mod sensors;
pub mod service;
pub mod session;

use std::path::Path;
use std::time::Duration;

use odc_render::draws;
use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};

pub const USAGE: &str =
    "usage: odc-e2e [--require <capability,...>] [--only <area>] [--modes <list>]
                [--dev-interface]
       odc-e2e --attended [--modes <list>]

  --require <list>  capabilities that must run real: lp360, lp360-reconnect,
                    cpu-temp, cpu-power, proc-stat, meminfo, gpu-amdgpu, gpu-i915,
                    gpu-xe, gpu-nvidia, gpu-other, board-fans, sensors-tool,
                    gpu-load-tool, dev-service, pipewire, weston, container
  --only <area>     run one area's checks: device, display, gpu-rank, sensors,
                    service, music, app; or, only when named, baseline, the dev
                    service's resource use over 10 minutes, and package, the Arch
                    package installed and uninstalled both ways in a rootless
                    throwaway root, about 10 minutes
  --modes <list>    the modes display, service and the tour take, as numbers and
                    ranges: 7, or 7-9,12; without it, every mode this build draws
  --dev-interface   with the Arch package installed: one pkexec step, which you
                    approve with your password, adds the dev interface and the
                    frame recording to the packaged service for this run (a
                    drop-in in /run and a restart) and removes both when the run
                    ends, however it ends; the dev-service checks need it
  --attended        the optional tour for you to watch on the cooler, through the
                    dev service: you answer each case; never a gate
  -h, --help        this text

Each check probes the PC first: a device present and usable is tested for real,
an absent one through fakes, and one present but unusable fails with its fix.
While the dev service runs, the checks that write to the cooler themselves hold
a test session that releases it, and the service takes it back afterwards.
Don't run the research rig (DeepCreative) at the same time: it writes to the
cooler too.";

/// A group of checks `--only` can choose. Later phases add theirs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Device,
    /// Each mode's examples, written straight to the cooler (owner, 2026-11-05: Phase 7's
    /// `load`, for every drawn mode).
    Display,
    GpuRank,
    Sensors,
    /// The dev service's own checks (Phase 7 plan, Task 9).
    Service,
    /// The Music checks through a virtual output: the capture and the delay (Phase 9 plan, Task 5).
    Music,
    /// The settings app through AT-SPI in a headless weston, against a fake service (Phase 10
    /// plan, Task 8).
    App,
    /// The resource baseline (Phase 7 plan, Task 10): only when named.
    Baseline,
    /// The Arch package in a rootless throwaway root (Phase 11 plan, Task 5): only when named.
    Package,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub require: Vec<Capability>,
    pub only: Option<Area>,
    /// The attended tour instead of the unattended checks (owner, 2026-10-24).
    pub attended: bool,
    /// The modes `--modes` lists, in order; `None` for every mode this build draws.
    pub modes: Option<Vec<u16>>,
    /// The packaged service gets the dev interface for this run, through one approved root step
    /// that stays until the run ends (Phase 11 plan, Task 9).
    pub dev_interface: bool,
}

impl Options {
    /// The modes the display, service and tour checks take.
    pub fn modes(&self) -> Vec<u16> {
        self.modes.clone().unwrap_or_else(drawn_modes)
    }
}

/// Every mode this build draws, in order.
pub fn drawn_modes() -> Vec<u16> {
    odc_render::Mode::all()
        .filter(|&mode| draws(mode))
        .map(odc_render::Mode::number)
        .collect()
}

/// A `--modes` list: numbers and ranges, comma-separated (Phase 8 plan, Task 3). A mode outside
/// the mode table, or one this build doesn't draw yet, is an error.
fn parse_modes(list: &str) -> Result<Vec<u16>, String> {
    let mut modes = Vec::new();
    for part in list.split(',') {
        let (first, last) = part.split_once('-').unwrap_or((part, part));
        let number = |text: &str| {
            text.parse::<u16>()
                .map_err(|_| format!("{part:?} isn't a mode or a range of modes"))
        };
        let (first, last) = (number(first)?, number(last)?);
        if first > last {
            return Err(format!("{part:?}: a range goes from the lower mode up"));
        }
        for n in first..=last {
            let mode = odc_render::Mode::new(n)
                .ok_or_else(|| format!("mode {n} isn't in the mode table"))?;
            if !draws(mode) {
                return Err(format!("mode {n} isn't drawn by this build yet"));
            }
            if !modes.contains(&n) {
                modes.push(n);
            }
        }
    }
    modes.sort_unstable();
    Ok(modes)
}

/// The options, or `None` for `--help`. An unknown flag, capability or area is an error.
pub fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut options = Options::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--attended" => options.attended = true,
            "--dev-interface" => options.dev_interface = true,
            "--require" => {
                let list = args.next().ok_or("--require needs a list")?;
                for name in list.split(',') {
                    let capability = Capability::from_name(name)
                        .ok_or_else(|| format!("{name:?} isn't a capability"))?;
                    if !options.require.contains(&capability) {
                        options.require.push(capability);
                    }
                }
            }
            "--modes" => {
                let list = args.next().ok_or("--modes needs a list")?;
                options.modes = Some(parse_modes(list)?);
            }
            "--only" => {
                let area = args.next().ok_or("--only needs an area")?;
                options.only = Some(match area.as_str() {
                    "device" => Area::Device,
                    "display" => Area::Display,
                    "gpu-rank" => Area::GpuRank,
                    "sensors" => Area::Sensors,
                    "service" => Area::Service,
                    "music" => Area::Music,
                    "app" => Area::App,
                    "baseline" => Area::Baseline,
                    "package" => Area::Package,
                    _ => return Err(format!("{area:?} isn't an area")),
                });
            }
            _ => return Err(format!("{arg:?} isn't an option")),
        }
    }
    if options.attended && options.dev_interface {
        return Err("--dev-interface isn't for the attended tour".into());
    }
    Ok(Some(options))
}

/// Runs the chosen areas' checks one at a time, `tick` apart where they send frames (100 ms on
/// the real cooler, `protocol.md` Timing), and returns the summary.
pub fn run(options: &Options, roots: &Roots, out_dir: &Path, tick: Duration) -> Summary {
    let mut summary = Summary::default();
    if let Some(path) = dev_interface::leftover(&roots.run) {
        let note = format!(
            "{} was left by an earlier run whose step was killed: {}",
            path.display(),
            if options.dev_interface {
                "this run's step replaces it"
            } else {
                "the packaged service keeps --dev until it is removed (a run with \
                 --dev-interface replaces it, and a reboot clears it)"
            }
        );
        println!("{note}");
        summary.check("dev_interface_leftover", Ok(note));
    }
    // Held until the areas are done, and closed on a panic too (its `Drop`).
    let dev_interface = if options.dev_interface {
        match dev_interface::open() {
            Ok(guard) => {
                summary.check(
                    "dev_interface",
                    Ok(format!(
                        "the packaged {} runs with --dev for this run",
                        dev_interface::UNIT
                    )),
                );
                Some(guard)
            }
            Err(e) => {
                summary.check("dev_interface", Err(e));
                return summary;
            }
        }
    } else {
        None
    };
    match device::restore_switch(roots) {
        Ok(Some(note)) => {
            println!("{note}");
            summary.check("start-up", Ok(note));
        }
        Ok(None) => {}
        Err(e) => summary.check("start-up", Err(e)),
    }
    for &capability in &options.require {
        let reason = match probe(capability, roots) {
            Probe::Real => continue,
            Probe::Absent { reason } => reason,
            Probe::Unusable { reason, fix } => format!("{reason}; fix: {fix}"),
        };
        summary.mode(
            capability,
            Mode::Failed {
                reason: format!("required real: {reason}"),
            },
        );
    }
    if options.only.is_none_or(|area| area == Area::Device) {
        device::run(roots, out_dir, tick, &mut summary);
    }
    if options.only.is_none_or(|area| area == Area::Display) {
        display::run(roots, out_dir, tick, &options.modes(), &mut summary);
    }
    if options.only.is_none_or(|area| area == Area::GpuRank) {
        gpu_rank::run(roots, out_dir, &mut summary);
    }
    if options.only.is_none_or(|area| area == Area::Sensors) {
        sensors::run(roots, out_dir, &mut summary);
    }
    if options.only.is_none_or(|area| area == Area::Service) {
        service::run(roots, tick, &options.modes(), &mut summary);
    }
    if options.only.is_none_or(|area| area == Area::Music) {
        music::run(roots, out_dir, &mut summary);
    }
    if options.only.is_none_or(|area| area == Area::App) {
        app::run(roots, out_dir, &mut summary);
    }
    if options.only == Some(Area::Baseline) {
        baseline::run(roots, &mut summary);
    }
    if options.only == Some(Area::Package) {
        package::run(roots, out_dir, &mut summary);
    }
    if let Some(guard) = dev_interface {
        let closed = guard
            .close()
            .and_then(|note| match dev_interface::leftover(&roots.run) {
                None => Ok(note),
                Some(path) => Err(format!("{} is still there", path.display())),
            });
        summary.check("dev_interface_removed", closed);
    }
    summary
}
