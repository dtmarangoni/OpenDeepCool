//! `Lp360` against the fake cooler (Phase 5 plan, Task 6): `03 08` first on every connection, a
//! rescan after any failure, the reasons, the new conversation on request, and the recording.

use std::path::PathBuf;

use odc_device::fake::{FakeConnector, Presence};
use odc_device::{DisplayPort, Link, Lp360, Reason, Recording};
use odc_protocol::{Frame, Message, decode};
use odc_testkit::capture;

/// A distinct picture for tick `n`: one lit pixel on the diagonal.
fn frame(n: usize) -> Frame {
    let mut f = Frame::blank();
    f.set(n, n, true);
    f
}

fn frames(ns: &[usize]) -> Vec<Message> {
    let mut messages = vec![Message::Startup];
    messages.extend(ns.iter().map(|&n| Message::Frame(frame(n))));
    messages
}

#[test]
fn absent_at_start_then_present_starts_with_03_08() {
    let fake = FakeConnector::new([Presence::Absent, Presence::Present]);
    let log = fake.log();
    let mut lp360 = Lp360::new(fake);
    assert_eq!(lp360.show(&frame(1)), Link::Disconnected(Reason::NotFound));
    assert!(
        log.connections().is_empty(),
        "nothing is written while absent"
    );
    assert_eq!(lp360.show(&frame(2)), Link::Connected);
    assert_eq!(log.connections(), [frames(&[2])]);
}

#[test]
fn a_steady_connection_sends_03_08_once() {
    let fake = FakeConnector::new([Presence::Present]);
    let log = fake.log();
    let mut lp360 = Lp360::new(fake);
    for n in 1..=3 {
        assert_eq!(lp360.show(&frame(n)), Link::Connected);
    }
    assert_eq!(log.connections(), [frames(&[1, 2, 3])]);
    assert_eq!(log.connect_calls(), 1);
}

#[test]
fn each_reason_is_reported_and_each_change_counted_once() {
    let path = PathBuf::from("/dev/hidraw7");
    let fake = FakeConnector::new([
        Presence::Absent,
        Presence::Absent,
        Presence::NoAccess(path.clone()),
        Presence::NoAccess(path.clone()),
        Presence::FailsAfter(1),
        Presence::Present,
    ]);
    let mut lp360 = Lp360::new(fake);
    let no_access = Link::Disconnected(Reason::NoAccess { path, errno: None });
    let expected = [
        (Link::Disconnected(Reason::NotFound), true),
        (Link::Disconnected(Reason::NotFound), false),
        (no_access.clone(), true),
        (no_access, false),
        (
            Link::Disconnected(Reason::WriteFailed { errno: None }),
            true,
        ),
        (Link::Connected, true),
        (Link::Connected, false),
    ];
    for (n, (link, changed)) in expected.into_iter().enumerate() {
        assert_eq!(lp360.show(&frame(n + 1)), link, "show {}", n + 1);
        assert_eq!(lp360.state_changed(), changed, "show {}", n + 1);
    }
}

#[test]
fn after_a_failed_write_the_next_show_reconnects_with_03_08() {
    let fake = FakeConnector::new([Presence::FailsAfter(3), Presence::Present]);
    let log = fake.log();
    let mut lp360 = Lp360::new(fake);
    assert_eq!(lp360.show(&frame(1)), Link::Connected);
    assert_eq!(lp360.show(&frame(2)), Link::Connected);
    assert_eq!(
        lp360.show(&frame(3)),
        Link::Disconnected(Reason::WriteFailed { errno: None })
    );
    assert_eq!(lp360.show(&frame(4)), Link::Connected);
    assert_eq!(log.connections(), [frames(&[1, 2]), frames(&[4])]);
}

#[test]
fn a_cooler_that_vanishes_and_returns_gets_03_08_first() {
    let fake = FakeConnector::new([
        Presence::FailsAfter(3),
        Presence::Absent,
        Presence::Absent,
        Presence::Absent,
        Presence::Present,
    ]);
    let log = fake.log();
    let mut lp360 = Lp360::new(fake);
    for n in 1..=2 {
        assert_eq!(lp360.show(&frame(n)), Link::Connected);
    }
    assert!(matches!(
        lp360.show(&frame(3)),
        Link::Disconnected(Reason::WriteFailed { .. })
    ));
    for n in 4..=6 {
        assert_eq!(lp360.show(&frame(n)), Link::Disconnected(Reason::NotFound));
    }
    assert_eq!(lp360.show(&frame(7)), Link::Connected);
    assert_eq!(log.connections(), [frames(&[1, 2]), frames(&[7])]);
}

#[test]
fn reset_connection_starts_a_new_conversation() {
    let fake = FakeConnector::new([Presence::Present]);
    let log = fake.log();
    let mut lp360 = Lp360::new(fake);
    assert_eq!(lp360.show(&frame(1)), Link::Connected);
    lp360.reset_connection();
    assert_eq!(lp360.show(&frame(2)), Link::Connected);
    assert!(!lp360.state_changed(), "the link stays connected");
    assert_eq!(log.connections(), [frames(&[1]), frames(&[2])]);
    assert_eq!(log.connect_calls(), 2);
}

#[test]
fn reset_connection_without_a_connection_changes_nothing() {
    let fake = FakeConnector::new([Presence::Present]);
    let log = fake.log();
    let mut lp360 = Lp360::new(fake);
    lp360.reset_connection();
    assert_eq!(log.connect_calls(), 0);
    assert_eq!(lp360.show(&frame(1)), Link::Connected);
    assert_eq!(log.connections(), [frames(&[1])]);
}

#[test]
fn the_recording_holds_exactly_the_successful_writes_in_order() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("reconnect-recording.log");
    let fake = FakeConnector::new([Presence::FailsAfter(3), Presence::Present]);
    let recording = Recording::create(fake, &path).expect("the recording file");
    let mut lp360 = Lp360::new(recording);
    for n in 1..=4 {
        lp360.show(&frame(n));
    }
    assert_eq!(lp360.connector().failure(), None);
    let recorded: Vec<Message> = capture::read(&path)
        .map(|c| {
            let bytes: [u8; 64] = c.bytes.as_slice().try_into().expect("64 bytes");
            decode(&bytes).expect("a documented message")
        })
        .collect();
    let mut expected = frames(&[1, 2]);
    expected.extend(frames(&[4]));
    assert_eq!(recorded, expected);
}
