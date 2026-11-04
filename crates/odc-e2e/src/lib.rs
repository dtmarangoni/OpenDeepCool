//! `odc-e2e`: layer 4 of the tests, unattended (PRD, Testing Strategy; AGENTS.md rule 3). It
//! probes the PC it runs on: a device present and usable is tested for real, an absent one
//! through fakes, and one present but unusable fails with its fix. Its options are flags given at
//! start, never environment variables (owner, 2026-10-24). It never runs in `cargo test` or CI,
//! because it touches real hardware when present.

pub mod attended;
pub mod device;
pub mod gpu_rank;
pub mod load;
pub mod plays;
pub mod sensors;
pub mod service;
pub mod session;

use std::path::Path;
use std::time::Duration;

use odc_testkit::hardware::{Capability, Mode, Probe, Roots, Summary, probe};

pub const USAGE: &str = "usage: odc-e2e [--require <capability,...>] [--only <area>]
       odc-e2e --attended

  --require <list>  capabilities that must run real: lp360, lp360-reconnect,
                    cpu-temp, cpu-power, proc-stat, meminfo, gpu-amdgpu, gpu-i915,
                    gpu-xe, gpu-nvidia, gpu-other, board-fans, sensors-tool,
                    gpu-load-tool, dev-service
  --only <area>     run one area's checks: device, load, gpu-rank, sensors,
                    service
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
    Load,
    GpuRank,
    Sensors,
    /// The dev service's own checks (Phase 7 plan, Task 9).
    Service,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub require: Vec<Capability>,
    pub only: Option<Area>,
    /// The attended tour instead of the unattended checks (owner, 2026-10-24).
    pub attended: bool,
}

/// The options, or `None` for `--help`. An unknown flag, capability or area is an error.
pub fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut options = Options::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--attended" => options.attended = true,
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
            "--only" => {
                let area = args.next().ok_or("--only needs an area")?;
                options.only = Some(match area.as_str() {
                    "device" => Area::Device,
                    "load" => Area::Load,
                    "gpu-rank" => Area::GpuRank,
                    "sensors" => Area::Sensors,
                    "service" => Area::Service,
                    _ => return Err(format!("{area:?} isn't an area")),
                });
            }
            _ => return Err(format!("{arg:?} isn't an option")),
        }
    }
    Ok(Some(options))
}

/// Runs the chosen areas' checks one at a time, `tick` apart where they send frames (100 ms on
/// the real cooler, `protocol.md` Timing), and returns the summary.
pub fn run(options: &Options, roots: &Roots, out_dir: &Path, tick: Duration) -> Summary {
    let mut summary = Summary::default();
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
    if options.only.is_none_or(|area| area == Area::Load) {
        load::run(roots, out_dir, tick, &mut summary);
    }
    if options.only.is_none_or(|area| area == Area::GpuRank) {
        gpu_rank::run(roots, out_dir, &mut summary);
    }
    if options.only.is_none_or(|area| area == Area::Sensors) {
        sensors::run(roots, out_dir, &mut summary);
    }
    if options.only.is_none_or(|area| area == Area::Service) {
        service::run(roots, tick, &mut summary);
    }
    summary
}
