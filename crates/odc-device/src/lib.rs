//! The display port and the LP360 adapter: it finds the cooler by its USB ID, writes reports to
//! its hidraw node, and keeps finding it after it disappears (`docs/spec/protocol.md`, Device to
//! Standby).

pub mod fake;
mod lp360;
mod port;
mod record;

pub use lp360::{Connector, Lp360, OpenError, Transport};
pub use port::{DisplayPort, Link, Reason};
pub use record::{Recording, RecordingTransport};
