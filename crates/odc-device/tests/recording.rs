//! The dev copy's recording (Phase 7 plan, Task 7; solution review P7-07, P7-13): the first 100
//! reports after the start, then nothing until a test session, then every report; each with its
//! tick and its monotonic write time; buffered, flushed at a session's end; rotated, keeping one
//! previous file. The fake cooler takes every write.

use std::fs;
use std::path::PathBuf;

use odc_device::fake::{FakeConnector, Presence};
use odc_device::{DisplayPort, Lp360, Recording};
use odc_protocol::Frame;
use odc_testkit::capture::{Recorded, read_recording};

fn path(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("recording");
    fs::create_dir_all(&dir).expect("a directory");
    let path = dir.join(format!("{name}.log"));
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(format!("{}.1", path.display()));
    path
}

fn read(path: &PathBuf) -> Vec<Recorded> {
    read_recording(path).collect()
}

#[test]
fn the_first_100_reports_then_only_what_a_session_holds() {
    let path = path("gated");
    let recording = Recording::dev(FakeConnector::new([Presence::Present]), &path, u64::MAX)
        .expect("a recording");
    let handle = recording.handle();
    let mut lp360 = Lp360::new(recording);
    // 03 08 and 149 frames: the first 100 reports are kept.
    for tick in 0..149 {
        handle.set_tick(tick);
        lp360.show(&Frame::blank());
    }
    handle.flush();
    let start = read(&path);
    assert_eq!(start.len(), 100);
    assert_eq!(
        (start[0].tick, start[99].tick),
        (0, 98),
        "03 08 shares tick 0"
    );
    assert!(
        start.windows(2).all(|w| w[0].ns <= w[1].ns),
        "monotonic times"
    );
    // A session: every report, with its tick, flushed when it ends.
    handle.begin_session();
    for tick in 149..159 {
        handle.set_tick(tick);
        lp360.show(&Frame::blank());
    }
    handle.end_session();
    let after = read(&path);
    assert_eq!(after.len(), 110);
    let ticks: Vec<u64> = after[100..].iter().map(|r| r.tick).collect();
    assert_eq!(ticks, (149..159).collect::<Vec<_>>());
    // After the session: nothing again.
    lp360.show(&Frame::blank());
    handle.flush();
    assert_eq!(read(&path).len(), 110);
    assert_eq!(lp360.connector().failure(), None);
}

#[test]
fn the_recording_rotates_keeping_one_previous_file() {
    let path = path("rotated");
    // A line is 140 bytes or so: rotate after 11 or so of them.
    let recording =
        Recording::dev(FakeConnector::new([Presence::Present]), &path, 1500).expect("a recording");
    let handle = recording.handle();
    let mut lp360 = Lp360::new(recording);
    for tick in 0..35 {
        handle.set_tick(tick);
        lp360.show(&Frame::blank());
    }
    handle.flush();
    let previous = PathBuf::from(format!("{}.1", path.display()));
    let (old, new) = (read(&previous), read(&path));
    assert!((5..=12).contains(&old.len()), "{}", old.len());
    assert!(new.len() <= 12, "{}", new.len());
    let last_old = old.last().expect("a line").tick;
    assert!(
        new.first().is_none_or(|r| r.tick > last_old),
        "in order across files"
    );
    assert!(36 - old.len() - new.len() >= 10, "older files are not kept");
}

#[test]
fn a_test_recording_keeps_every_report_at_once() {
    let path = path("all");
    let recording =
        Recording::create(FakeConnector::new([Presence::Present]), &path).expect("a recording");
    let handle = recording.handle();
    let mut lp360 = Lp360::new(recording);
    for tick in 0..150 {
        handle.set_tick(tick);
        lp360.show(&Frame::blank());
    }
    // Written at once, no flush needed.
    assert_eq!(read(&path).len(), 151);
}
