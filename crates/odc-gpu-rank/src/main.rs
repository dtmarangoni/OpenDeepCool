//! `odc-gpu-rank`: see the library's documentation. `opendeepcool-gpu-rank.service` starts it
//! before the display service, only while no ranking exists. A failed kind query is a warning in
//! the journal, never a failure: the GPU is then unknown (owner, 2026-10-27; solution review
//! P6-07).

use std::path::PathBuf;
use std::process::ExitCode;

use odc_gpu_rank::{DriverKinds, rank, write_ranking};
use odc_sensors::RankedKind;

const USAGE: &str = "usage: odc-gpu-rank [--state-dir <dir>] [--sys <dir>] [--dev <dir>]

Asks each GPU's driver whether the GPU is dedicated or integrated, writes the
ranking to gpu-rank.json in the state directory, and exits.

  --state-dir <dir>  where to write it (default /var/lib/opendeepcool)
  --sys <dir>        the sysfs root (default /sys), for tests
  --dev <dir>        the device root (default /dev), for tests
  -h, --help         this text";

/// Where to write, and the roots to look under.
struct Options {
    state_dir: PathBuf,
    sys: PathBuf,
    dev: PathBuf,
}

/// The options, or `None` for `--help`. An unknown flag or a flag without its value is an error.
fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut options = Options {
        state_dir: PathBuf::from("/var/lib/opendeepcool"),
        sys: PathBuf::from("/sys"),
        dev: PathBuf::from("/dev"),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--state-dir" => &mut options.state_dir,
            "--sys" => &mut options.sys,
            "--dev" => &mut options.dev,
            _ => return Err(format!("{arg:?} isn't an option")),
        };
        *slot = args
            .next()
            .ok_or_else(|| format!("{arg} needs a folder"))?
            .into();
    }
    Ok(Some(options))
}

/// Lines with `<N>` in front reach the journal at that level (sd-daemon(3)): 3 an error, 4 a
/// warning, 6 information.
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("odc-gpu-rank: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let ranking = rank(
        &options.sys,
        &mut DriverKinds::new(&options.sys, &options.dev),
    );
    for gpu in &ranking.gpus {
        let (level, kind) = match &gpu.kind {
            RankedKind::Dedicated { signal } => (6, format!("dedicated ({signal})")),
            RankedKind::Integrated { signal } => (6, format!("integrated ({signal})")),
            RankedKind::NoSignal => (6, "unknown: its driver has no kind signal".to_owned()),
            RankedKind::QueryFailed { error } => (4, format!("unknown: the query failed: {error}")),
        };
        eprintln!(
            "<{level}>odc-gpu-rank: {} ({}): {kind}",
            gpu.pci, gpu.driver
        );
    }
    match write_ranking(&options.state_dir, &ranking) {
        Ok(path) => {
            let count = match ranking.gpus.len() {
                1 => "1 GPU".to_owned(),
                n => format!("{n} GPUs"),
            };
            eprintln!("<6>odc-gpu-rank: {count} ranked in {}", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("<3>odc-gpu-rank: {}: {e}", options.state_dir.display());
            ExitCode::FAILURE
        }
    }
}
