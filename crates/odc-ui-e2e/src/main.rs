//! `odc-ui-e2e`: drives the settings app through AT-SPI (Phase 10 plan, Task 8). `odc-e2e --only
//! app` starts it with the app already running in a headless weston against a fake service;
//! it runs every scenario and prints one line for each, `ok <name> <seconds> [<note>]` or
//! `FAILED <name> <seconds> <why>`, and on a failure writes the app's accessible tree beside
//! the run. Its options are flags, never environment variables (owner, 2026-10-24).

mod driver;
mod fake;
mod scenarios;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

const USAGE: &str = "usage: odc-ui-e2e --a11y-bus ADDRESS --fake DIR --app-log FILE \
--omarchy-state DIR --wayland-display SOCKET --out DIR [--only NAME,...]";

/// Where everything the run needs is.
pub struct Paths {
    /// The accessibility bus the app is on.
    pub a11y_bus: String,
    /// The fake service's folder.
    pub fake: PathBuf,
    /// The app's standard error.
    pub app_log: PathBuf,
    /// The folder the app reads its Omarchy theme from.
    pub omarchy_state: PathBuf,
    /// The headless weston's socket, for a window of another client.
    pub wayland_display: PathBuf,
    /// Where a failed scenario's tree is written.
    pub out: PathBuf,
}

fn parse(args: &[String]) -> Result<(Paths, Vec<String>), String> {
    let mut values: Vec<(&str, Option<String>)> = [
        "--a11y-bus",
        "--fake",
        "--app-log",
        "--omarchy-state",
        "--wayland-display",
        "--out",
        "--only",
    ]
    .into_iter()
    .map(|flag| (flag, None))
    .collect();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
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
        omarchy_state: need("--omarchy-state")?.into(),
        wayland_display: need("--wayland-display")?.into(),
        out: need("--out")?.into(),
    };
    let only = take("--only")
        .map(|list| list.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    Ok((paths, only))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (paths, only) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("odc-ui-e2e: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let mut failed = false;
    let mut run = match scenarios::Run::start(&paths) {
        Ok(run) => run,
        Err(e) => {
            println!("FAILED start 0.0 {e}");
            return ExitCode::FAILURE;
        }
    };
    for (name, scenario) in scenarios::ALL {
        if !only.is_empty() && !only.iter().any(|o| name.contains(o.as_str())) {
            continue;
        }
        let started = Instant::now();
        let result = scenario(&mut run);
        let seconds = started.elapsed().as_secs_f64();
        match result {
            Ok(note) if note.is_empty() => println!("ok {name} {seconds:.1}"),
            Ok(note) => println!("ok {name} {seconds:.1} {note}"),
            Err(why) => {
                failed = true;
                let tree = paths.out.join(format!("{name}.tree.txt"));
                let _ = std::fs::write(&tree, run.driver.dump());
                println!(
                    "FAILED {name} {seconds:.1} {why} (the tree: {})",
                    tree.display()
                );
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
