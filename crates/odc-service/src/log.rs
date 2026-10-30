//! The journal (Phase 7 plan, Task 7): stderr lines with `sd-daemon(3)`'s `<N>` priority
//! prefixes, which journald reads from a service's stderr, so `journalctl -p` filters them. Every
//! line carries its own prefix; a line without one is logged as info. There's no logging crate.

use std::io::Write;
use std::sync::{Mutex, MutexGuard};

/// A line's priority, as `sd-daemon(3)` numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Critical = 2,
    Error = 3,
    Warning = 4,
    Notice = 5,
    Info = 6,
}

/// Where the service's journal lines go.
pub trait Journal: Send + Sync {
    /// Writes `text`, each of its lines with the priority's prefix.
    fn write(&self, priority: Priority, text: &str);
}

/// The service's stderr, which systemd hands to journald.
pub struct Stderr;

impl Journal for Stderr {
    fn write(&self, priority: Priority, text: &str) {
        let mut out = String::new();
        for line in text.lines() {
            out.push_str(&format!("<{}>{line}\n", priority as u8));
        }
        // Nothing else could report a journal that can't be written.
        let _ = std::io::stderr().lock().write_all(out.as_bytes());
    }
}

/// The lines written, kept for the tests.
#[derive(Default)]
pub struct Recorded {
    lines: Mutex<Vec<(Priority, String)>>,
}

impl Recorded {
    fn lock(&self) -> MutexGuard<'_, Vec<(Priority, String)>> {
        self.lines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn lines(&self) -> Vec<(Priority, String)> {
        self.lock().clone()
    }
}

impl Journal for Recorded {
    fn write(&self, priority: Priority, text: &str) {
        let mut lines = self.lock();
        for line in text.lines() {
            lines.push((priority, line.to_owned()));
        }
    }
}
