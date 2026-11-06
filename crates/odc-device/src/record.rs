//! The recording: the reports actually written, so the tests compare what reached the cooler
//! with the expected frames (AGENTS.md rule 3). One line per report,
//! `<CLOCK_MONOTONIC ns> <tick> out 64 <hex>`, which `odc_testkit::capture::read_recording`
//! reads. The tick is the engine's, set through a [`RecordingHandle`] before each frame (Phase 7
//! plan, Task 7; solution review P7-13). The time is the kernel's monotonic clock itself, the same
//! in every process, so a test sets its own events against the frames: the Music delay (owner,
//! 2026-11-05; Phase 9 plan, Task 5).

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use odc_protocol::{Report, startup_report};

use crate::lp360::{Connector, OpenError, Transport};

/// `CLOCK_MONOTONIC` now, in ns: the kernel's clock from boot, which every process reads alike
/// and a suspend doesn't advance (clock_gettime(2)).
pub fn monotonic_ns() -> u64 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    let secs = u64::try_from(now.tv_sec).expect("the monotonic clock is never negative");
    let nanos = u64::try_from(now.tv_nsec).expect("0 to 999 999 999");
    secs * 1_000_000_000 + nanos
}

/// Which reports a recording keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keep {
    /// Every report, each line written at once: the tests' own recordings.
    All,
    /// The dev copy's (owner, 2026-10-27; solution review P7-07): the first `first` reports
    /// after the start, for the start check, and every report while a test session is held;
    /// buffered, flushed once a second and at a session's end.
    StartAndSessions { first: u64 },
}

/// After a session that released the cooler: the recording keeps the service's new `03 08` and
/// the report after it, then stops (owner, 2026-11-04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Return {
    No,
    /// Waiting for the new conversation's `03 08`.
    Startup,
    /// `03 08` recorded: the next report is the last.
    FirstFrame,
}

/// The dev copy's recording rotates at this size, keeping one previous file.
pub const ROTATE_AT: u64 = 64 * 1024 * 1024;

/// Wraps any [`Connector`]: each successful write through its connections is recorded as
/// [`Keep`] says.
pub struct Recording<C> {
    inner: C,
    log: Arc<Mutex<Log>>,
}

struct Log {
    path: PathBuf,
    out: BufWriter<File>,
    /// The first failed write to the recording; nothing more is recorded after it.
    failure: Option<String>,
    keep: Keep,
    /// The tick of the frame being written.
    tick: u64,
    /// Reports written to the cooler since the start, recorded or not.
    reports: u64,
    session: bool,
    returning: Return,
    size: u64,
    rotate_at: u64,
    flushed: Instant,
}

fn lock(log: &Mutex<Log>) -> MutexGuard<'_, Log> {
    log.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Log {
    fn fail(&mut self, e: &io::Error) {
        if self.failure.is_none() {
            self.failure = Some(format!("{}: {e}", self.path.display()));
        }
    }

    fn flush(&mut self) {
        if let Err(e) = self.out.flush() {
            self.fail(&e);
        }
        self.flushed = Instant::now();
    }

    /// Renames the file to `<path>.1`, replacing an older one, and starts a new file.
    fn rotate(&mut self) -> io::Result<()> {
        self.out.flush()?;
        let previous = PathBuf::from(format!("{}.1", self.path.display()));
        fs::rename(&self.path, previous)?;
        self.out = BufWriter::new(File::create(&self.path)?);
        self.size = 0;
        Ok(())
    }

    fn record(&mut self, report: &Report) {
        let first = self.reports == 0;
        self.reports += 1;
        let kept = match self.keep {
            Keep::All => true,
            Keep::StartAndSessions { first: n } => {
                self.reports <= n || self.session || self.returning != Return::No
            }
        };
        if !kept || self.failure.is_some() {
            return;
        }
        let ns = monotonic_ns();
        let hex: String = report
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let line = format!("{ns} {} out 64 {hex}\n", self.tick);
        if let Err(e) = self.out.write_all(line.as_bytes()) {
            self.fail(&e);
            return;
        }
        self.size += line.len() as u64;
        let returned = match self.returning {
            Return::Startup if *report == startup_report() => {
                self.returning = Return::FirstFrame;
                false
            }
            Return::FirstFrame => {
                self.returning = Return::No;
                true
            }
            _ => false,
        };
        if self.keep == Keep::All || first || returned {
            self.flush();
        }
        if self.size >= self.rotate_at
            && let Err(e) = self.rotate()
        {
            self.fail(&e);
        }
    }
}

impl<C> Recording<C> {
    /// Records every report into `path`, created or emptied.
    pub fn create(inner: C, path: impl AsRef<Path>) -> io::Result<Self> {
        Recording::with(inner, path, Keep::All, u64::MAX)
    }

    /// The dev copy's recording into `path`: the first 100 reports, and every report while a
    /// test session is held, rotated at `rotate_at` bytes ([`ROTATE_AT`] in the service).
    pub fn dev(inner: C, path: impl AsRef<Path>, rotate_at: u64) -> io::Result<Self> {
        Recording::with(
            inner,
            path,
            Keep::StartAndSessions { first: 100 },
            rotate_at,
        )
    }

    fn with(inner: C, path: impl AsRef<Path>, keep: Keep, rotate_at: u64) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let out = BufWriter::new(File::create(&path)?);
        let now = Instant::now();
        Ok(Recording {
            inner,
            log: Arc::new(Mutex::new(Log {
                path,
                out,
                failure: None,
                keep,
                tick: 0,
                reports: 0,
                session: false,
                returning: Return::No,
                size: 0,
                rotate_at,
                flushed: now,
            })),
        })
    }

    /// The handle the device thread and the dev interface keep.
    pub fn handle(&self) -> RecordingHandle {
        RecordingHandle {
            log: Arc::clone(&self.log),
        }
    }

    /// The first error writing the recording, if any. A test fails on it: an incomplete recording
    /// proves nothing.
    pub fn failure(&self) -> Option<String> {
        lock(&self.log).failure.clone()
    }
}

/// Sets the tick, starts and ends a test session's recording, and flushes.
#[derive(Clone)]
pub struct RecordingHandle {
    log: Arc<Mutex<Log>>,
}

impl RecordingHandle {
    /// The tick of the frame about to be written.
    pub fn set_tick(&self, tick: u64) {
        lock(&self.log).tick = tick;
    }

    pub fn begin_session(&self) {
        lock(&self.log).session = true;
    }

    /// Ends the session's recording, and flushes it.
    pub fn end_session(&self) {
        let mut log = lock(&self.log);
        log.session = false;
        log.returning = Return::No;
        log.flush();
    }

    /// Ends the session's recording of a session that released the cooler, but keeps the new
    /// conversation's `03 08` and the report after it, so a test sees the service take the cooler
    /// back (owner, 2026-11-04); that report is flushed at once.
    pub fn end_session_keeping_return(&self) {
        let mut log = lock(&self.log);
        log.session = false;
        log.returning = Return::Startup;
        log.flush();
    }

    /// Flushes once a second.
    pub fn flush_if_due(&self) {
        let mut log = lock(&self.log);
        if log.flushed.elapsed() >= Duration::from_secs(1) {
            log.flush();
        }
    }

    pub fn flush(&self) {
        lock(&self.log).flush();
    }
}

impl<C: Connector> Connector for Recording<C> {
    type Transport = RecordingTransport<C::Transport>;

    fn connect(&mut self) -> Result<Option<Self::Transport>, OpenError> {
        Ok(self.inner.connect()?.map(|inner| RecordingTransport {
            inner,
            log: Arc::clone(&self.log),
        }))
    }
}

/// A connection whose successful writes are recorded.
pub struct RecordingTransport<T> {
    inner: T,
    log: Arc<Mutex<Log>>,
}

impl<T: Transport> Transport for RecordingTransport<T> {
    fn write_report(&mut self, report: &Report) -> io::Result<()> {
        self.inner.write_report(report)?;
        lock(&self.log).record(report);
        Ok(())
    }
}
