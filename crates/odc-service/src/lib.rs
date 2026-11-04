//! The display service (Phase 7 plan, Tasks 5–8): its settings, the engine and the threads around
//! it, the D-Bus interface, and the `opendeepcoold` binary that runs them under systemd.

pub mod dbus;
pub mod engine;
pub mod log;
pub mod polkit;
pub mod pulse;
pub mod runtime;
pub mod service;
pub mod settings;
pub mod sleep;
pub mod status;
pub mod watchers;

/// The service's state directory, systemd's `StateDirectory=opendeepcool`, which the GPU rank
/// helper writes its ranking into too.
pub const STATE_DIR: &str = "/var/lib/opendeepcool";

/// The dev copy's frame recording, in the state directory (Phase 7 plan, Recommendation).
pub const RECORDING: &str = "dev-frames.log";
