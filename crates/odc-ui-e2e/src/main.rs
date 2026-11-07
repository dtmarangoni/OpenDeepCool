//! `odc-ui-e2e`: drives the settings app through AT-SPI (Phase 10 plan, Task 8). `odc-e2e --only
//! app` starts it with the app already running in a headless weston against a fake service;
//! it runs every scenario and prints one line for each, `ok <name> <seconds> [<note>]` or
//! `FAILED <name> <seconds> <why>`, and on a failure writes the app's accessible tree beside
//! the run. Its options are flags, never environment variables (owner, 2026-10-24).

mod driver;
mod fake;
mod real;
mod scenarios;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

const USAGE: &str = "usage: odc-ui-e2e --a11y-bus ADDRESS --fake DIR --app-log FILE \
--frame-log FILE --omarchy-state DIR --wayland-display SOCKET --out DIR [--only NAME,...] \
[--real]";

/// Where everything the run needs is.
pub struct Paths {
    /// The accessibility bus the app is on.
    pub a11y_bus: String,
    /// The fake service's folder.
    pub fake: PathBuf,
    /// The app's standard error.
    pub app_log: PathBuf,
    /// The frames the app logs (its `--frame-log`).
    pub frame_log: PathBuf,
    /// The folder the app reads its Omarchy theme from.
    pub omarchy_state: PathBuf,
    /// The headless weston's socket, for a window of another client.
    pub wayland_display: PathBuf,
    /// Where a failed scenario's tree is written.
    pub out: PathBuf,
}

/// What the flags ask: where everything is, the scenarios to run (all when empty), and whether
/// the run is the real one.
struct Asked {
    paths: Paths,
    only: Vec<String>,
    real: bool,
}

fn parse(args: &[String]) -> Result<Asked, String> {
    let mut values: Vec<(&str, Option<String>)> = [
        "--a11y-bus",
        "--fake",
        "--app-log",
        "--frame-log",
        "--omarchy-state",
        "--wayland-display",
        "--out",
        "--only",
    ]
    .into_iter()
    .map(|flag| (flag, None))
    .collect();
    let mut real = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--real" {
            if real {
                return Err("--real given twice".to_owned());
            }
            real = true;
            continue;
        }
        let slot = values
            .iter_mut()
            .find(|(flag, _)| flag == arg)
            .ok_or_else(|| format!("unknown argument {arg:?}"))?;
        let value = rest.next().ok_or_else(|| format!("{arg} needs a value"))?;
        if slot.1.replace(value.clone()).is_some() {
            return Err(format!("{arg} given twice"));
        }
    }
    let mut take = |flag: &str| {
        values
            .iter_mut()
            .find(|(f, _)| *f == flag)
            .and_then(|(_, v)| v.take())
    };
    let mut need = |flag: &str| take(flag).ok_or_else(|| format!("{flag} is needed"));
    let paths = Paths {
        a11y_bus: need("--a11y-bus")?,
        fake: need("--fake")?.into(),
        app_log: need("--app-log")?.into(),
        frame_log: need("--frame-log")?.into(),
        omarchy_state: need("--omarchy-state")?.into(),
        wayland_display: need("--wayland-display")?.into(),
        out: need("--out")?.into(),
    };
    let only = take("--only")
        .map(|list| list.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    Ok(Asked { paths, only, real })
}

/// A scenario of either run, by name: it gives a note, or why it failed.
type Named<R> = (&'static str, fn(&mut R) -> driver::Result<String>);

/// Runs each scenario, prints its line, and writes the tree of each that fails.
fn report<R>(
    run: &mut R,
    scenarios: &[Named<R>],
    only: &[String],
    out: &std::path::Path,
    dump: impl Fn(&R) -> String,
) -> bool {
    let mut passed = true;
    for (name, scenario) in scenarios {
        if !only.is_empty() && !only.iter().any(|o| name.contains(o.as_str())) {
            continue;
        }
        let started = Instant::now();
        let result = scenario(run);
        let seconds = started.elapsed().as_secs_f64();
        match result {
            Ok(note) if note.is_empty() => println!("ok {name} {seconds:.1}"),
            Ok(note) => println!("ok {name} {seconds:.1} {note}"),
            Err(why) => {
                passed = false;
                let tree = out.join(format!("{name}.tree.txt"));
                let _ = std::fs::write(&tree, dump(run));
                println!(
                    "FAILED {name} {seconds:.1} {why} (the tree: {})",
                    tree.display()
                );
            }
        }
    }
    passed
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let asked = match parse(&args) {
        Ok(asked) => asked,
        Err(e) => {
            eprintln!("odc-ui-e2e: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let passed = if asked.real {
        match driver::Driver::connect(&asked.paths.a11y_bus) {
            Ok(mut driver) => report(
                &mut driver,
                &real::ALL,
                &asked.only,
                &asked.paths.out,
                driver::Driver::dump,
            ),
            Err(e) => {
                println!("FAILED start 0.0 {e}");
                false
            }
        }
    } else {
        match scenarios::Run::start(&asked.paths) {
            Ok(mut run) => report(
                &mut run,
                &scenarios::ALL,
                &asked.only,
                &asked.paths.out,
                |run| run.driver.dump(),
            ),
            Err(e) => {
                println!("FAILED start 0.0 {e}");
                false
            }
        }
    };
    if passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
