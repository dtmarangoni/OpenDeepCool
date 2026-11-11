//! `opendeepcool-boot on|off`, started by `pkexec` (see the library). Every refusal and failure
//! names its cause on standard error and exits non-zero: 2 for a refusal, before anything runs,
//! and 1 for a failed check or step.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::process::ExitCode;

use odc_boot::{FAILED, Programs, REFUSED, USAGE, check, effective_uid, run};

fn main() -> ExitCode {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let euid = match fs::read_to_string("/proc/self/status")
        .ok()
        .as_deref()
        .and_then(effective_uid)
    {
        Some(euid) => euid,
        None => {
            eprintln!("opendeepcool-boot: /proc/self/status gives no effective user ID: refused");
            return ExitCode::from(REFUSED);
        }
    };
    let request = match check(euid, env::var_os("PKEXEC_UID").as_deref(), &args) {
        Ok(request) => request,
        Err(refusal) => {
            eprintln!("opendeepcool-boot: {refusal}\n{USAGE}");
            return ExitCode::from(REFUSED);
        }
    };
    match run(request, &Programs::system()) {
        Ok(done) => {
            println!("opendeepcool-boot: {done}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("opendeepcool-boot: {e}");
            ExitCode::from(FAILED)
        }
    }
}
