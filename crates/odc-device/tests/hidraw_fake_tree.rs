//! The real hidraw adapter against fake sysfs trees (Phase 5 plan, Task 7): it finds the LP360 by
//! USB ID under any node name. A regular file stands in for each `/dev/hidrawN`. Every other
//! device and name is made up (AGENTS.md rule 10).

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use odc_device::{
    Connector, DisplayPort, HidrawConnector, Link, Lp360, OpenError, Reason, Transport,
};
use odc_protocol::{Frame, Message, Report, decode};

const LP360: &str = "0003:00003633:0000000C";

/// A fake `/sys` and `/dev` in a folder of its own.
struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("hidraw-trees")
            .join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("sys/class/hidraw")).expect("a fake /sys");
        fs::create_dir_all(root.join("dev")).expect("a fake /dev");
        Tree { root }
    }

    fn connector(&self) -> HidrawConnector {
        HidrawConnector::new(self.root.join("sys"), self.root.join("dev"))
    }

    fn node_path(&self, number: u32) -> PathBuf {
        self.root.join(format!("dev/hidraw{number}"))
    }

    /// A hidraw node whose device's uevent says `hid_id`, and its stand-in device file.
    fn node(&self, number: u32, hid_id: &str) {
        let device = self
            .root
            .join(format!("sys/class/hidraw/hidraw{number}/device"));
        fs::create_dir_all(&device).expect("a node's device folder");
        let uevent = format!("DRIVER=hid-generic\nHID_ID={hid_id}\nHID_NAME=Made-up Device\n");
        fs::write(device.join("uevent"), uevent).expect("a uevent");
        fs::write(self.node_path(number), b"").expect("a stand-in node");
    }

    fn remove_node(&self, number: u32) {
        fs::remove_dir_all(self.root.join(format!("sys/class/hidraw/hidraw{number}")))
            .expect("the node's sysfs entry");
        fs::remove_file(self.node_path(number)).expect("the stand-in node");
    }

    /// The messages written to a stand-in node.
    fn written(&self, number: u32) -> Vec<Message> {
        let bytes = fs::read(self.node_path(number)).expect("a stand-in node");
        let (reports, rest) = bytes.as_chunks::<64>();
        assert!(rest.is_empty(), "whole reports only");
        reports
            .iter()
            .map(|r| decode(r).expect("a documented message"))
            .collect()
    }
}

fn frame(n: usize) -> Frame {
    let mut f = Frame::blank();
    f.set(n, 15 - n, true);
    f
}

#[test]
fn no_hidraw_class_finds_nothing() {
    let tree = Tree::new("no-class");
    fs::remove_dir_all(tree.root.join("sys/class/hidraw")).expect("the class folder");
    assert!(matches!(tree.connector().connect(), Ok(None)));
}

#[test]
fn other_devices_alone_find_nothing() {
    let tree = Tree::new("others");
    tree.node(0, "0003:0000ABCD:00001234");
    tree.node(1, "0005:00001111:00002222");
    assert!(matches!(tree.connector().connect(), Ok(None)));
}

#[test]
fn the_lp360_among_others_gets_03_08_and_the_frame() {
    let tree = Tree::new("among-others");
    tree.node(0, "0003:0000ABCD:00001234");
    tree.node(3, "0003:00001111:00002222");
    tree.node(5, LP360);
    tree.node(8, "0018:0000BEEF:00000001");
    let mut lp360 = Lp360::new(tree.connector());
    assert_eq!(lp360.show(&frame(1)), Link::Connected);
    assert_eq!(
        tree.written(5),
        [Message::Startup, Message::Frame(frame(1))]
    );
    assert!(tree.written(3).is_empty() && tree.written(8).is_empty());
}

#[test]
fn ids_in_lower_case_are_found() {
    let tree = Tree::new("lower-case");
    tree.node(2, "0003:00003633:0000000c");
    assert!(matches!(tree.connector().connect(), Ok(Some(_))));
}

#[test]
fn hidraw9_comes_before_hidraw10() {
    let tree = Tree::new("numeric-order");
    tree.node(10, LP360);
    tree.node(9, LP360);
    let mut lp360 = Lp360::new(tree.connector());
    assert_eq!(lp360.show(&frame(1)), Link::Connected);
    assert_eq!(tree.written(9).len(), 2);
    assert!(tree.written(10).is_empty());
}

#[test]
fn the_vendor_alone_or_the_product_alone_is_not_an_lp360() {
    let tree = Tree::new("unrelated-ids");
    tree.node(1, "0003:00003633:0000000D");
    tree.node(2, "0003:00003634:0000000C");
    tree.node(3, "0005:00003633:0000000C");
    assert!(matches!(tree.connector().connect(), Ok(None)));
}

#[test]
fn an_entry_without_a_readable_uevent_is_skipped() {
    let tree = Tree::new("no-uevent");
    fs::create_dir_all(tree.root.join("sys/class/hidraw/hidraw0")).expect("a bare entry");
    fs::write(tree.root.join("sys/class/hidraw/notes"), b"").expect("a stray file");
    tree.node(4, LP360);
    let mut lp360 = Lp360::new(tree.connector());
    assert_eq!(lp360.show(&frame(1)), Link::Connected);
    assert_eq!(tree.written(4).len(), 2);
}

/// A node that can't be opened is "no access", naming the node. The stand-in is a folder: opening
/// a folder for writing fails for every account, root included, which CI runs as.
#[test]
fn a_node_that_cant_be_opened_is_no_access() {
    let tree = Tree::new("no-access");
    tree.node(4, LP360);
    fs::remove_file(tree.node_path(4)).expect("the stand-in node");
    fs::create_dir(tree.node_path(4)).expect("a folder in its place");
    match tree.connector().connect() {
        Err(OpenError { path, .. }) => assert_eq!(path, tree.node_path(4)),
        other => panic!(
            "expected an open error, got {:?}",
            other.map(|t| t.is_some())
        ),
    }
    let mut lp360 = Lp360::new(tree.connector());
    match lp360.show(&frame(1)) {
        Link::Disconnected(Reason::NoAccess { path, errno }) => {
            assert_eq!(path, tree.node_path(4));
            assert!(errno.is_some(), "the kernel's error number");
        }
        other => panic!("expected no access, got {other:?}"),
    }
}

/// Wraps the real connector so a test can make the next write fail.
struct FailNext<C> {
    inner: C,
    fail: Arc<AtomicBool>,
}

struct FailNextTransport<T> {
    inner: T,
    fail: Arc<AtomicBool>,
}

impl<C: Connector> Connector for FailNext<C> {
    type Transport = FailNextTransport<C::Transport>;

    fn connect(&mut self) -> Result<Option<Self::Transport>, OpenError> {
        Ok(self.inner.connect()?.map(|inner| FailNextTransport {
            inner,
            fail: Arc::clone(&self.fail),
        }))
    }
}

impl<T: Transport> Transport for FailNextTransport<T> {
    fn write_report(&mut self, report: &Report) -> io::Result<()> {
        if self.fail.swap(false, Ordering::Relaxed) {
            return Err(io::Error::other("injected write failure"));
        }
        self.inner.write_report(report)
    }
}

#[test]
fn a_moved_node_is_found_again_after_a_write_error() {
    let tree = Tree::new("moved");
    tree.node(1, LP360);
    let fail = Arc::new(AtomicBool::new(false));
    let mut lp360 = Lp360::new(FailNext {
        inner: tree.connector(),
        fail: Arc::clone(&fail),
    });
    assert_eq!(lp360.show(&frame(1)), Link::Connected);
    assert_eq!(tree.written(1).len(), 2);
    tree.remove_node(1);
    tree.node(6, LP360);
    fail.store(true, Ordering::Relaxed);
    assert!(matches!(
        lp360.show(&frame(2)),
        Link::Disconnected(Reason::WriteFailed { .. })
    ));
    assert_eq!(lp360.show(&frame(3)), Link::Connected);
    assert_eq!(
        tree.written(6),
        [Message::Startup, Message::Frame(frame(3))]
    );
}
