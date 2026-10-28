//! The display port: the one thing the display logic asks of the cooler (PRD, Architecture Notes:
//! ports and adapters).

use std::path::PathBuf;

use odc_protocol::Frame;

/// Where frames go. Phase 7's device thread calls [`DisplayPort::show`] with the newest frame,
/// once per 100 ms tick (`docs/spec/protocol.md`, Timing).
pub trait DisplayPort {
    /// Sends `frame` to the cooler, connecting first when needed, and says how the link stands.
    fn show(&mut self, frame: &Frame) -> Link;
}

/// How the link to the cooler stands after a `show`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    /// The frame was written.
    Connected,
    /// Nothing was written, and why (owner, 2026-10-27; solution review P5-02).
    Disconnected(Reason),
}

/// Why no frame reached the cooler. `errno` is the kernel's error number, when the error came
/// from the kernel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    /// No cooler is attached.
    NotFound,
    /// A cooler is attached, but its device node can't be opened, for example without the udev
    /// rule's access.
    NoAccess { path: PathBuf, errno: Option<i32> },
    /// A write failed: the cooler vanished or stalled.
    WriteFailed { errno: Option<i32> },
}
