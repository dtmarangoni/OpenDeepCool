//! Test support, only ever a dev-dependency of the program's crates, so none of it ships: the
//! expected-frame loader, the picture diff, the capture-log reader and the sysfs lookups that the
//! tests and dev tools of every phase share (PRD, Testing Strategy).

pub mod capture;
pub mod diff;
pub mod expected;
pub mod sysfs;
