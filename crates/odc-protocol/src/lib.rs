//! The LP360's messages, as pure code: a 14 × 14 picture, its rotation, and the 64-byte reports
//! the cooler accepts (`docs/spec/protocol.md`). Only this crate can build a report, so only the
//! messages the spec documents ever reach the device (AGENTS.md rule 9).

mod frame;
mod report;

pub use frame::{Frame, Rotation};
pub use report::{DecodeError, Message, Report, decode, encode_frame, startup_report};
