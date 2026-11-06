//! `opendeepcool-audio`, the session's sound helper (Phase 9 plan, Task 4): a user unit enabled
//! for every user, which hands the display service the default output's sound while it shows a
//! Music mode. Its only flag is `--target <node>`, for the end-to-end test; anything else is an
//! error. It ends with an error when the system bus goes, and its unit starts it again.

use std::process::ExitCode;

use odc_audio::{Config, Log, Priority, Stderr, parse_flags, run};

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
    let result = zbus::block_on(async {
        let conn = zbus::connection::Builder::system().build().await?;
        run(&conn, &config, &Stderr, std::future::pending()).await
    });
    match result {
        Ok(()) => Stderr.write(Priority::Error, "the system bus connection ended"),
        Err(e) => Stderr.write(Priority::Error, &format!("the system bus: {e}")),
    }
    ExitCode::FAILURE
}
