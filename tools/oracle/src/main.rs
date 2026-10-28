//! `odc-oracle` writes the expected frame of every test case, `docs/spec/frames/mode-001.txt` to
//! `mode-022.txt`, from the spec's rules alone. It's a dev-only tool, never shipped, and it shares
//! no code with OpenDeepCool, so the program is never compared with its own code (PRD, Testing
//! Strategy).

mod drawing;
mod frames;
mod rules;

use std::path::Path;
use std::process::ExitCode;
use std::{env, fs, io};

const USAGE: &str = "usage: odc-oracle <out-dir>

Writes the expected frames, mode-001.txt to mode-022.txt, into <out-dir>.
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
                println!("{cases} cases written to {out}");
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

/// Writes every file and returns the number of cases.
fn write(out: &Path) -> io::Result<usize> {
    fs::create_dir_all(out)?;
    let mut cases = 0;
    for (name, text) in frames::files() {
        cases += text.lines().filter(|l| l.starts_with("## ")).count();
        fs::write(out.join(name), text)?;
    }
    Ok(cases)
}
