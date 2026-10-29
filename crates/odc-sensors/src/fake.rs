//! Fakes for tests, this crate's and Phase 7's: a clock that moves only when the test says,
//! scripted readings, and NVIDIA's library.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::model::{Clock, Readings, Snapshot, Unavailable};
use crate::nvml::{NvmlReading, NvmlSource};

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

/// NVIDIA's library for tests: each start answers `start`, and each GPU it knows answers its
/// temperature (°C), usage (%) and power (mW). It counts its starts, so a test can show NVML never
/// starts on a PC whose chosen GPU isn't NVIDIA's.
pub struct FakeNvml {
    start: Result<(), Unavailable>,
    gpus: Vec<(String, [u32; 3])>,
    starts: Arc<AtomicUsize>,
}

impl FakeNvml {
    pub fn new(start: Result<(), Unavailable>, gpus: &[(&str, u32, u32, u32)]) -> Self {
        FakeNvml {
            start,
            gpus: gpus
                .iter()
                .map(|&(bus_id, temperature, usage, milliwatts)| {
                    (bus_id.to_owned(), [temperature, usage, milliwatts])
                })
                .collect(),
            starts: Arc::default(),
        }
    }

    /// A handle on the count of starts, for the test to read.
    pub fn starts(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.starts)
    }
}

impl NvmlSource for FakeNvml {
    fn start(&mut self) -> Result<(), Unavailable> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        self.start.clone()
    }

    fn read(&mut self, bus_id: &str, reading: NvmlReading) -> Result<u32, Unavailable> {
        let (_, values) = self
            .gpus
            .iter()
            .find(|(known, _)| known == bus_id)
            .ok_or(Unavailable::NoDevice)?;
        Ok(match reading {
            NvmlReading::Temperature => values[0],
            NvmlReading::Usage => values[1],
            NvmlReading::Power => values[2],
        })
    }
}
