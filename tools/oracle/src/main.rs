//! `odc-oracle` writes the expected frame of every test case, `docs/spec/frames/mode-001.txt` to
//! `mode-023.txt`, the dash cases `unavailable.txt` and the scenarios of modes 1–12,
//! `scenarios/mode-001.txt` to `mode-012.txt`, the spectrum's test signals and expected rows,
//! `spectrum/signals.txt` and `expected.txt`, and the Dancer's poses, `spectrum/dancer.txt`, from
//! the spec's rules alone. It's a dev-only tool, never shipped, and it shares no code with
//! OpenDeepCool, so the program is never compared with its own code (PRD, Testing Strategy).

mod dancer;
mod drawing;
mod frames;
mod movement;
mod rules;
mod scenarios;
mod spectrum;

use std::path::Path;
use std::process::ExitCode;
use std::{env, fs, io};

const USAGE: &str = "usage: odc-oracle <out-dir>

Writes the expected frames, mode-001.txt to mode-023.txt, unavailable.txt and
the scenarios, scenarios/mode-001.txt to mode-012.txt, and the spectrum's
spectrum/signals.txt, spectrum/expected.txt and spectrum/dancer.txt, into
<out-dir>.
The committed copies live in docs/spec/frames/.";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "-h" || flag == "--help" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        [out] if !out.starts_with('-') => match write(Path::new(out)) {
            Ok(cases) => {
                println!("{cases} cases and scenarios written to {out}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("odc-oracle: {out}: {e}");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Writes every file and returns the number of cases and scenarios.
fn write(out: &Path) -> io::Result<usize> {
    let mut cases = 0;
    for (name, text) in frames::files() {
        cases += text.lines().filter(|l| l.starts_with("## ")).count();
        let path = out.join(name);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, text)?;
    }
    Ok(cases)
}
