//! The sound for the Music modes (Phase 9 plan, Task 3; `docs/spec/music.md`, Spectrum: analysis):
//! the latest 2048 mono samples, the engine's window. In the service they come from pipes read on
//! threads of their own ([`PipeAudio`]): the one whose write end the session's helper got from
//! `AttachAudio()`, and on the dev copy a test's override. The engine's tests set each tick's
//! window themselves ([`FakeAudio`]). No sample is ever logged or kept beyond the window.

use std::fmt;
use std::fs::File;
use std::io::{self, ErrorKind, Read};
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use odc_render::WINDOW;
use rustix::event::{PollFd, PollFlags, Timespec, poll};

/// Sound that stops coming counts as silence once its newest sample is older than this, two
/// ticks (owner, 2026-11-05).
pub const STALE: Duration = Duration::from_millis(200);

/// How often a reader whose pipe brings nothing looks whether it must stop: a closed pipe is
/// closed within this.
const STOP_CHECK: Duration = Duration::from_millis(20);

/// One stereo frame: two `f32`, left then right, in the machine's byte order.
const FRAME: usize = 8;

/// The most a reader takes from its pipe at once: a whole pipe's default capacity, 65 536 bytes
/// (solution review P9-09).
const CHUNK: usize = 64 * 1024;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Where the engine takes its window from.
pub trait AudioSource: Send + Sync {
    /// The latest 2048 mono samples, oldest first: silence before the first sample, after the
    /// sound's pipe closes, and once the newest sample is older than [`STALE`].
    fn window(&self) -> [f32; WINDOW];
}

/// No sound at all: the engine's source unless it's given one.
pub struct Silence;

impl AudioSource for Silence {
    fn window(&self) -> [f32; WINDOW] {
        [0.0; WINDOW]
    }
}

/// A sample as the window takes it: NaN and ±∞ count as 0, and anything beyond ±1 is clipped to
/// ±1, so a broken stream draws silence or full bars, never a stuck bar (`music.md`, Samples out
/// of range; solution review P4-08, P9-03).
pub fn sanitised(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// One stereo frame as one mono sample: each channel sanitised, then the two averaged.
pub fn mono(frame: [u8; FRAME]) -> f32 {
    let channel = |bytes: &[u8]| {
        sanitised(f32::from_ne_bytes(
            bytes.try_into().expect("four bytes a channel"),
        ))
    };
    f32::midpoint(channel(&frame[..4]), channel(&frame[4..]))
}

/// The latest samples of one pipe, and when the newest was read.
struct Ring {
    samples: [f32; WINDOW],
    /// Where the next sample goes; the oldest is there.
    next: usize,
    newest: Option<Instant>,
}

impl Ring {
    fn new() -> Ring {
        Ring {
            samples: [0.0; WINDOW],
            next: 0,
            newest: None,
        }
    }

    fn push(&mut self, sample: f32) {
        self.samples[self.next] = sample;
        self.next = (self.next + 1) % WINDOW;
    }

    /// The window at `now`: the samples oldest first, or silence once the newest is stale.
    fn window(&self, now: Instant) -> [f32; WINDOW] {
        match self.newest {
            Some(read) if now.saturating_duration_since(read) <= STALE => {
                std::array::from_fn(|i| self.samples[(self.next + i) % WINDOW])
            }
            _ => [0.0; WINDOW],
        }
    }
}

/// Why a pipe ended by itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ended {
    /// The other end closed: an end of file.
    Closed,
    /// A read or poll failed.
    Failed(String),
}

impl fmt::Display for Ended {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ended::Closed => f.write_str("its writer closed it"),
            Ended::Failed(why) => write!(f, "a read failed: {why}"),
        }
    }
}

/// One pipe being read: its samples, and the flag that stops its thread, which then closes it.
struct Stream {
    ring: Arc<Mutex<Ring>>,
    stop: Arc<AtomicBool>,
}

impl Stream {
    /// Reads `pipe` on a thread of its own until it ends or the stream is dropped; `ended` is
    /// told when it ends by itself.
    fn start(pipe: OwnedFd, ended: impl FnOnce(Ended) + Send + 'static) -> io::Result<Stream> {
        let ring = Arc::new(Mutex::new(Ring::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (reader_ring, reader_stop) = (Arc::clone(&ring), Arc::clone(&stop));
        thread::Builder::new().name("audio".into()).spawn(move || {
            let why = read(File::from(pipe), &reader_ring, &reader_stop);
            if let Some(why) = why {
                *lock(&reader_ring) = Ring::new();
                ended(why);
            }
        })?;
        Ok(Stream { ring, stop })
    }

    fn window(&self, now: Instant) -> [f32; WINDOW] {
        lock(&self.ring).window(now)
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// The reader's loop: whole stereo frames into the ring, a frame split between two reads joined;
/// returns why the pipe ended by itself, or `None` when told to stop.
fn read(mut pipe: File, ring: &Mutex<Ring>, stop: &AtomicBool) -> Option<Ended> {
    let wait = Timespec::try_from(STOP_CHECK).expect("a short wait");
    let mut buffer = vec![0u8; CHUNK];
    let mut partial: Vec<u8> = Vec::with_capacity(FRAME);
    loop {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        let mut fds = [PollFd::new(&pipe, PollFlags::IN)];
        match poll(&mut fds, Some(&wait)) {
            Ok(0) => continue,
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(e) => return Some(Ended::Failed(e.to_string())),
        }
        let n = match pipe.read(&mut buffer) {
            Ok(0) => return Some(Ended::Closed),
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Some(Ended::Failed(e.to_string())),
        };
        let mut bytes = &buffer[..n];
        let mut ring = lock(ring);
        if !partial.is_empty() {
            let wanted = (FRAME - partial.len()).min(bytes.len());
            partial.extend_from_slice(&bytes[..wanted]);
            bytes = &bytes[wanted..];
            if partial.len() == FRAME {
                ring.push(mono(partial[..].try_into().expect("a frame")));
                partial.clear();
            }
        }
        let (frames, rest) = bytes.as_chunks::<FRAME>();
        for frame in frames {
            ring.push(mono(*frame));
        }
        partial.extend_from_slice(rest);
        ring.newest = Some(Instant::now());
    }
}

/// The pipe the session's helper attached: its number, its caller's unique bus name, and the
/// stream reading it.
struct Attached {
    id: u64,
    holder: String,
    stream: Stream,
}

/// The service's sound: the attached helper's pipe, and on the dev copy a test's override, which
/// the window comes from instead while it's set (`docs/spec/dbus.md`, Dev interface).
#[derive(Default)]
pub struct PipeAudio {
    attached: Arc<Mutex<Option<Attached>>>,
    overridden: Mutex<Option<Stream>>,
    attaches: AtomicU64,
}

impl PipeAudio {
    pub fn new() -> PipeAudio {
        PipeAudio::default()
    }

    /// Reads `pipe` from now on, for the caller `holder`, in place of an earlier pipe, which is
    /// closed. When this pipe ends by itself, at its end of file or a read error, it's let go and
    /// `ended` is told who held it and why. Returns the attachment's number.
    ///
    /// # Errors
    ///
    /// When the reader thread can't start; nothing changes then.
    pub fn attach(
        &self,
        pipe: OwnedFd,
        holder: &str,
        ended: impl FnOnce(&str, Ended) + Send + 'static,
    ) -> io::Result<u64> {
        let id = self.attaches.fetch_add(1, Ordering::Relaxed) + 1;
        let attached = Arc::clone(&self.attached);
        let name = holder.to_owned();
        let stream = Stream::start(pipe, move |why| {
            let mut slot = lock(&attached);
            if slot.as_ref().is_some_and(|a| a.id == id) {
                *slot = None;
                drop(slot);
                ended(&name, why);
            }
        })?;
        *lock(&self.attached) = Some(Attached {
            id,
            holder: holder.to_owned(),
            stream,
        });
        Ok(id)
    }

    /// The attachment now: its number and its caller's unique bus name.
    pub fn holder(&self) -> Option<(u64, String)> {
        lock(&self.attached)
            .as_ref()
            .map(|a| (a.id, a.holder.clone()))
    }

    /// Closes the attached pipe; returns who held it.
    pub fn detach(&self) -> Option<String> {
        lock(&self.attached).take().map(|a| a.holder)
    }

    /// Closes the attached pipe if it's still attachment `id`; returns whether it was.
    pub fn detach_if(&self, id: u64) -> bool {
        let mut slot = lock(&self.attached);
        if slot.as_ref().is_some_and(|a| a.id == id) {
            *slot = None;
            true
        } else {
            false
        }
    }

    /// The dev copy's: samples from `pipe` replace the attached pipe's in the window until
    /// [`PipeAudio::clear_override`]; its end of file is silence.
    ///
    /// # Errors
    ///
    /// When the reader thread can't start; nothing changes then.
    pub fn override_with(&self, pipe: OwnedFd) -> io::Result<()> {
        let stream = Stream::start(pipe, |_| {})?;
        *lock(&self.overridden) = Some(stream);
        Ok(())
    }

    /// Back to the attached pipe's samples; the override's pipe is closed.
    pub fn clear_override(&self) {
        lock(&self.overridden).take();
    }

    /// Whether an override is set.
    pub fn overridden(&self) -> bool {
        lock(&self.overridden).is_some()
    }

    /// The window at `now`, from the override while one is set, else from the attached pipe.
    pub fn window_at(&self, now: Instant) -> [f32; WINDOW] {
        if let Some(stream) = lock(&self.overridden).as_ref() {
            return stream.window(now);
        }
        lock(&self.attached)
            .as_ref()
            .map_or([0.0; WINDOW], |a| a.stream.window(now))
    }
}

impl AudioSource for PipeAudio {
    fn window(&self) -> [f32; WINDOW] {
        self.window_at(Instant::now())
    }
}

/// The tests' sound: the window set for the next tick, silence until one is.
pub struct FakeAudio {
    window: Mutex<[f32; WINDOW]>,
}

impl Default for FakeAudio {
    fn default() -> FakeAudio {
        FakeAudio {
            window: Mutex::new([0.0; WINDOW]),
        }
    }
}

impl FakeAudio {
    pub fn new() -> FakeAudio {
        FakeAudio::default()
    }

    pub fn set(&self, window: [f32; WINDOW]) {
        *lock(&self.window) = window;
    }

    pub fn silence(&self) {
        self.set([0.0; WINDOW]);
    }
}

impl AudioSource for FakeAudio {
    fn window(&self) -> [f32; WINDOW] {
        *lock(&self.window)
    }
}
