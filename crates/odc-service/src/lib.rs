//! The display service (Phase 7 plan, Tasks 5–8): its settings, the engine and the threads around
//! it, the D-Bus interface, and the `opendeepcoold` binary that runs them under systemd.

pub mod engine;
pub mod runtime;
pub mod settings;
pub mod status;
