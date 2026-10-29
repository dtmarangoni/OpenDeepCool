//! Fakes for tests, this crate's and Phase 7's: a clock that moves only when the test says, and
//! scripted readings.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::model::{Clock, Readings, Snapshot};

/// A clock that moves only through [`FakeClock::advance`]. Its clones share one time, so a test
/// can keep one and give another to the code under test.
#[derive(Clone, Debug)]
pub struct FakeClock {
    now: Arc<Mutex<Instant>>,
}

impl FakeClock {
    pub fn new() -> Self {
        FakeClock {
            now: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn advance(&self, by: Duration) {
        *self.now.lock().expect("the fake clock's lock") += by;
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        FakeClock::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        *self.now.lock().expect("the fake clock's lock")
    }
}

/// Answers each snapshot with the next of its script; the last one repeats. It counts the calls
/// to `restart_intervals`.
pub struct FakeReadings {
    script: VecDeque<Snapshot>,
    last: Snapshot,
    restarts: usize,
}

impl FakeReadings {
    /// # Panics
    ///
    /// When the script is empty.
    pub fn new(script: impl IntoIterator<Item = Snapshot>) -> Self {
        let script: VecDeque<Snapshot> = script.into_iter().collect();
        let last = script
            .back()
            .cloned()
            .expect("a script of at least one snapshot");
        FakeReadings {
            script,
            last,
            restarts: 0,
        }
    }

    /// How many times `restart_intervals` was called.
    pub fn restarts(&self) -> usize {
        self.restarts
    }
}

impl Readings for FakeReadings {
    fn snapshot(&mut self) -> Snapshot {
        self.script.pop_front().unwrap_or_else(|| self.last.clone())
    }

    fn restart_intervals(&mut self) {
        self.restarts += 1;
    }
}
