//! The window in a headless weston (Phase 10 plan, Task 4; layer 3): the preview's pixels for
//! the expected frames of modes 1, 10 and 13 in every rotation, no redraw for an unchanged frame,
//! the banners and states in the design's order and words, the feedback label, the requests,
//! and the accessible names. Against a fake client: the test gives the window the service's
//! state and records what it asks.
//! It runs in a headless weston through `common::run`. Behind the `weston-tests` feature.

mod common;

use std::collections::BTreeSet;
use std::process::ExitCode;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use odc_app::model::{SERVICE_BANNER, View};
use odc_app::theme::Tokens;
use odc_app::ui::window::Window;
use odc_app::ui::{Reason, Refused, Request, preview};
use odc_ipc::CoolerState;
use odc_protocol::Frame;
use odc_testkit::expected::load_mode;

use common::{
    Check, Renderer, base, close, dots_of, near, run_for, run_until, shown, shown_saying,
};

fn main() -> ExitCode {
    common::run(&CHECKS)
}

const CHECKS: [(&str, Check); 7] = [
    ("the_preview_shows_every_expected_frame", preview_pixels),
    ("an_unchanged_frame_draws_nothing", no_redraw),
    ("banners_and_states_follow_the_design", banners_and_states),
    ("sending_fails_shows_after_2_s", sending_fails),
    ("the_feedback_label", feedback),
    ("the_window_s_requests", requests),
    ("every_name_of_the_design_s_table", names),
];

// Each case of the expected frames of modes 1, 10 (scrolling) and 13 (silence), in all four
// rotations as the files give them: every dot's centre is lit or unlit exactly as the frame says.
fn preview_pixels(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(base());
    let tokens = Tokens::default_theme(true);
    let (lit, unlit) = (tokens.fg, tokens.unlit());
    let renderer = Renderer::new();
    let area = window.preview().widget().clone();
    assert_eq!(
        (area.width(), area.height()),
        (preview::SIDE, preview::SIDE)
    );
    for mode in [1, 10, 13] {
        let mut checked = 0;
        for case in load_mode(mode).cases {
            for picture in case.kind.pictures() {
                // Mode 13's spectrum pixels may be anything; in silence they're off.
                let Some(frame) = picture.in_silence().to_frame() else {
                    continue;
                };
                // A new frame draws at the next frame-clock tick; the same one draws nothing.
                if window.preview().frame() != Some(frame) {
                    let before = window.preview().draws();
                    window.frame(frame);
                    run_until("the preview draws", || window.preview().draws() > before);
                }
                assert_eq!(window.preview().frame(), Some(frame));
                let colours = dots_of(&renderer.0, &area, preview::PITCH);
                for row in 0..Frame::SIZE {
                    for col in 0..Frame::SIZE {
                        let [r, g, b, _] = colours[row * Frame::SIZE + col];
                        let colour = [r, g, b];
                        let want = if frame.get(row + 1, col + 1) {
                            lit
                        } else {
                            unlit
                        };
                        assert!(
                            near(colour, want),
                            "mode {mode}, {}: dot {row},{col} is {colour:?}, not {want:?}",
                            case.heading
                        );
                    }
                }
                checked += 1;
            }
        }
        assert!(checked > 0, "mode {mode} has frames");
        print!("mode {mode}: {checked} frames; ");
    }
    drop(renderer);
    close(window);
}

// The same frame again draws nothing; a new one draws once, and so does a theme change.
fn no_redraw(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(base());
    let mut one = Frame::blank();
    one.set(3, 4, true);
    window.frame(one);
    run_for(Duration::from_millis(150));
    let before = window.preview().draws();
    for _ in 0..5 {
        window.frame(one);
        run_for(Duration::from_millis(40));
    }
    assert_eq!(window.preview().draws(), before, "the same frame drew");
    let mut two = one;
    two.set(5, 6, true);
    window.frame(two);
    run_for(Duration::from_millis(150));
    assert_eq!(window.preview().draws(), before + 1, "a new frame");
    window.set_theme(&Tokens::default_theme(false));
    run_for(Duration::from_millis(150));
    assert_eq!(window.preview().draws(), before + 2, "a theme change");
    close(window);
}

fn assert_banners(window: &Window, service: bool, cooler: Option<&str>, what: &str) {
    assert_eq!(window.banners().service_shown(), service, "{what}");
    assert_eq!(
        window.banners().cooler_text().as_deref(),
        cooler,
        "{what}: the cooler banner"
    );
}

// The service banner, each cooler banner and the not-available screen, each in the design's
// states and words; with the service gone, the service state first, whatever the session.
fn banners_and_states(app: &gtk::Application) {
    let ((window, _), said) = shown_saying(app);
    // The window starts as the service down, until the client says otherwise.
    window.down();
    assert_banners(&window, true, None, "service down");
    assert_eq!(window.screen(), ("main".to_owned(), "down".to_owned()));
    assert_eq!(window.header(), [(true, false), (true, false)]);
    assert!(!window.actions_enabled());
    assert_eq!(
        window.column_words(),
        [
            "No picture",
            "The service draws the picture",
            "Cooler: unknown",
            "Service not running",
            "—",
            "—"
        ]
    );
    assert_eq!(
        window.names().description("Service banner").as_deref(),
        Some(SERVICE_BANNER.join(" ").as_str())
    );
    assert_eq!(window.preview().frame(), None, "all unlit");

    window.state(base());
    assert_banners(&window, false, None, "connected");
    assert_eq!(window.screen(), ("main".to_owned(), "display".to_owned()));
    assert_eq!(window.header(), [(true, true), (true, true)]);
    assert!(window.actions_enabled());
    assert_eq!(
        window.column_words(),
        [
            "Load · CPU",
            "Shown on the cooler now",
            "Cooler connected",
            "Service running",
            "1,180 RPM",
            "3,400 RPM"
        ]
    );
    for (cooler, banner, status, line) in [
        (
            Some(CoolerState::NotFound),
            "The cooler isn't connected. Changes are still saved, and the cooler shows them when it's back.",
            "Cooler not connected",
            "Not on the cooler: it isn't connected",
        ),
        (
            Some(CoolerState::NoAccess),
            "The cooler is plugged in, but the service can't open it. The udev rule that gives the service access is missing or not applied yet. Restart the PC; if that doesn't help, reinstall OpenDeepCool. The journal says more.",
            "Cooler: no access",
            "Not on the cooler: the service can't open it",
        ),
        (
            Some(CoolerState::Released),
            "The cooler isn't connected. Changes are still saved, and the cooler shows them when it's back.",
            "Cooler not connected",
            "Not on the cooler: it isn't connected",
        ),
        (
            None,
            "The cooler isn't connected. Changes are still saved, and the cooler shows them when it's back.",
            "Cooler not connected",
            "Not on the cooler: it isn't connected",
        ),
    ] {
        let mut state = base();
        state.cooler_state = cooler;
        window.state(state);
        assert_banners(&window, false, Some(banner), &format!("{cooler:?}"));
        let words = window.column_words();
        assert_eq!(words[1], line, "{cooler:?}");
        assert_eq!(words[2], status, "{cooler:?}");
        assert_eq!(
            window.header(),
            [(true, true), (true, true)],
            "nothing disabled"
        );
    }

    // A Music mode adds the sound.
    let mut state = base();
    state.mode = Some(13);
    window.state(state);
    assert_eq!(window.column_words()[4], "Sound connected");

    // Not available in this session: only the message.
    window.refused();
    assert_eq!(
        window.screen().0,
        "not-available",
        "only the not-available message"
    );
    assert_eq!(window.header(), [(false, false), (false, false)]);
    assert_banners(&window, false, None, "not available");
    assert!(!window.actions_enabled());
    assert!(window.not_available().is_visible());
    assert_eq!(window.shown(), Some(View::NotAvailable));
    // Announced once on entering it, not again while it stays.
    window.refused();
    assert_eq!(
        *said.borrow(),
        [
            "OpenDeepCool can be used only from the session at the PC. This session is remote or not the active one."
        ]
    );

    // The service gone: the service state, whatever the session was.
    window.down();
    assert_banners(&window, true, None, "down after a refusal");
    assert_eq!(window.screen().0, "main");
    close(window);
}

// `write-failed` shows as connected for 2 s, then its banner; any other state ends the wait.
fn sending_fails(app: &gtk::Application) {
    let (window, _) = shown(app);
    let mut state = base();
    state.cooler_state = Some(CoolerState::WriteFailed);
    window.state(state.clone());
    assert_banners(&window, false, None, "under 2 s");
    run_for(Duration::from_millis(1700));
    assert_banners(&window, false, None, "still under 2 s");
    run_until("the 2 s pass", || window.banners().cooler_text().is_some());
    assert_eq!(
        window.banners().cooler_text().as_deref(),
        Some(
            "The cooler is connected, but sending it the picture fails. The service keeps trying by itself, and changes are still saved. The journal says more."
        )
    );
    assert_eq!(window.column_words()[2], "Cooler: sending fails");
    // Connected again: the banner goes; failing again waits 2 s again.
    window.state(base());
    assert_banners(&window, false, None, "connected again");
    window.state(state);
    run_for(Duration::from_millis(500));
    assert_banners(&window, false, None, "a new wait");
    close(window);
}

// "Refreshed" for 1.5 s; a refusal's words.
fn feedback(app: &gtk::Application) {
    let ((window, _), said) = shown_saying(app);
    window.state(base());
    assert_eq!(window.feedback(), None);
    window.refreshed();
    assert_eq!(window.feedback().as_deref(), Some("Refreshed"));
    run_for(Duration::from_millis(1300));
    assert_eq!(window.feedback().as_deref(), Some("Refreshed"));
    run_until("the label goes", || window.feedback().is_none());
    window.refusal(Refused::Change, &Reason::NotAuthorized);
    assert_eq!(
        window.feedback().as_deref(),
        Some("Not changed. This session may not change settings.")
    );
    window.refusal(
        Refused::Refresh,
        &Reason::Message("the service failed".to_owned()),
    );
    assert_eq!(
        window.feedback().as_deref(),
        Some("Not refreshed. the service failed")
    );
    assert_eq!(
        *said.borrow(),
        [
            "Not changed. This session may not change settings.",
            "Not refreshed. the service failed"
        ],
        "screen readers get the same words"
    );
    close(window);
}

// Frames are watched once mapped; Refresh and the pages through their actions, and only where
// they can be used; the choose-input link opens the Configuration page.
fn requests(app: &gtk::Application) {
    let (window, asked) = shown(app);
    assert!(asked.borrow().contains(&Request::WatchFrames(true)));
    window.down();
    asked.borrow_mut().clear();
    WidgetExt::activate_action(window.widget(), "win.refresh", None).ok();
    run_for(Duration::from_millis(20));
    assert!(
        !asked.borrow().contains(&Request::Refresh),
        "no Refresh without the service"
    );
    let mut state = base();
    state.cooler_fan_input = Some(None);
    window.state(state);
    WidgetExt::activate_action(window.widget(), "win.refresh", None).expect("the refresh action");
    run_for(Duration::from_millis(20));
    assert_eq!(*asked.borrow(), [Request::Refresh]);
    let link = window
        .names()
        .find("Choose cooler fans input")
        .and_then(|w| w.downcast::<gtk::Button>().ok())
        .expect("the link");
    assert!(link.is_visible());
    link.emit_clicked();
    assert_eq!(window.screen().1, "config");
    WidgetExt::activate_action(window.widget(), "win.page-display", None).expect("the page action");
    assert_eq!(window.screen().1, "display");
    WidgetExt::activate_action(window.widget(), "win.page-config", None).expect("the page action");
    assert_eq!(window.screen().1, "config");
    let copy = window
        .names()
        .find("Copy start command")
        .and_then(|w| w.downcast::<gtk::Button>().ok())
        .expect("the copy button");
    copy.emit_clicked();
    let text = glib::MainContext::default()
        .block_on(copy.clipboard().read_text_future())
        .expect("the clipboard");
    assert_eq!(
        text.as_deref(),
        Some("sudo systemctl start opendeepcool.service")
    );
    close(window);
}

/// The design's tile names (Accessible names), typed here apart from the app's.
const TILE_NAMES: [&str; 23] = [
    "Load: CPU",
    "Load: GPU",
    "Load: RAM",
    "Load: CPU + GPU",
    "Load: CPU + RAM",
    "Load: GPU + RAM",
    "Temperature: CPU",
    "Temperature: GPU",
    "Temperature: CPU + GPU",
    "Power: CPU",
    "Power: GPU",
    "Power: CPU + GPU",
    "Music: Spectrum",
    "Music: Emoji 1",
    "Music: Emoji 2",
    "Music: Emoji 3",
    "Music: Emoji 4",
    "Music: CPU temperature",
    "Music: GPU temperature",
    "Music: CPU usage",
    "Music: GPU usage",
    "Music: RAM usage",
    "Music: Dancer",
];

// Every name of the design's table built so far, typed here apart from the window's: each set
// on its widget, with the widget's role.
fn names(app: &gtk::Application) {
    use gtk::AccessibleRole as R;
    let (window, _) = shown(app);
    let mut want: Vec<(&str, R)> = vec![
        ("Display", R::ToggleButton),
        ("Configuration", R::ToggleButton),
        ("Refresh display", R::Button),
        ("Change feedback", R::Label),
        ("Service banner", R::Group),
        ("Copy start command", R::Button),
        ("Not available in this session", R::Group),
        ("Cooler banner", R::Group),
        ("Cooler display preview", R::Img),
        ("Unavailable reading", R::Label),
        ("Cooler status", R::Label),
        ("Service status", R::Label),
        ("Music sound status", R::Label),
        ("Cooler fans speed", R::Label),
        ("Pump speed", R::Label),
        ("Choose cooler fans input", R::Button),
        ("Choose pump input", R::Button),
        // The Display page (Task 5).
        ("Celsius", R::ToggleButton),
        ("Fahrenheit", R::ToggleButton),
        ("Load options", R::Grid),
        ("Temperature options", R::Grid),
        ("Power options", R::Grid),
        ("Music options", R::Grid),
        ("Load unavailable reasons", R::Label),
        ("Temperature unavailable reasons", R::Label),
        ("Power unavailable reasons", R::Label),
        ("Music unavailable reasons", R::Label),
    ];
    want.extend(TILE_NAMES.iter().map(|name| (*name, R::GridCell)));
    let labels: BTreeSet<String> = window.names().labels().into_iter().collect();
    let typed: BTreeSet<String> = want.iter().map(|(n, _)| (*n).to_owned()).collect();
    assert_eq!(labels, typed);
    assert_eq!(labels.len(), want.len(), "each name once");
    for (name, role) in want {
        let widget = window.names().find(name).expect(name);
        assert!(
            gtk::test_accessible_has_property(&widget, gtk::AccessibleProperty::Label),
            "{name}: no label"
        );
        assert_eq!(widget.accessible_role(), role, "{name}");
    }
    window.state(base());
    assert_eq!(
        window
            .names()
            .description("Cooler display preview")
            .as_deref(),
        Some("Load · CPU")
    );
    close(window);
}
