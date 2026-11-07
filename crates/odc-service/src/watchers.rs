//! The watchers (Phase 7 plan, Task 7; `docs/spec/dbus.md`, Who may call): the callers that get
//! `StateChanged`, and from Phase 10 `Frame`, at most 8 per Unix user and 32 in all, a caller
//! that watches both counting once. Each is checked again every second; one that fails is
//! skipped until it passes again, and then gets the whole state.

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

/// What a caller watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watch {
    /// `StateChanged`, from `Watch()`.
    State,
    /// `Frame`, from `WatchFrames()` (Phase 10).
    Frames,
}

struct Watcher {
    /// Its unique bus name.
    name: String,
    user: u32,
    state: bool,
    frames: bool,
    /// Whether it passed its last check.
    allowed: bool,
}

#[derive(Default)]
pub struct Watchers {
    list: Vec<Watcher>,
}

impl Watchers {
    /// Adds `what` to the watches of a caller that just passed its check; a second call for the
    /// same name and watch changes nothing, and a caller that already watches the other counts
    /// no further.
    ///
    /// # Errors
    ///
    /// The `TooManyWatchers` text, when a new caller's user or everyone together already has the
    /// most.
    pub fn add(&mut self, name: &str, user: u32, what: Watch) -> Result<(), String> {
        if let Some(watcher) = self.list.iter_mut().find(|w| w.name == name) {
            watcher.set(what, true);
            return Ok(());
        }
        if self.list.iter().filter(|w| w.user == user).count() >= PER_USER {
            return Err(format!("user {user} already has {PER_USER} watchers"));
        }
        if self.list.len() >= IN_ALL {
            return Err(format!("there are already {IN_ALL} watchers"));
        }
        let mut watcher = Watcher {
            name: name.to_owned(),
            user,
            state: false,
            frames: false,
            allowed: true,
        };
        watcher.set(what, true);
        self.list.push(watcher);
        Ok(())
    }

    /// Ends one watch: `Unwatch()` or `UnwatchFrames()`. A caller left watching nothing is
    /// dropped.
    pub fn end(&mut self, name: &str, what: Watch) {
        if let Some(watcher) = self.list.iter_mut().find(|w| w.name == name) {
            watcher.set(what, false);
        }
        self.list.retain(|w| w.state || w.frames);
    }

    /// Drops a caller and every watch it had: its name left the bus. Whether it was there.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.list.len();
        self.list.retain(|w| w.name != name);
        self.list.len() < before
    }

    /// Every watcher's name, for the check each second.
    pub fn names(&self) -> Vec<String> {
        self.list.iter().map(|w| w.name.clone()).collect()
    }

    /// The callers watching `what` that passed their last check.
    pub fn allowed(&self, what: Watch) -> Vec<String> {
        self.list
            .iter()
            .filter(|w| w.allowed && w.watches(what))
            .map(|w| w.name.clone())
            .collect()
    }

    /// Whether `name` watches `what`.
    pub fn watches(&self, name: &str, what: Watch) -> bool {
        self.list.iter().any(|w| w.name == name && w.watches(what))
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

impl Watcher {
    fn set(&mut self, what: Watch, on: bool) {
        match what {
            Watch::State => self.state = on,
            Watch::Frames => self.frames = on,
        }
    }

    fn watches(&self, what: Watch) -> bool {
        match what {
            Watch::State => self.state,
            Watch::Frames => self.frames,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A caller watching the state and the frames counts once against the caps; ending one watch
    // keeps the other, and ending both drops it.
    #[test]
    fn a_caller_watching_both_counts_once_and_keeps_the_other_watch() {
        let mut w = Watchers::default();
        w.add(":1.1", 1000, Watch::State).expect("added");
        w.add(":1.1", 1000, Watch::Frames).expect("added to it");
        w.add(":1.1", 1000, Watch::Frames).expect("no change");
        for n in 2..=PER_USER {
            w.add(&format!(":1.{n}"), 1000, Watch::Frames)
                .expect("under the cap");
        }
        assert!(w.add(":1.99", 1000, Watch::State).is_err(), "the cap");
        assert_eq!(w.allowed(Watch::State), [":1.1"]);
        assert_eq!(w.allowed(Watch::Frames).len(), PER_USER);
        w.end(":1.1", Watch::State);
        assert!(w.allowed(Watch::State).is_empty());
        assert!(w.watches(":1.1", Watch::Frames));
        w.end(":1.1", Watch::Frames);
        assert!(!w.names().contains(&":1.1".to_owned()), "dropped");
        assert!(w.add(":1.99", 1000, Watch::State).is_ok(), "a place again");
    }

    // A caller that fails its check gets neither; leaving the bus drops both watches.
    #[test]
    fn a_failed_check_skips_both_watches_and_leaving_drops_them() {
        let mut w = Watchers::default();
        w.add(":1.5", 1000, Watch::State).expect("added");
        w.add(":1.5", 1000, Watch::Frames).expect("added");
        assert_eq!(w.checked(":1.5", false), Checked::Failed);
        assert!(w.allowed(Watch::State).is_empty());
        assert!(w.allowed(Watch::Frames).is_empty());
        assert_eq!(w.checked(":1.5", true), Checked::Again);
        assert_eq!(w.allowed(Watch::Frames), [":1.5"]);
        assert!(w.remove(":1.5"));
        assert!(w.names().is_empty());
    }
}
