//! `opendeepcool-audio`, the session's sound helper (Phase 9 plan, Task 4): a user unit enabled
//! for every user, which hands the display service the default output's sound while it shows a
//! Music mode. Its only flag is `--target <node>`, for the end-to-end test; anything else is an
//! error. It ends with an error when the system bus goes, or when its program file is replaced or
//! removed, and its unit starts it again.

use std::process::ExitCode;

use odc_audio::{Config, Ending, Log, Priority, Stderr, helper, parse_flags};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let target = match parse_flags(&args) {
        Ok(target) => target,
        Err(e) => {
            Stderr.write(Priority::Error, &e);
            return ExitCode::from(2);
        }
    };
    let config = Config::new(target);
    // Its own program file, watched so an upgrade's new one takes over (Phase 11 plan, Task 4).
    let program = match std::env::current_exe() {
        Ok(program) => Some(program),
        Err(e) => {
            Stderr.write(
                Priority::Warning,
                &format!("its program file can't be found: {e}"),
            );
            None
        }
    };
    let ending = zbus::block_on(async {
        match zbus::connection::Builder::system().build().await {
            Ok(conn) => helper(&conn, &config, &Stderr, program.as_deref()).await,
            Err(e) => Ending::BusError(e.to_string()),
        }
    });
    // An upgrade's replacement is routine: a notice. Every ending is a failure for the unit,
    // which starts the helper again.
    let (priority, why) = match ending {
        Ending::ProgramReplaced => (
            Priority::Notice,
            "its program file was replaced or removed: ending, so its unit starts the new one"
                .to_owned(),
        ),
        Ending::BusEnded => (
            Priority::Error,
            "the system bus connection ended".to_owned(),
        ),
        Ending::BusError(e) => (Priority::Error, format!("the system bus: {e}")),
    };
    Stderr.write(priority, &why);
    ExitCode::FAILURE
}
