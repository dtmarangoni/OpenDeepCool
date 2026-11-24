use super::*;

// Row "Cooler disconnected (`not-found`)": its banner and status; everything still changeable.
#[test]
fn cooler_not_found() {
    let mut state = base();
    state.cooler_state = Some(CoolerState::NotFound);
    assert_eq!(
        view_of(state),
        cooler_view(
            ("Cooler not connected", Dot::Warn),
            "Not on the cooler: it isn't connected.",
            Some(Banner::NotConnected)
        )
    );
}

// Row "Cooler found, no access (`no-access`)": the error banner with the fix.
#[test]
fn cooler_no_access() {
    let mut state = base();
    state.cooler_state = Some(CoolerState::NoAccess);
    assert_eq!(
        view_of(state),
        cooler_view(
            ("Cooler: no access", Dot::Bad),
            "Not on the cooler: the service can't open it.",
            Some(Banner::NoAccess)
        )
    );
    assert!(Banner::NoAccess.is_error());
    assert_eq!(
        Banner::NoAccess.text().0,
        "The cooler is plugged in, but the service can't open it."
    );
}

// Row "Sending to the cooler fails, 2 s or more": the warning banner.
#[test]
fn cooler_sending_fails_for_2_s() {
    let mut state = base();
    state.cooler_state = Some(CoolerState::WriteFailed);
    let ui = Ui {
        sending_failed_long: true,
        ..Ui::default()
    };
    assert_eq!(
        view_with(state, ui),
        cooler_view(
            ("Cooler: sending fails", Dot::Warn),
            "Not on the cooler: sending it fails.",
            Some(Banner::SendingFails)
        )
    );
    assert!(!Banner::SendingFails.is_error());
}

// Row "Sending fails, under 2 s": as connected, no banner.
#[test]
fn cooler_sending_fails_under_2_s() {
    let mut state = base();
    state.cooler_state = Some(CoolerState::WriteFailed);
    assert_eq!(view_of(state), base_view());
}

// The Theme card in each state of the design's table (Theme card, States): what the two controls
// and the line show, from the service's two settings and what the app knows of the system.
#[test]
fn the_theme_card_in_each_state_of_the_design() {
    use crate::choice::{Facts, Preference as P};
    let card = |omarchy: Option<bool>, preference: P, use_system: bool, dark: bool| {
        let mut state = base();
        state.use_system_theme = Some(use_system);
        state.dark_theme = Some(dark);
        let ui = Ui {
            system: Facts {
                omarchy,
                preference,
            },
            ..Ui::default()
        };
        view_with(state, ui).config.theme
    };
    let card_of = |use_system, sensitive, dark, toggles, line| ThemeCard {
        use_system,
        use_system_sensitive: sensitive,
        dark,
        toggles_sensitive: toggles,
        opacity: WindowOpacity::Opaque,
        corners: WindowCorners::Rounded,
        line,
    };
    // (a) An Omarchy theme is found and followed: the toggles dimmed on its mode, whatever the
    // preference and `DarkTheme` say.
    for preference in [P::Unsupported, P::Default, P::Dark, P::Light] {
        for dark in [true, false] {
            assert_eq!(
                card(Some(true), preference, true, dark),
                card_of(true, true, true, false, ThemeLine::Omarchy)
            );
            assert_eq!(
                card(Some(false), preference, true, dark),
                card_of(true, true, false, false, ThemeLine::Omarchy)
            );
        }
    }
    // (a) No Omarchy theme; the system's preference is followed.
    for dark in [true, false] {
        assert_eq!(
            card(None, P::Dark, true, dark),
            card_of(
                true,
                true,
                true,
                false,
                ThemeLine::OwnAsDesktopPrefers { dark: true }
            )
        );
        assert_eq!(
            card(None, P::Light, true, dark),
            card_of(
                true,
                true,
                false,
                false,
                ThemeLine::OwnAsDesktopPrefers { dark: false }
            )
        );
        assert_eq!(
            card(None, P::Default, true, dark),
            card_of(true, true, true, false, ThemeLine::OwnNoPreference)
        );
    }
    // (b) The system's theme is found but turned off: the service's `DarkTheme` shows.
    for (omarchy, preference) in [(Some(false), P::Light), (None, P::Dark), (None, P::Default)] {
        for dark in [true, false] {
            assert_eq!(
                card(omarchy, preference, false, dark),
                card_of(false, true, dark, true, ThemeLine::Own { dark })
            );
        }
    }
    // (c) No system theme can be found: the switch off and dimmed whatever the service holds,
    // the toggles usable, the warning.
    for use_system in [true, false] {
        for dark in [true, false] {
            assert_eq!(
                card(None, P::Unsupported, use_system, dark),
                card_of(false, false, dark, true, ThemeLine::NotFound)
            );
        }
    }
    assert!(ThemeLine::NotFound.is_warning());
    assert!(!ThemeLine::Omarchy.is_warning());
    assert_eq!(
        ThemeLine::NotFound.text(),
        "OpenDeepCool can't find the system's theme and is using its own."
    );
}

// Rows "Service not running" and "Off" (D10): the off screen in place of the pages, the column
// unlit, and the service's status as the off screen's heading says: off on purpose, hollow, or
// not running, red; both wait for the first read of systemd.
#[test]
fn service_not_running() {
    let column = |service: Option<(&'static str, Dot)>| {
        let mut status = vec![Status {
            name: "Cooler status",
            text: "Cooler: unknown",
            dot: Dot::Off,
        }];
        status.extend(service.map(|(text, dot)| Status {
            name: "Service status",
            text,
            dot,
        }));
        Column {
            caption: "No picture".to_owned(),
            away: None,
            reason: None,
            status,
        }
    };
    let new_install = Boot {
        known: Known::Facts(Facts {
            service_file: UnitFile::Disabled,
            service: Run::Stopped,
            sound_file: UnitFile::Disabled,
            sound: false,
        }),
        ..Boot::default()
    };
    let failed = Boot {
        known: Known::Facts(Facts {
            service_file: UnitFile::Enabled,
            service: Run::Failed,
            sound_file: UnitFile::Enabled,
            sound: true,
        }),
        ..Boot::default()
    };
    for (startup, status, heading) in [
        (
            &new_install,
            Some(("Service off", Dot::Off)),
            Some("OpenDeepCool is off"),
        ),
        (
            &failed,
            Some(("Service not running", Dot::Bad)),
            Some("OpenDeepCool isn't running"),
        ),
        (&Boot::default(), None, None),
    ] {
        assert_eq!(
            view(&Service::Down, Ui::default(), &products(), startup),
            View::ServiceDown(Box::new(Down {
                column: column(status),
                screen: boot::off_screen(startup),
            }))
        );
        assert_eq!(boot::off_screen(startup).heading, heading);
    }
    // The Startup card follows the switch, on the normal screens too.
    let normal = view(
        &Service::Up(Box::new(base())),
        Ui::default(),
        &products(),
        &failed,
    );
    let View::Normal(normal) = normal else {
        panic!("not the normal screens");
    };
    assert_eq!(normal.startup, boot::switch(&failed));
    assert!(
        normal
            .startup
            .note
            .is_some_and(|n| n.text.contains("failed"))
    );
}

// Row "Short read failure, first second": the service sends no reason then, so nothing is
// disabled; the preview mirrors the cooler's dashes.
#[test]
fn a_short_read_failure() {
    let mut state = base();
    state.unavailable_reasons = Some(HashMap::new());
    assert_eq!(view_of(state), base_view());
}

// Rows "A change refused" and "Back from standby": no state of their own. A refused change
// leaves the service's state as it was, so the controls show the saved value; after standby
// the same state gives the same view. The theme rows are the theme's tests.
#[test]
fn rows_without_a_state_of_their_own() {
    let before = view_of(base());
    // The app asked for mode 4; the service refused it, and sent nothing.
    assert_eq!(view_of(base()), before);
    assert!(before.display.groups[1].tiles[2].selected);
}
