//! The pulse: the service's plain threads send it, and the D-Bus side's tasks wait for it. zbus 6
//! has no public timer or channel, and the service has no async runtime of its own (Phase 7
//! plan, Recommendation), so this mutex and its wakers are the one bridge from the threads to the
//! D-Bus side: the status changed, a second passed, or the engine took an override.

use std::future::{Future, poll_fn};
use std::sync::{Mutex, MutexGuard};
use std::task::{Poll, Waker};

#[derive(Default)]
pub struct Pulse {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    count: u64,
    wakers: Vec<Waker>,
}

impl Pulse {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Sends a pulse: every task waiting wakes up.
    pub fn send(&self) {
        let wakers = {
            let mut inner = self.lock();
            inner.count += 1;
            std::mem::take(&mut inner.wakers)
        };
        for waker in wakers {
            waker.wake();
        }
    }

    /// How many pulses were sent so far.
    pub fn count(&self) -> u64 {
        self.lock().count
    }

    /// Waits until more than `seen` pulses were sent, and returns how many.
    pub fn after(&self, seen: u64) -> impl Future<Output = u64> + Send + '_ {
        poll_fn(move |cx| {
            let mut inner = self.lock();
            if inner.count > seen {
                return Poll::Ready(inner.count);
            }
            if !inner.wakers.iter().any(|w| w.will_wake(cx.waker())) {
                inner.wakers.push(cx.waker().clone());
            }
            Poll::Pending
        })
    }
}
