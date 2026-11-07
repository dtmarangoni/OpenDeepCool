//! The headless weston the app's tests run in (Phase 10 plan, Task 3): it starts and stops,
//! two in one test get their own sockets, and a killed test leaves no compositor and no socket
//! that the next start keeps. Behind the `weston-tests` feature (solution review P4-05, P10-04).

#![cfg(feature = "weston-tests")]

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use odc_testkit::weston::Weston;

fn dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("weston")
        .join(name)
}

fn gone(pid: u32) -> bool {
    !Path::new(&format!("/proc/{pid}")).exists()
}

/// Waits up to 5 s for `done`.
fn eventually(done: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if done() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    done()
}

#[test]
fn it_starts_takes_connections_and_stops() {
    let weston = Weston::headless(&dir("start")).expect("weston starts");
    let socket = weston.display().to_owned();
    assert!(socket.is_absolute());
    assert!(UnixStream::connect(&socket).is_ok());
    let pid = weston.pid();
    drop(weston);
    assert!(!socket.exists(), "the socket is removed");
    assert!(eventually(|| gone(pid)), "weston stopped");
}

#[test]
fn two_in_one_test_get_their_own_sockets() {
    let one = Weston::headless(&dir("two")).expect("the first");
    let two = Weston::headless(&dir("two")).expect("the second");
    assert_ne!(one.display(), two.display());
    assert!(UnixStream::connect(one.display()).is_ok());
    assert!(UnixStream::connect(two.display()).is_ok());
    let first = one.display().to_owned();
    drop(one);
    assert!(!first.exists());
    assert!(
        UnixStream::connect(two.display()).is_ok(),
        "the other runs on"
    );
}

/// The child of the next test: it starts weston, says where, and waits to be killed.
#[test]
#[ignore = "the child process of a_killed_test_leaves_no_compositor_and_no_socket"]
fn holds_a_weston_until_killed() {
    let weston = Weston::headless(&dir("killed")).expect("weston starts");
    // On a line of its own: libtest's "test … ... " has no line end.
    println!(
        "\nheld weston {} {}",
        weston.pid(),
        weston.display().display()
    );
    thread::sleep(Duration::from_secs(30));
}

#[test]
fn a_killed_test_leaves_no_compositor_and_no_socket() {
    let mut child = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "holds_a_weston_until_killed",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the child starts");
    let stdout = child.stdout.take().expect("its stdout");
    let (pid, socket) = BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
        .find_map(|line| {
            let rest = line.strip_prefix("held weston ")?;
            let (pid, socket) = rest.split_once(' ')?;
            Some((pid.parse::<u32>().ok()?, PathBuf::from(socket)))
        })
        .expect("the child's weston");
    assert!(UnixStream::connect(&socket).is_ok());
    child.kill().expect("killed");
    child.wait().expect("reaped");
    assert!(eventually(|| gone(pid)), "weston died with the test");
    assert!(UnixStream::connect(&socket).is_err(), "nothing listens");
    let next = Weston::headless(&dir("killed")).expect("the next start");
    assert!(!socket.exists(), "the next start removed the dead socket");
    drop(next);
}
