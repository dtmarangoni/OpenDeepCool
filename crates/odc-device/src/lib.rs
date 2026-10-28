//! The display port and the LP360 adapter: it finds the cooler by its USB ID, writes reports to
//! its hidraw node, and keeps finding it after it disappears (`docs/spec/protocol.md`, Device to
//! Standby).
