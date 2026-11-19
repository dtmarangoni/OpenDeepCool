//! The window in a headless weston (Phase 10 plan, Task 4; layer 3): the preview's pixels for
//! the expected frames of modes 1, 10 and 13 in every rotation, no redraw for an unchanged frame,
//! the banners and states in the design's order and words, the off screen and the Start at boot
//! switch on both screens (Phase 11 plan, Task 3), the feedback label, the requests, and the
//! accessible names. Against a fake client: the test gives the window the service's state and
//! systemd's, and records what it asks.
//! It runs in a headless weston through `common::run`. Behind the `weston-tests` feature.

mod common;

use std::collections::BTreeSet;
use std::process::ExitCode;
use std::time::Duration;

use gtk::prelude::*;
use odc_app::boot::{Boot, Direction, Facts, Failure, Known, Run, UnitFile};
use odc_app::model::View;
use odc_app::theme::Tokens;
use odc_app::ui::window::Window;
use odc_app::ui::{Reason, Refused, Request, preview};
use odc_ipc::CoolerState;
use odc_protocol::Frame;
use odc_testkit::expected::load_mode;

use common::{
    Check, Renderer, base, close, dots_of, near, run_for, run_until, save_png, shown, shown_saying,
};

fn main() -> ExitCode {
    common::run(&CHECKS)
}

const CHECKS: [(&str, Check); 9] = [
    ("the_preview_shows_every_expected_frame", preview_pixels),
    ("an_unchanged_frame_draws_nothing", no_redraw),
    ("banners_and_states_follow_the_design", banners_and_states),
    ("the_start_at_boot_switch_on_both_screens", start_at_boot),
    ("sending_fails_shows_after_2_s", sending_fails),
    ("the_feedback_label", feedback),
    ("the_header_and_the_two_pages", header_and_pages),
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

/// The four facts, in a [`Boot`] read from systemd.
fn read(service_file: UnitFile, service: Run, sound_file: UnitFile, sound: bool) -> Boot {
    Boot {
        known: Known::Facts(Facts {
            service_file,
            service,
            sound_file,
            sound,
        }),
        ..Boot::default()
    }
}

fn new_install() -> Boot {
    read(UnitFile::Disabled, Run::Stopped, UnitFile::Disabled, false)
}

fn all_on() -> Boot {
    read(UnitFile::Enabled, Run::Running, UnitFile::Enabled, true)
}

fn assert_banners(window: &Window, cooler: Option<&str>, what: &str) {
    assert_eq!(
        window.banners().cooler_text().as_deref(),
        cooler,
        "{what}: the cooler banner"
    );
}

// Each cooler banner, the off screen and the not-available screen, each in the design's states
// and words; with the service gone, the off screen first, whatever the session.
fn banners_and_states(app: &gtk::Application) {
    let ((window, _), said) = shown_saying(app);
    // The window starts as the service down, until the client says otherwise; the off screen
    // waits for systemd's first read, so no heading flashes.
    window.down();
    assert_banners(&window, None, "service down");
    assert_eq!(window.screen(), ("main".to_owned(), "off".to_owned()));
    assert_eq!(window.off_heading(), None, "not read yet");
    assert!(!window.startup().card().is_visible());
    // The gear stays, insensitive and not pressed; the close icon always works (Window, The
    // header, by state). Refresh is on the Display page, which the off screen replaces.
    assert_eq!(window.header(), [(true, false), (true, true)]);
    assert!(!window.gear_pressed());
    assert_eq!(window.actions_enabled(), [false; 3]);
    assert_eq!(
        window.column_words(),
        ["No picture", "Cooler: unknown"],
        "no line under the caption"
    );
    assert_eq!(
        window
            .names()
            .description("Cooler display preview")
            .as_deref(),
        Some("No picture")
    );
    window.boot(new_install());
    assert_eq!(window.off_heading().as_deref(), Some("OpenDeepCool is off"));
    assert_eq!(window.column_words()[2], "Service off");
    assert_eq!(
        window.names().description("Service message").as_deref(),
        Some("OpenDeepCool is off. The cooler shows nothing until the display service runs.")
    );
    window.boot(read(
        UnitFile::Enabled,
        Run::Failed,
        UnitFile::Enabled,
        true,
    ));
    assert_eq!(
        window.off_heading().as_deref(),
        Some("OpenDeepCool isn't running")
    );
    assert_eq!(window.column_words()[2], "Service not running");
    assert_eq!(window.preview().frame(), None, "all unlit");
    window.boot(all_on());

    window.state(base());
    assert_banners(&window, None, "connected");
    assert_eq!(window.screen(), ("main".to_owned(), "display".to_owned()));
    assert_eq!(window.header(), [(true, true), (true, true)]);
    assert!(!window.gear_pressed());
    // Configuration and Refresh work; Back only on the Configuration page.
    assert_eq!(window.actions_enabled(), [true, false, true]);
    let refresh = window.display_page().refresh();
    assert!(refresh.is_visible() && refresh.is_sensitive());
    assert_eq!(
        window.column_words(),
        ["Load · CPU", "Cooler connected", "Service running"]
    );
    assert_eq!(
        window
            .names()
            .description("Cooler display preview")
            .as_deref(),
        Some("Load · CPU"),
        "the caption alone while the cooler has the picture"
    );
    for (cooler, banner, status, away) in [
        (
            Some(CoolerState::NotFound),
            "The cooler isn't connected. Changes are still saved, and the cooler shows them when it's back.",
            "Cooler not connected",
            "Not on the cooler: it isn't connected.",
        ),
        (
            Some(CoolerState::NoAccess),
            "The cooler is plugged in, but the service can't open it. The udev rule that gives the service access is missing or not applied yet. Restart the PC; if that doesn't help, reinstall OpenDeepCool. The journal says more.",
            "Cooler: no access",
            "Not on the cooler: the service can't open it.",
        ),
        (
            Some(CoolerState::Released),
            "The cooler isn't connected. Changes are still saved, and the cooler shows them when it's back.",
            "Cooler not connected",
            "Not on the cooler: it isn't connected.",
        ),
        (
            None,
            "The cooler isn't connected. Changes are still saved, and the cooler shows them when it's back.",
            "Cooler not connected",
            "Not on the cooler: it isn't connected.",
        ),
    ] {
        let mut state = base();
        state.cooler_state = cooler;
        window.state(state);
        assert_banners(&window, Some(banner), &format!("{cooler:?}"));
        let words = window.column_words();
        assert_eq!(words.len(), 3, "the column keeps its lines: {cooler:?}");
        assert_eq!(words[1], status, "{cooler:?}");
        assert_eq!(
            window
                .names()
                .description("Cooler display preview")
                .as_deref(),
            Some(format!("Load · CPU. {away}").as_str()),
            "{cooler:?}"
        );
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
    assert_eq!(window.column_words()[3], "Sound connected");

    // The off screen's note was announced when it showed (Accessible names).
    assert_eq!(
        said.borrow().first().map(String::as_str),
        Some(
            "Not fully on: the display service failed. Turn it on to finish. The journal says why \
             the service failed."
        )
    );
    said.borrow_mut().clear();
    // Not available in this session: only the message.
    window.refused();
    assert_eq!(
        window.screen().0,
        "not-available",
        "only the not-available message"
    );
    // Only the title and the close icon (Window, The header, by state).
    assert_eq!(window.header(), [(false, false), (true, true)]);
    assert_banners(&window, None, "not available");
    assert_eq!(window.actions_enabled(), [false; 3]);
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

    // The service gone: the off screen, whatever the session was.
    window.down();
    assert_banners(&window, None, "down after a refusal");
    assert_eq!(window.screen(), ("main".to_owned(), "off".to_owned()));
    close(window);
}

/// The text of every label under `widget`.
fn label_texts(widget: &gtk::Widget, out: &mut Vec<String>) {
    if let Some(label) = widget.downcast_ref::<gtk::Label>() {
        out.push(label.text().to_string());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        label_texts(&next, out);
        child = next.next_sibling();
    }
}

/// The app has no terminal commands, lead-in or Copy button anywhere (owner, 2026-11-19): they
/// are in the README.
fn assert_no_terminal(window: &Window) {
    for name in ["Terminal commands", "Copy terminal commands"] {
        assert!(window.names().find(name).is_none(), "{name}");
    }
    let mut texts = Vec::new();
    label_texts(window.widget().upcast_ref(), &mut texts);
    for text in texts {
        let lower = text.to_lowercase();
        assert!(
            !lower.contains("systemctl") && !lower.contains("terminal"),
            "a terminal command in the app: {text:?}"
        );
    }
}

// The Start at boot switch (Start at boot): on the off screen without its heading, on the
// Configuration page as the last card; one switch, moved between them, and no terminal commands
// anywhere (owner, 2026-11-19: they are in the README). A flip asks the client, keeps the trough at systemd's state,
// and at its end screen readers hear the switch's state and the switch has the focus again.
fn start_at_boot(app: &gtk::Application) {
    let ((window, asked), said) = shown_saying(app);
    let startup = window.startup();
    let switch = startup.switch().clone();
    window.down();
    window.boot(new_install());
    // The off screen: the card, no heading, and no terminal commands.
    assert!(window.startup_on_off_screen());
    assert!(startup.card().is_visible());
    assert!(!startup.heading_shown());
    assert_no_terminal(&window);
    // The row is its title and the switch: no subtitle under the title (owner, 2026-11-19).
    let mut texts = Vec::new();
    label_texts(startup.card().upcast_ref(), &mut texts);
    assert!(texts.iter().all(|t| !t.contains("whole PC")), "{texts:?}");
    // And no hint at the card's foot (owner, 2026-11-19).
    assert!(
        texts.iter().all(|t| !t.starts_with("When it's on")),
        "{texts:?}"
    );
    assert_eq!(startup.note(), None, "a new install has no note");
    assert!(!switch.is_active() && !switch.state() && switch.is_sensitive());
    run_until("the switch takes the first focus", || switch.is_focus());
    let renderer = Renderer::new();
    run_for(Duration::from_millis(100));
    print!(
        "{}; ",
        save_png(&renderer.0, window.widget(), "off-screen").display()
    );

    // A flip: the client is asked, the trough stays off.
    asked.borrow_mut().clear();
    switch.set_active(true);
    run_for(Duration::from_millis(20));
    assert_eq!(*asked.borrow(), [Request::StartAtBoot(true)]);
    assert!(!switch.state(), "the trough keeps systemd's state");
    // The client's flip runs: the handle on, the trough off, insensitive, its line.
    let mut flipping = new_install();
    flipping.begin(Direction::On).expect("a flip");
    window.boot(flipping.clone());
    assert!(switch.is_active() && !switch.state() && !switch.is_sensitive());
    assert_eq!(
        startup.note().as_deref(),
        Some("Turning on… Your desktop asks for the administrator's password first.")
    );
    assert_eq!(
        window.names().description("Start at boot").as_deref(),
        startup.note().as_deref()
    );
    // Closed dialog: back at off, sensitive, with no note (it changed nothing, and the switch
    // shows the state; owner, 2026-11-19); announced as the switch's state, focus back.
    said.borrow_mut().clear();
    flipping.end(Some(Failure::Dismissed(Direction::On)), false);
    window.boot(flipping);
    assert!(!switch.is_active() && !switch.state() && switch.is_sensitive());
    assert_eq!(startup.note(), None, "no note for a closed dialog");
    assert_eq!(*said.borrow(), ["Start at boot is off."]);
    run_until("the focus comes back", || switch.is_focus());

    // A remote or inactive session: the heading, no card, and no commands.
    window.boot(Boot {
        may_flip: false,
        ..new_install()
    });
    assert!(!startup.card().is_visible());
    assert_no_terminal(&window);
    assert_eq!(window.off_heading().as_deref(), Some("OpenDeepCool is off"));

    // The service runs: the card moves to the Configuration page, last, with its heading.
    window.boot(read(
        UnitFile::Disabled,
        Run::Running,
        UnitFile::Disabled,
        false,
    ));
    window.state(base());
    assert!(!window.startup_on_off_screen());
    assert!(startup.card().is_visible() && startup.heading_shown());
    let cards = window.config_page().cards().clone();
    assert_eq!(
        cards.last_child().as_ref(),
        Some(startup.card().upcast_ref()),
        "the last card"
    );
    assert_eq!(
        startup.note().as_deref(),
        Some(
            "Not fully on: the display service runs now, but doesn't start at boot, and the sound \
             for Music is off. Turn it on to finish."
        )
    );
    assert_no_terminal(&window);
    // A failed read: its note alone, and the switch can't be used.
    window.boot(Boot {
        known: Known::Failed,
        ..Boot::default()
    });
    assert_eq!(
        startup.note().as_deref(),
        Some("Can't tell whether OpenDeepCool is on: reading it from systemd failed.")
    );
    assert_no_terminal(&window);
    assert!(!switch.is_sensitive());
    // On: everything holds, no note.
    window.boot(all_on());
    assert!(switch.is_active() && switch.state() && switch.is_sensitive());
    assert_eq!(startup.note(), None);
    // A failed off: its note, and no commands.
    let mut off = all_on();
    off.begin(Direction::Off).expect("a flip");
    window.boot(off.clone());
    assert!(!switch.is_active() && switch.state() && !switch.is_sensitive());
    off.end(Some(Failure::NotAuthorized(Direction::Off)), true);
    window.boot(off);
    assert_eq!(
        startup.note().as_deref(),
        Some(
            "Not changed: the password wasn't accepted, or no password dialog could open in this \
             session."
        )
    );
    assert_no_terminal(&window);
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(100));
    let page = save_png(
        &renderer.0,
        window.config_page().widget(),
        "configuration-startup",
    );
    print!("{}; ", page.display());
    drop(renderer);
    // The app setting the switch itself asks nothing.
    asked.borrow_mut().clear();
    window.boot(new_install());
    window.boot(all_on());
    run_for(Duration::from_millis(20));
    assert!(
        !asked
            .borrow()
            .iter()
            .any(|r| matches!(r, Request::StartAtBoot(_))),
        "{:?}",
        asked.borrow()
    );
    close(window);
}

// `write-failed` shows as connected for 2 s, then its banner; any other state ends the wait.
fn sending_fails(app: &gtk::Application) {
    let (window, _) = shown(app);
    let mut state = base();
    state.cooler_state = Some(CoolerState::WriteFailed);
    window.state(state.clone());
    assert_banners(&window, None, "under 2 s");
    run_for(Duration::from_millis(1700));
    assert_banners(&window, None, "still under 2 s");
    run_until("the 2 s pass", || window.banners().cooler_text().is_some());
    assert_eq!(
        window.banners().cooler_text().as_deref(),
        Some(
            "The cooler is connected, but sending it the picture fails. The service keeps trying by itself, and changes are still saved. The journal says more."
        )
    );
    assert_eq!(window.column_words()[1], "Cooler: sending fails");
    // Connected again: the banner goes; failing again waits 2 s again.
    window.state(base());
    assert_banners(&window, None, "connected again");
    window.state(state);
    run_for(Duration::from_millis(500));
    assert_banners(&window, None, "a new wait");
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

// The header and the two pages (Window): the title stays in the middle with the gear and the
// close icon at the end; the gear is pressed on Configuration and goes back when pressed again;
// Back, Escape's action and the gear leave it; the page's title is announced and the focus moves
// when the user changes the page, not when the window returns to it after the service did.
fn header_and_pages(app: &gtk::Application) {
    let ((window, asked), said) = shown_saying(app);
    window.state(base());
    run_for(Duration::from_millis(150));
    let offset = window.title_offset().expect("the title has a place");
    assert!(
        offset.abs() <= 1.0,
        "the title is {offset} px off the middle"
    );

    // The gear opens Configuration: pressed, the title announced, the focus on Back.
    let gear = window.gear().clone();
    gear.emit_clicked();
    assert_eq!(window.screen().1, "config");
    assert!(window.gear_pressed());
    assert!(window.config_page().back().is_focus(), "focus on Back");
    assert_eq!(*said.borrow(), ["Configuration"]);
    // Pressed again, it goes back, and the focus lands on the gear.
    gear.emit_clicked();
    assert_eq!(window.screen().1, "display");
    assert!(!window.gear_pressed());
    assert!(gear.is_focus(), "focus on the gear");
    assert_eq!(*said.borrow(), ["Configuration", "Display"]);
    // The back icon is a button with the Back action: it closes the page too.
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    window.config_page().back().emit_clicked();
    assert_eq!(window.screen().1, "display");
    assert!(gear.is_focus());
    said.borrow_mut().clear();

    // The service goes down while Configuration shows: the off screen, the gear insensitive and
    // not pressed; back again, the window returns to Configuration with the gear pressed, and
    // says nothing.
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    said.borrow_mut().clear();
    window.down();
    assert_eq!(window.screen().1, "off");
    assert_eq!(window.header()[0], (true, false));
    assert!(!window.gear_pressed());
    window.state(base());
    assert_eq!(window.screen().1, "config");
    assert!(window.gear_pressed());
    assert!(
        said.borrow().is_empty(),
        "no announcement: {:?}",
        said.borrow()
    );
    assert!(
        !asked.borrow().contains(&Request::Refresh),
        "changing the page asks the service nothing"
    );
    close(window);
}

// Frames are watched once mapped; Refresh and the pages through their actions, and only where
// they can be used.
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
    window.state(base());
    WidgetExt::activate_action(window.widget(), "win.refresh", None).expect("the refresh action");
    run_for(Duration::from_millis(20));
    assert_eq!(*asked.borrow(), [Request::Refresh]);
    // Configuration opens the page, Back closes it, and Back works only there.
    WidgetExt::activate_action(window.widget(), "win.back", None).ok();
    assert_eq!(window.screen().1, "display", "Back does nothing on Display");
    WidgetExt::activate_action(window.widget(), "win.configuration", None)
        .expect("the page action");
    assert_eq!(window.screen().1, "config");
    assert_eq!(window.actions_enabled(), [true, true, true]);
    WidgetExt::activate_action(window.widget(), "win.back", None).expect("the back action");
    assert_eq!(window.screen().1, "display");
    // The service gone: neither page action works, and the page asked for stays shown nowhere.
    window.down();
    WidgetExt::activate_action(window.widget(), "win.configuration", None).ok();
    assert_eq!(window.screen().1, "off");
    window.state(base());
    // The keys of each action (Keyboard, Shortcuts): AT-SPI can't type on Wayland, so the
    // end-to-end run reaches the actions and this check their keys (decisions log D71).
    for (action, keys) in [
        ("win.configuration", &["<Control>comma"][..]),
        ("win.back", &["Escape", "<Alt>Left"]),
        ("win.refresh", &["F5", "<Control>r"]),
        ("win.close", &["<Control>w", "<Control>q"]),
    ] {
        assert_eq!(app.accels_for_action(action), keys, "{action}");
    }
    // The off screen has no commands and no Copy button.
    window.down();
    window.boot(new_install());
    assert_no_terminal(&window);
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
// on its widget, with the widget's role. A label's fixed name is on a group around it (decisions
// log D72).
fn names(app: &gtk::Application) {
    use gtk::AccessibleRole as R;
    let (window, _) = shown(app);
    let mut want: Vec<(&str, R)> = vec![
        ("Configuration", R::ToggleButton),
        ("Close", R::Button),
        ("Back", R::Button),
        ("Refresh display", R::Button),
        ("Change feedback", R::Group),
        ("Not available in this session", R::Group),
        ("Cooler banner", R::Group),
        ("Cooler display preview", R::Img),
        ("Unavailable reading", R::Group),
        ("Cooler status", R::Group),
        ("Service status", R::Group),
        ("Music sound status", R::Group),
        // The Display page (Task 5).
        ("Load options", R::Grid),
        ("Temperature options", R::Grid),
        ("Power options", R::Grid),
        ("Music options", R::Grid),
        ("Load unavailable reasons", R::Group),
        ("Temperature unavailable reasons", R::Group),
        ("Power unavailable reasons", R::Group),
        ("Music unavailable reasons", R::Group),
        // The Configuration page (Task 6) and its Theme card (Phase 11a Task 11).
        ("Theme settings", R::Group),
        ("Use system theme", R::Switch),
        ("Dark theme", R::ToggleButton),
        ("Light theme", R::ToggleButton),
        ("Theme status", R::Group),
        ("Display settings", R::Group),
        ("Music settings", R::Group),
        ("Rotation 0°", R::ToggleButton),
        ("Rotation 90°", R::ToggleButton),
        ("Rotation 180°", R::ToggleButton),
        ("Rotation 270°", R::ToggleButton),
        ("Celsius", R::ToggleButton),
        ("Fahrenheit", R::ToggleButton),
        ("GPU", R::ComboBox),
        ("GPU in use", R::Group),
        ("Peak dots", R::Switch),
        ("Bar fall", R::Slider),
        ("Peak fall", R::Slider),
        // The Start at boot switch (Phase 11, Task 3) and the off screen.
        ("Service message", R::Group),
        ("Startup settings", R::Group),
        ("Start at boot", R::Switch),
        ("Start at boot status", R::Group),
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
