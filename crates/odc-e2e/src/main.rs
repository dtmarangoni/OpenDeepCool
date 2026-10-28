//! `odc-e2e`: the unattended end-to-end checks; see the library's documentation.

use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use odc_testkit::hardware::Roots;

/// One frame per 100 ms tick (`docs/spec/protocol.md`, Timing).
const TICK: Duration = Duration::from_millis(100);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = match odc_e2e::parse_args(&args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{}", odc_e2e::USAGE);
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("odc-e2e: {e}\n\n{}", odc_e2e::USAGE);
            return ExitCode::from(2);
        }
    };
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/e2e");
    let summary = odc_e2e::run(&options, &Roots::system(), &out_dir, TICK);
    print!("{}", summary.table());
    if summary.passed() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
