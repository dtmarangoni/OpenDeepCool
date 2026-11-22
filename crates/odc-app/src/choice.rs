//! Which theme the window shows (`docs/design/settings-app.md`, Theme; Phase 11a Task 11): the
//! Omarchy theme, or OpenDeepCool's own, dark or light. Pure: no GTK and no files, only what the
//! app knows (an Omarchy theme found or not, the system's dark or light preference) and what the
//! service keeps for the whole PC (follow the system's theme; OpenDeepCool's own is dark).

use odc_ipc::{WindowCorners, WindowOpacity};

/// The system's dark or light preference, as GTK reports it in
/// `GtkSettings:gtk-interface-color-scheme`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preference {
    /// The system gives none: there is no portal, or nothing answers it. "No system theme found"
    /// when there is no Omarchy theme either.
    #[default]
    Unsupported,
    /// The system answers, with no preference: OpenDeepCool's own dark theme.
    Default,
    Dark,
    Light,
}

/// What the app knows of the system's theme: an Omarchy theme found and whether it is a dark
/// one (its `colors.toml` `mode`), and the system's preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub omarchy: Option<bool>,
    pub preference: Preference,
}

/// What decides the theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inputs {
    /// An Omarchy theme was found, and whether it is a dark one (its `colors.toml` `mode`).
    pub omarchy: Option<bool>,
    pub preference: Preference,
    /// The service's `UseSystemTheme`.
    pub use_system: bool,
    /// The service's `DarkTheme`: OpenDeepCool's own theme is the dark one.
    pub dark: bool,
    /// The service's `WindowOpacity` and `WindowCorners`: how OpenDeepCool's own theme draws the
    /// window (Phase 11a Task 49); with the system's theme neither applies.
    pub opacity: WindowOpacity,
    pub corners: WindowCorners,
}

/// The theme the window shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The Omarchy theme, as today.
    Omarchy,
    /// OpenDeepCool's own theme, dark or light.
    Own { dark: bool },
}

impl Inputs {
    /// The facts and the service's two settings together.
    pub fn new(facts: Facts, use_system: bool, dark: bool) -> Inputs {
        Inputs {
            omarchy: facts.omarchy,
            preference: facts.preference,
            use_system,
            dark,
            opacity: WindowOpacity::Opaque,
            corners: WindowCorners::Rounded,
        }
    }

    /// The same with the service's window opacity and corners: the defaults, opaque and
    /// rounded, until the service has answered.
    pub fn with_window(self, opacity: WindowOpacity, corners: WindowCorners) -> Inputs {
        Inputs {
            opacity,
            corners,
            ..self
        }
    }

    /// Whether the system gives a theme: an Omarchy theme, or a preference other than
    /// `Unsupported`.
    pub fn system_found(&self) -> bool {
        self.omarchy.is_some() || self.preference != Preference::Unsupported
    }

    /// Whether the window follows the system's theme now: it is asked to, and there is one.
    pub fn follows_system(&self) -> bool {
        self.use_system && self.system_found()
    }

    /// The theme shown. Following the system, an Omarchy theme wins over the preference; the
    /// preference gives OpenDeepCool's own theme, light for `Light` and dark for `Dark` and
    /// `Default`. Otherwise OpenDeepCool's own, as chosen.
    pub fn source(&self) -> Source {
        if !self.follows_system() {
            return Source::Own { dark: self.dark };
        }
        match (self.omarchy, self.preference) {
            (Some(_), _) => Source::Omarchy,
            (None, Preference::Light) => Source::Own { dark: false },
            (None, _) => Source::Own { dark: true },
        }
    }

    /// What the dark and light control shows: while the system's theme is followed, what the
    /// system gives (and the control is disabled); otherwise the service's choice.
    pub fn shown_dark(&self) -> bool {
        match self.source() {
            Source::Omarchy => self.omarchy.unwrap_or(true),
            Source::Own { dark } => dark,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFERENCES: [Preference; 4] = [
        Preference::Unsupported,
        Preference::Default,
        Preference::Dark,
        Preference::Light,
    ];

    fn inputs(
        omarchy: Option<bool>,
        preference: Preference,
        use_system: bool,
        dark: bool,
    ) -> Inputs {
        Inputs {
            omarchy,
            preference,
            use_system,
            dark,
            opacity: WindowOpacity::Opaque,
            corners: WindowCorners::Rounded,
        }
    }

    // Every combination of the four inputs (3 x 4 x 2 x 2 = 48), against the design's table.
    #[test]
    fn every_combination_gives_the_theme_the_design_says() {
        let mut seen = 0;
        for omarchy in [None, Some(true), Some(false)] {
            for preference in PREFERENCES {
                for use_system in [true, false] {
                    for dark in [true, false] {
                        let i = inputs(omarchy, preference, use_system, dark);
                        let found = omarchy.is_some() || preference != Preference::Unsupported;
                        let want = if use_system && found {
                            match (omarchy, preference) {
                                (Some(_), _) => Source::Omarchy,
                                (None, Preference::Light) => Source::Own { dark: false },
                                (None, _) => Source::Own { dark: true },
                            }
                        } else {
                            Source::Own { dark }
                        };
                        assert_eq!(i.system_found(), found, "{i:?}");
                        assert_eq!(i.source(), want, "{i:?}");
                        seen += 1;
                    }
                }
            }
        }
        assert_eq!(seen, 48);
    }

    // No system theme found (no Omarchy theme and no preference): the choice decides, whatever
    // "use the system's theme" says; the control for it is disabled and off (the card's words).
    #[test]
    fn with_no_system_theme_the_service_s_choice_decides() {
        for use_system in [true, false] {
            for dark in [true, false] {
                let i = inputs(None, Preference::Unsupported, use_system, dark);
                assert!(!i.system_found() && !i.follows_system());
                assert_eq!(i.source(), Source::Own { dark });
                assert_eq!(i.shown_dark(), dark);
            }
        }
    }

    // "No preference" shows OpenDeepCool's dark theme; a light preference its light one; an
    // Omarchy theme found wins over the preference while the system's theme is followed, and
    // is not used when it isn't.
    #[test]
    fn the_preference_and_an_omarchy_theme_follow_the_design() {
        let on = |omarchy, preference| inputs(omarchy, preference, true, false);
        assert_eq!(
            on(None, Preference::Default).source(),
            Source::Own { dark: true }
        );
        assert_eq!(
            on(None, Preference::Dark).source(),
            Source::Own { dark: true }
        );
        assert_eq!(
            on(None, Preference::Light).source(),
            Source::Own { dark: false }
        );
        assert_eq!(on(Some(false), Preference::Dark).source(), Source::Omarchy);
        assert_eq!(
            inputs(Some(true), Preference::Light, false, true).source(),
            Source::Own { dark: true }
        );
    }

    // What the dark and light control shows: while the system's theme is followed, what the
    // system gives (an Omarchy theme's own mode, or the preference); else the service's choice.
    #[test]
    fn the_control_shows_what_the_theme_is() {
        assert!(inputs(Some(true), Preference::Light, true, false).shown_dark());
        assert!(!inputs(Some(false), Preference::Dark, true, true).shown_dark());
        assert!(!inputs(None, Preference::Light, true, true).shown_dark());
        assert!(inputs(None, Preference::Default, true, false).shown_dark());
        assert!(!inputs(Some(true), Preference::Dark, false, false).shown_dark());
        assert!(inputs(None, Preference::Light, false, true).shown_dark());
    }
}
