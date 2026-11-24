//! The Theme card and the theme it chooses, in a headless weston with the XDG desktop portal's
//! fake (Phase 11a plan, Task 11; layer 3; `docs/design/settings-app.md`, Theme card, Which theme
//! is used): GTK fills `gtk-interface-color-scheme` from the portal, and the window follows it
//! live. Each preference runs in a process of its own (`--portal=none|default|dark|light`): GDK
//! reads the portal at start, and a bus without it, or none, reads `Unsupported`. Behind the
//! `weston-tests` feature.

mod common;

use std::process::{Command, ExitCode};
use std::time::Duration;

use gtk::prelude::*;
use odc_app::theme::Tokens;
use odc_app::theme_choice::Preference;
use odc_app::ui::Request;
use odc_app::ui::window::Window;
use odc_ipc::State;

use common::{Check, base, close, flip_portal, run_for, run_until, shown};

const MODES: [&str; 4] = ["none", "default", "dark", "light"];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.iter().find_map(|a| a.strip_prefix("--portal="));
    if let Some(mode) = mode {
        let (checks, scheme): (&[(&str, Check)], _) = match mode {
            "none" => (&NONE, None),
            "default" => (&DEFAULT, Some(0)),
            "dark" => (&DARK, Some(1)),
            _ => (&LIGHT, Some(2)),
        };
        return common::run_with_portal(checks, (1280, 1100), scheme);
    }
    // The parent: each preference in a process of its own.
    for mode in MODES {
        let status = Command::new(std::env::current_exe().expect("this test"))
            .args(args.iter().skip(1))
            .arg(format!("--portal={mode}"))
            .status()
            .expect("the checks start");
        if !status.success() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

const NONE: [(&str, Check); 3] = [
    (
        "no_portal_reads_unsupported_and_no_system_theme_is_found",
        none_found,
    ),
    (
        "the_service_s_choice_decides_without_a_system_theme",
        none_choice,
    ),
    ("an_omarchy_theme_found_is_the_system_s_theme", none_omarchy),
];
const DEFAULT: [(&str, Check); 1] = [("no_preference_shows_the_dark_theme", no_preference)];
const DARK: [(&str, Check); 2] = [
    ("a_dark_preference_is_followed_live", dark_then_light),
    (
        "turning_the_system_theme_off_stops_following",
        stops_following,
    ),
];
const LIGHT: [(&str, Check); 1] = [("a_light_preference_opens_in_the_light_theme", light_first)];

/// A state with the service's two theme settings.
fn with(use_system: bool, dark: bool) -> State {
    State {
        use_system_theme: Some(use_system),
        dark_theme: Some(dark),
        ..base()
    }
}

/// The window on the Configuration page, with the service's settings given.
fn open(app: &gtk::Application, use_system: bool, dark: bool) -> (Window, common::Log<Request>) {
    let (window, asked) = shown(app);
    window.state(with(use_system, dark));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(250));
    (window, asked)
}

fn preference() -> gtk::InterfaceColorScheme {
    gtk::Settings::default()
        .expect("settings")
        .gtk_interface_color_scheme()
}

/// What the card shows: (switch on, switch usable, Dark pressed, buttons usable, the line).
fn card(window: &Window) -> (bool, bool, bool, bool, String) {
    window.config_page().theme_shown()
}

fn own_line(dark: bool) -> String {
    format!(
        "Using OpenDeepCool's {} theme.",
        if dark { "dark" } else { "light" }
    )
}

fn desktop_line(dark: bool) -> String {
    format!(
        "Using OpenDeepCool's {} theme, as your desktop prefers.",
        if dark { "dark" } else { "light" }
    )
}

// Without a portal GTK reads `Unsupported`, so no system theme can be found: the switch off and
// dimmed, the toggles usable, the warning, and OpenDeepCool's own dark theme (the default).
fn none_found(app: &gtk::Application) {
    assert_eq!(preference(), gtk::InterfaceColorScheme::Unsupported);
    let (window, _) = open(app, true, true);
    assert_eq!(
        card(&window),
        (
            false,
            false,
            true,
            true,
            "OpenDeepCool can't find the system's theme and is using its own.".to_owned()
        )
    );
    assert!(window.config_page().theme_warning());
    assert!(window.prefers_dark());
    close(window);
}

// The service's `DarkTheme` decides, whatever `UseSystemTheme` holds; each button asks for its
// value and the pressed one stays the saved one until the service says so.
fn none_choice(app: &gtk::Application) {
    let (window, asked) = open(app, true, false);
    assert!(!card(&window).2, "Light pressed");
    assert!(!window.prefers_dark(), "the light theme");
    window.state(with(false, true));
    assert!(card(&window).2 && window.prefers_dark(), "dark again");
    asked.borrow_mut().clear();
    let dark_button = |name: &str| {
        window
            .names()
            .find(name)
            .and_then(|w| w.downcast::<gtk::ToggleButton>().ok())
            .expect(name)
    };
    dark_button("Light theme").emit_clicked();
    run_for(Duration::from_millis(30));
    assert_eq!(*asked.borrow(), [Request::SetDarkTheme(false)]);
    assert!(
        card(&window).2,
        "Dark is still pressed until the service says so"
    );
    assert!(
        window.prefers_dark(),
        "the theme changes only when the service confirms"
    );
    window.state(with(false, false));
    assert!(!card(&window).2 && !window.prefers_dark());
    close(window);
}

// An Omarchy theme found is the system's theme: followed, it is shown, the toggles dimmed on its
// mode; the switch turned off shows OpenDeepCool's own.
fn none_omarchy(app: &gtk::Application) {
    let (window, _) = open(app, true, true);
    let mut light = Tokens::default_theme(false);
    light.accent = Tokens::default_theme(true).green; // an Omarchy theme of its own colours
    window.omarchy_theme(Some(light));
    run_for(Duration::from_millis(100));
    assert_eq!(
        card(&window),
        (
            true,
            true,
            false,
            false,
            "Using the Omarchy theme.".to_owned()
        )
    );
    assert!(!window.config_page().theme_warning());
    assert!(!window.prefers_dark(), "its mode is light");
    // Not followed: OpenDeepCool's own, as the service keeps it.
    window.state(with(false, true));
    assert_eq!(card(&window), (false, true, true, true, own_line(true)));
    assert!(window.prefers_dark());
    // Followed again, then the theme goes away: no system theme to follow.
    window.state(with(true, true));
    assert!(!window.prefers_dark());
    window.omarchy_theme(None);
    assert!(!card(&window).1, "the switch is dimmed again");
    assert!(window.prefers_dark());
    close(window);
}

// A portal that answers "no preference" gives OpenDeepCool's dark theme, and says so.
fn no_preference(app: &gtk::Application) {
    assert_eq!(preference(), gtk::InterfaceColorScheme::Default);
    let (window, _) = open(app, true, false);
    assert_eq!(
        card(&window),
        (
            true,
            true,
            true,
            false,
            "Using OpenDeepCool's dark theme. Your desktop has no preference.".to_owned()
        ),
        "dark, whatever DarkTheme holds, since the system is followed"
    );
    assert!(window.prefers_dark());
    close(window);
}

// The system prefers dark: followed, dark; and when the preference flips to light the window
// follows within a second, the pressed button with it.
fn dark_then_light(app: &gtk::Application) {
    assert_eq!(preference(), gtk::InterfaceColorScheme::Dark);
    let (window, _) = open(app, true, true);
    assert_eq!(card(&window), (true, true, true, false, desktop_line(true)));
    assert!(window.prefers_dark());
    flip_portal(2);
    run_until("the preference follows", || {
        preference() == gtk::InterfaceColorScheme::Light
    });
    run_for(Duration::from_millis(100));
    assert_eq!(
        card(&window),
        (true, true, false, false, desktop_line(false))
    );
    assert!(!window.prefers_dark(), "the light theme, live");
    flip_portal(1);
    run_until("dark again", || {
        preference() == gtk::InterfaceColorScheme::Dark
    });
    run_for(Duration::from_millis(100));
    assert!(window.prefers_dark());
    // An Omarchy theme found wins over the preference while the system's theme is followed.
    let mut omarchy = Tokens::default_theme(false);
    omarchy.accent = Tokens::default_theme(true).green;
    window.omarchy_theme(Some(omarchy));
    assert!(!window.prefers_dark());
    assert_eq!(card(&window).4, "Using the Omarchy theme.");
    close(window);
}

// With the system's theme turned off the preference no longer matters: the service's choice
// decides, and a flip of the preference changes nothing.
fn stops_following(app: &gtk::Application) {
    let (window, _) = open(app, false, false);
    assert_eq!(card(&window), (false, true, false, true, own_line(false)));
    assert!(
        !window.prefers_dark(),
        "the service says light, though the system prefers dark"
    );
    flip_portal(2);
    run_until("the preference follows", || {
        preference() == gtk::InterfaceColorScheme::Light
    });
    window.state(with(false, true));
    flip_portal(1);
    run_until("dark again", || {
        preference() == gtk::InterfaceColorScheme::Dark
    });
    run_for(Duration::from_millis(100));
    assert!(window.prefers_dark(), "as the service keeps it");
    close(window);
}

// The system prefers light: the window opens in OpenDeepCool's light theme.
fn light_first(app: &gtk::Application) {
    assert_eq!(preference(), gtk::InterfaceColorScheme::Light);
    let (window, _) = open(app, true, true);
    assert_eq!(
        card(&window),
        (true, true, false, false, desktop_line(false))
    );
    assert!(!window.prefers_dark());
    let _ = Preference::Light;
    close(window);
}
