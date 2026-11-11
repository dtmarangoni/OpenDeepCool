//! `odc-fake-pkexec`: the settings app's end-to-end run gives the app this program, through a
//! symbolic link in the fake systemd's folder, in place of `pkexec` (Phase 11 plan, Task 3). It
//! logs its arguments, then ends as the test's answer says
//! ([`odc_testkit::fake_systemd::fake_pkexec`]). It runs as the user and changes nothing.

use std::path::PathBuf;
use std::process::ExitCode;

use odc_testkit::fake_systemd::fake_pkexec;

fn main() -> ExitCode {
    let mut args = std::env::args_os();
    let argv0 = args.next().map(PathBuf::from).unwrap_or_default();
    let args: Vec<String> = args.map(|a| a.to_string_lossy().into_owned()).collect();
    match fake_pkexec(&argv0, &args) {
        Ok((code, stderr)) => {
            eprint!("{stderr}");
            ExitCode::from(u8::try_from(code).unwrap_or(1))
        }
        Err(e) => {
            eprintln!("odc-fake-pkexec: {e}");
            ExitCode::from(127)
        }
    }
}
