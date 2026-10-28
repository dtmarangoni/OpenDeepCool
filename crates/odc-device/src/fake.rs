//! A fake cooler for tests, this crate's and Phase 7's: a script of presences, and a shared log
//! of what each connection received.

use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use odc_protocol::{Message, Report, decode};

use crate::lp360::{Connector, OpenError, Transport};

/// One scripted answer to `connect`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    /// No cooler.
    Absent,
    /// A cooler whose device node can't be opened.
    NoAccess(PathBuf),
    /// A cooler whose connection takes every write.
    Present,
    /// A cooler whose connection takes `n` writes, then fails every write.
    FailsAfter(usize),
}

/// Answers each `connect` with the next presence of its script; the last one repeats.
pub struct FakeConnector {
    script: VecDeque<Presence>,
    last: Presence,
    log: FakeLog,
}

impl FakeConnector {
    /// # Panics
    ///
    /// When the script is empty.
    pub fn new(script: impl IntoIterator<Item = Presence>) -> Self {
        let script: VecDeque<Presence> = script.into_iter().collect();
        let last = script
            .back()
            .cloned()
            .expect("a script of at least one presence");
        FakeConnector {
            script,
            last,
            log: FakeLog::default(),
        }
    }

    /// A handle on what each connection received, for the test to read.
    pub fn log(&self) -> FakeLog {
        self.log.clone()
    }
}

impl Connector for FakeConnector {
    type Transport = FakeTransport;

    fn connect(&mut self) -> Result<Option<FakeTransport>, OpenError> {
        self.log.connects.fetch_add(1, Ordering::Relaxed);
        let presence = self.script.pop_front().unwrap_or_else(|| self.last.clone());
        let writes_left = match presence {
            Presence::Absent => return Ok(None),
            Presence::NoAccess(path) => {
                return Err(OpenError {
                    path,
                    error: io::Error::from(io::ErrorKind::PermissionDenied),
                });
            }
            Presence::Present => None,
            Presence::FailsAfter(n) => Some(n),
        };
        let mut connections = self.log.connections.lock().expect("the fake's log");
        connections.push(Vec::new());
        Ok(Some(FakeTransport {
            index: connections.len() - 1,
            writes_left,
            log: self.log.clone(),
        }))
    }
}

/// One fake connection.
pub struct FakeTransport {
    index: usize,
    writes_left: Option<usize>,
    log: FakeLog,
}

impl Transport for FakeTransport {
    fn write_report(&mut self, report: &Report) -> io::Result<()> {
        match &mut self.writes_left {
            Some(0) => return Err(io::Error::other("the fake cooler stopped taking writes")),
            Some(n) => *n -= 1,
            None => {}
        }
        self.log.connections.lock().expect("the fake's log")[self.index].push(report.clone());
        Ok(())
    }
}

/// What the fake's connections received, shared with the test.
#[derive(Clone, Default)]
pub struct FakeLog {
    connections: Arc<Mutex<Vec<Vec<Report>>>>,
    connects: Arc<AtomicUsize>,
}

impl FakeLog {
    /// The messages each connection received, in order, one list per connection.
    pub fn connections(&self) -> Vec<Vec<Message>> {
        self.connections
            .lock()
            .expect("the fake's log")
            .iter()
            .map(|reports| {
                reports
                    .iter()
                    .map(|r| decode(r.as_bytes()).expect("only encoder output reaches the fake"))
                    .collect()
            })
            .collect()
    }

    /// How many times `connect` was called.
    pub fn connect_calls(&self) -> usize {
        self.connects.load(Ordering::Relaxed)
    }
}
