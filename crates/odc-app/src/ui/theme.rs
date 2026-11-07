//! The theme in the window (Phase 10 plan, Task 7; `docs/design/settings-app.md`, Theme): the
//! current Omarchy theme, read from `<state>/current/theme`, where `<state>` is
//! `~/.local/state/omarchy` or `--omarchy-state`, and followed live; OpenDeepCool's default theme
//! where there's none.
//!
//! Omarchy switches theme by staging it in `current/next-theme`, removing `current/theme` and
//! moving the staged folder in its place (its `omarchy-theme-set`, read 2026-11-06), so a
//! `GFileMonitor` on `current/` sees every switch, and one on `current/theme/` an edit in place.
//! Each folder of that path is watched once it exists, and the nearest existing one above it
//! meanwhile, so a theme that appears later is found too (decisions log D65). After 150 ms of
//! quiet the files are read again, and the window takes the theme only when its tokens changed:
//! the same provider reloaded, the preview and the tiles redrawn, the page and scroll position
//! kept.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::window::{WeakWindow, Window};
use crate::theme::{self, Theme, Tokens};

/// The quiet a change waits for before the theme is read again.
pub const QUIET: Duration = Duration::from_millis(150);

/// The current theme's folder in an Omarchy state folder.
pub fn theme_dir(state: &Path) -> PathBuf {
    state.join("current").join("theme")
}

/// Reads the theme in `state`, or the default without one; the source and each problem go to
/// `log`, one line each.
pub fn load(state: Option<&Path>, log: &dyn Fn(&str)) -> Theme {
    let dir = state.map(theme_dir);
    let read = |name: &str| {
        dir.as_ref()
            .and_then(|d| fs::read_to_string(d.join(name)).ok())
    };
    let colors = read("colors.toml");
    let theme = theme::theme(colors.as_deref(), read("shell.toml").as_deref());
    for problem in &theme.problems {
        log(&format!("theme: {problem}"));
    }
    theme
}

fn source(theme_found: bool) -> &'static str {
    if theme_found {
        "the Omarchy theme"
    } else {
        "the default theme"
    }
}

/// Whether `state` holds a theme to read.
fn has_theme(state: Option<&Path>) -> bool {
    state.is_some_and(|s| theme_dir(s).join("colors.toml").is_file())
}

/// The folders watched for `state`: each of `state`, `current/` and `current/theme/` that exists,
/// and the nearest existing folder above `state` when it doesn't.
fn watched(state: &Path) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = [state.to_path_buf(), state.join("current"), theme_dir(state)]
        .into_iter()
        .filter(|p| p.is_dir())
        .collect();
    if folders.is_empty() {
        let mut above = state.parent();
        while let Some(folder) = above {
            if folder.is_dir() {
                folders.push(folder.to_path_buf());
                break;
            }
            above = folder.parent();
        }
    }
    folders
}

struct Inner {
    window: WeakWindow,
    state: PathBuf,
    log: Box<dyn Fn(&str)>,
    tokens: RefCell<Tokens>,
    monitors: RefCell<Vec<gio::FileMonitor>>,
    timer: RefCell<Option<glib::SourceId>>,
}

/// Follows the Omarchy theme for a window; it follows as long as it's kept.
#[derive(Clone)]
pub struct LiveTheme(Rc<Inner>);

impl LiveTheme {
    /// The theme in `state` for a window about to be built, with the source logged.
    pub fn first(state: Option<&Path>, log: &dyn Fn(&str)) -> Theme {
        let theme = load(state, log);
        log(&format!("theme: {}", source(has_theme(state))));
        theme
    }

    /// Follows `state` for `window`, which shows `tokens` now. Without a state folder, as away
    /// from Omarchy with no home folder, there's nothing to follow.
    pub fn follow(
        window: &Window,
        state: Option<PathBuf>,
        tokens: Tokens,
        log: impl Fn(&str) + 'static,
    ) -> Option<LiveTheme> {
        let live = LiveTheme(Rc::new(Inner {
            window: window.downgrade(),
            state: state?,
            log: Box::new(log),
            tokens: RefCell::new(tokens),
            monitors: RefCell::new(Vec::new()),
            timer: RefCell::new(None),
        }));
        live.watch();
        Some(live)
    }

    /// Watches the folders of the path, again, as they are now.
    fn watch(&self) {
        let mut monitors = Vec::new();
        for folder in watched(&self.0.state) {
            match gio::File::for_path(&folder).monitor_directory(
                gio::FileMonitorFlags::WATCH_MOVES,
                None::<&gio::Cancellable>,
            ) {
                Ok(monitor) => {
                    let weak = Rc::downgrade(&self.0);
                    monitor.connect_changed(move |_, _, _, _| {
                        if let Some(inner) = weak.upgrade() {
                            LiveTheme(inner).changed();
                        }
                    });
                    monitors.push(monitor);
                }
                Err(error) => {
                    (self.0.log)(&format!(
                        "theme: {} can't be watched: {error}",
                        folder.display()
                    ));
                }
            }
        }
        // The old monitors stop when dropped.
        *self.0.monitors.borrow_mut() = monitors;
    }

    /// A change: the theme is read again after 150 ms of quiet.
    fn changed(&self) {
        if let Some(timer) = self.0.timer.borrow_mut().take() {
            timer.remove();
        }
        let weak = Rc::downgrade(&self.0);
        let timer = glib::timeout_add_local_once(QUIET, move || {
            if let Some(inner) = weak.upgrade() {
                inner.timer.borrow_mut().take();
                LiveTheme(inner).reload();
            }
        });
        *self.0.timer.borrow_mut() = Some(timer);
    }

    fn reload(&self) {
        self.watch();
        let Some(window) = self.0.window.upgrade() else {
            return;
        };
        let state = Some(self.0.state.as_path());
        let theme = load(state, &*self.0.log);
        if theme.tokens == *self.0.tokens.borrow() {
            return;
        }
        window.set_theme(&theme.tokens);
        *self.0.tokens.borrow_mut() = theme.tokens;
        (self.0.log)(&format!("theme: reloaded, {}", source(has_theme(state))));
    }

    /// The tokens the window shows, for the tests.
    pub fn tokens(&self) -> Tokens {
        *self.0.tokens.borrow()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_watched_folders_follow_what_exists() {
        let root = std::env::temp_dir().join(format!("odc-app-watched-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("a folder");
        let state = root.join("omarchy");
        // Nothing of the state yet: the nearest folder above it.
        assert_eq!(watched(&state), std::slice::from_ref(&root));
        fs::create_dir_all(state.join("current")).expect("current");
        assert_eq!(watched(&state), [state.clone(), state.join("current")]);
        fs::create_dir_all(theme_dir(&state)).expect("theme");
        assert_eq!(
            watched(&state),
            [state.clone(), state.join("current"), theme_dir(&state)]
        );
        fs::remove_dir_all(&root).expect("cleaned");
    }
}
