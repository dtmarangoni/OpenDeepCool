//! The watchers (Phase 7 plan, Task 7; `docs/spec/dbus.md`, Who may call): the callers that get
//! `StateChanged`, at most 8 per Unix user and 32 in all. Each is checked again every second; one
//! that fails is skipped until it passes again, and then gets the whole state.

/// At most this many watchers per Unix user.
pub const PER_USER: usize = 8;

/// At most this many watchers in all.
pub const IN_ALL: usize = 32;

/// What a watcher's check changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checked {
    /// It passes again after failing: it gets the whole state.
    Again,
    /// It fails after passing: it's skipped until it passes again.
    Failed,
    /// No change, or no such watcher.
    Same,
}

struct Watcher {
    /// Its unique bus name.
    name: String,
    user: u32,
    /// Whether it passed its last check.
    allowed: bool,
}

#[derive(Default)]
pub struct Watchers {
    list: Vec<Watcher>,
}

impl Watchers {
    /// Adds a watcher that just passed its check; a second `Watch()` from the same name changes
    /// nothing.
    ///
    /// # Errors
    ///
    /// The `TooManyWatchers` text, when its user or everyone together already has the most.
    pub fn add(&mut self, name: &str, user: u32) -> Result<(), String> {
        if self.list.iter().any(|w| w.name == name) {
            return Ok(());
        }
        if self.list.iter().filter(|w| w.user == user).count() >= PER_USER {
            return Err(format!("user {user} already has {PER_USER} watchers"));
        }
        if self.list.len() >= IN_ALL {
            return Err(format!("there are already {IN_ALL} watchers"));
        }
        self.list.push(Watcher {
            name: name.to_owned(),
            user,
            allowed: true,
        });
        Ok(())
    }

    /// Drops a watcher: `Unwatch()`, or its name left the bus. Whether it was there.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.name != name);
        self.list.len() < before
    }

    /// Every watcher's name, for the check each second.
    pub fn names(&self) -> Vec<String> {
        self.list.iter().map(|w| w.name.clone()).collect()
    }

    /// The watchers that passed their last check: they get `StateChanged`.
    pub fn allowed(&self) -> Vec<String> {
        self.list
            .iter()
            .filter(|w| w.allowed)
            .map(|w| w.name.clone())
            .collect()
    }

    /// Records a watcher's check.
    pub fn checked(&mut self, name: &str, allowed: bool) -> Checked {
        let Some(watcher) = self.list.iter_mut().find(|w| w.name == name) else {
            return Checked::Same;
        };
        let change = match (watcher.allowed, allowed) {
            (false, true) => Checked::Again,
            (true, false) => Checked::Failed,
            _ => Checked::Same,
        };
        watcher.allowed = allowed;
        change
    }
}
