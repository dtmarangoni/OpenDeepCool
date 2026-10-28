//! The recording: every report actually written, in the capture format, so the tests compare what
//! reached the cooler with the expected frames (AGENTS.md rule 3).

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use odc_protocol::Report;

use crate::lp360::{Connector, OpenError, Transport};

/// Wraps any [`Connector`]. Each successful write through its connections appends one line,
/// `<unix ns> 0 0 out 64 <hex>`, the capture format `odc_testkit::capture` reads.
pub struct Recording<C> {
    inner: C,
    log: Arc<Mutex<Log>>,
}

struct Log {
    path: PathBuf,
    out: File,
    /// The first failed write to the recording; nothing more is recorded after it.
    failure: Option<String>,
}

impl<C> Recording<C> {
    /// Records into `path`, created or emptied.
    pub fn create(inner: C, path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let out = File::create(&path)?;
        Ok(Recording {
            inner,
            log: Arc::new(Mutex::new(Log {
                path,
                out,
                failure: None,
            })),
        })
    }

    /// The first error writing the recording, if any. A test fails on it: an incomplete recording
    /// proves nothing.
    pub fn failure(&self) -> Option<String> {
        lock(&self.log).failure.clone()
    }
}

fn lock(log: &Mutex<Log>) -> std::sync::MutexGuard<'_, Log> {
    log.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
        let mut log = lock(&self.log);
        if log.failure.is_none() {
            let unix_ns = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let hex: String = report
                .as_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            if let Err(e) = writeln!(log.out, "{unix_ns} 0 0 out 64 {hex}") {
                log.failure = Some(format!("{}: {e}", log.path.display()));
            }
        }
        Ok(())
    }
}
