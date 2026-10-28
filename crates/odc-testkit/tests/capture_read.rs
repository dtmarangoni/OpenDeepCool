//! The capture-log reader on real files: a good log reads back in order, and a malformed line
//! panics with its file and line number.

use std::fs;
use std::path::PathBuf;

use odc_testkit::capture::{self, Direction};

fn log(name: &str, text: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::write(&path, text).expect("the test's temporary folder is writable");
    path
}

#[test]
fn a_log_reads_back_in_order() {
    let path = log("good.log", "1 0 0 out 2 1068\n2 0 0 out 1 16\n");
    let messages: Vec<_> = capture::read(&path).collect();
    assert_eq!(messages.len(), 2);
    assert_eq!((messages[0].line, messages[0].unix_ns), (1, 1));
    assert_eq!(messages[0].bytes, [0x10, 0x68]);
    assert_eq!((messages[1].line, messages[1].dir), (2, Direction::Out));
}

#[test]
#[should_panic(expected = "malformed.log:2: 5 fields, not 6")]
fn a_malformed_line_panics_with_its_line_number() {
    let path = log("malformed.log", "1 0 0 out 1 10\n2 0 0 out 1\n");
    capture::read(&path).for_each(drop);
}

#[test]
#[should_panic(expected = "missing.log")]
fn a_missing_log_panics() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("missing.log");
    let _ = fs::remove_file(&path);
    capture::read(&path).for_each(drop);
}
