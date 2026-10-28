//! `Lp360` keeps the cooler fed: `03 08` first on every connection, and a rescan after any failure
//! (`docs/spec/protocol.md`: Disconnect and reconnect, Standby).

use std::io;
use std::path::PathBuf;

use odc_protocol::{Frame, Report, encode_frame, startup_report};

use crate::port::{DisplayPort, Link, Reason};

/// An open connection to a cooler. It takes only a [`Report`], which only the encoder makes, so
/// only documented messages reach the device (AGENTS.md rule 9).
pub trait Transport {
    fn write_report(&mut self, report: &Report) -> io::Result<()>;
}

/// Finds the cooler and opens it.
pub trait Connector {
    type Transport: Transport;

    /// `Ok(None)` when no cooler is attached; `Err` when one is attached but can't be opened.
    fn connect(&mut self) -> Result<Option<Self::Transport>, OpenError>;
}

/// A cooler's device node that couldn't be opened.
#[derive(Debug)]
pub struct OpenError {
    pub path: PathBuf,
    pub error: io::Error,
}

/// The cooler behind a [`Connector`]. It owns the open connection and starts every new one with
/// the start-up command. It has no timer and no thread: the caller's tick paces it, and while the
/// cooler is away every `show` looks for it again, with no limit (owner, 2026-10-24).
///
/// A write can block for up to 5 s on a stalled cooler (the kernel's `USB_CTRL_SET_TIMEOUT`), so
/// Phase 7 calls `show` from a device thread of its own, never from its tick.
pub struct Lp360<C: Connector> {
    connector: C,
    transport: Option<C::Transport>,
    last: Option<Link>,
    changed: bool,
}

impl<C: Connector> Lp360<C> {
    pub fn new(connector: C) -> Self {
        Lp360 {
            connector,
            transport: None,
            last: None,
            changed: false,
        }
    }

    /// Whether the last `show` changed the link, its reason included, so each change is logged
    /// once, never every tick. The first `show` counts as a change.
    pub fn state_changed(&self) -> bool {
        self.changed
    }

    /// Starts a new conversation: drops the open connection, if any, so the next `show`
    /// connects again and sends `03 08` first, even when the device node survived. Phase 7 calls
    /// it after every wake-up from standby (`docs/spec/protocol.md`, Standby; owner, 2026-10-26).
    pub fn reset_connection(&mut self) {
        self.transport = None;
    }

    /// The connector, for example to check a recording.
    pub fn connector(&self) -> &C {
        &self.connector
    }

    fn try_show(&mut self, frame: &Frame) -> Link {
        if self.transport.is_none() {
            match self.connector.connect() {
                Ok(None) => return Link::Disconnected(Reason::NotFound),
                Err(e) => {
                    return Link::Disconnected(Reason::NoAccess {
                        path: e.path,
                        errno: e.error.raw_os_error(),
                    });
                }
                Ok(Some(mut transport)) => {
                    if let Err(e) = transport.write_report(&startup_report()) {
                        return write_failed(&e);
                    }
                    self.transport = Some(transport);
                }
            }
        }
        let transport = self.transport.as_mut().expect("a connection");
        match transport.write_report(&encode_frame(frame)) {
            Ok(()) => Link::Connected,
            Err(e) => {
                self.transport = None;
                write_failed(&e)
            }
        }
    }
}

fn write_failed(e: &io::Error) -> Link {
    Link::Disconnected(Reason::WriteFailed {
        errno: e.raw_os_error(),
    })
}

impl<C: Connector> DisplayPort for Lp360<C> {
    fn show(&mut self, frame: &Frame) -> Link {
        let link = self.try_show(frame);
        self.changed = self.last.as_ref() != Some(&link);
        self.last = Some(link.clone());
        link
    }
}
