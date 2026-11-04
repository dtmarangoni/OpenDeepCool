//! A panic in the engine thread ends the whole process with SIGABRT, so systemd restarts the
//! service instead of leaving it alive with a dead tick (Phase 7 plan, Task 6; solution review
//! OV-03). The test runs itself again as a child, with `--panic-child`, and watches it die. The
//! child turns its own core dump off first, so the expected abort leaves no core file and raises
//! no crash notification (owner, 2026-11-04); it still dies by SIGABRT.

use std::os::unix::process::ExitStatusExt;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::thread;

use odc_protocol::Frame;
use odc_sensors::{Exact, Fan, Reading, Snapshot};
use odc_service::engine::{Apply, Engine, Published, Requests};
use odc_service::runtime::{Outputs, Progress, Slot, SystemTicker, abort_on_panic, run_engine};
use odc_service::settings::Settings;

/// Linux's SIGABRT.
const SIGABRT: i32 = 6;

/// An engine thread's outputs that panic on the first frame, as a renderer bug would.
struct Panics;

impl Outputs for Panics {
    fn frame(&mut self, _: u64, _: Frame) {
        panic!("a renderer that panics");
    }
    fn requests(&mut self, _: Requests) {}
    fn status(&mut self, _: &Engine) {}
    fn notify(&mut self, _: &str) {}
    fn skipped(&mut self, _: u64, _: u64) {}
}

fn child() {
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
        .expect("the child's core dump turned off");
    abort_on_panic();
    let engine = thread::spawn(|| {
        let snapshot = Snapshot::new(
            |_| Reading::Value(Exact::whole(42)),
            Fan::NotChosen,
            Fan::NotChosen,
        );
        let engine = Engine::new(Settings::default(), snapshot);
        let progress = Progress::new(std::time::Instant::now());
        run_engine(
            engine,
            &SystemTicker,
            &Slot::<Apply>::default(),
            &Slot::<Published>::default(),
            &progress,
            &mut Panics,
            &AtomicBool::new(false),
            Some(5),
        );
    });
    let _ = engine.join();
    // Only reached if the panic didn't end the process.
    std::process::exit(0);
}

fn main() {
    if std::env::args().any(|arg| arg == "--panic-child") {
        child();
        return;
    }
    let exe = std::env::current_exe().expect("the test's own program");
    let out = Command::new(exe)
        .arg("--panic-child")
        .output()
        .expect("the child runs");
    assert_eq!(
        out.status.signal(),
        Some(SIGABRT),
        "the child ended with {:?}",
        out.status
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("<2>"),
        "a critical journal line: {stderr}"
    );
    assert!(stderr.contains("a renderer that panics"), "{stderr}");
    println!("test a_panic_in_the_engine_thread_aborts_the_service ... ok");
}
