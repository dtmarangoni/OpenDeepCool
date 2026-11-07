//! Test support, only ever a dev-dependency of the program's crates, so none of it ships: the
//! expected-frame loader, the spectrum's test signals and expected rows, the picture diff, the
//! capture-log reader, the research sweeps for the local layer 2 tests, the sysfs lookups, fake
//! trees and their sensor fixtures, a private D-Bus daemon, a headless weston, and the hardware
//! probe that the tests and dev tools of every phase share (PRD, Testing Strategy).

pub mod bus;
pub mod capture;
pub mod diff;
pub mod expected;
pub mod fake_tree;
pub mod fixtures;
pub mod hardware;
pub mod spectrum;
pub mod sweep;
pub mod sysfs;
pub mod weston;
