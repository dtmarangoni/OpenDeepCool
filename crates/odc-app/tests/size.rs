//! The window's size in a headless weston, on outputs of three sizes (Phase 11a plan, Task 9;
//! layer 3; `docs/design/settings-app.md`, Size and scrolling): the window is its content's
//! natural size and the same on every page, give or take a wrapped line; on a smaller output it is capped at the output less
//! 48 px and the pages scroll inside it, the preview staying put; on a tiny one it scrolls
//! sideways too, and a focused control scrolls into view; it can't be resized and has no size of
//! its own; the not-available message is a small window. The pixel counts depend on the font, so
//! the checks compare the window with the output and with itself. Each output size runs in its
//! own process (`--size=WxH`). Behind the `weston-tests` feature.

mod common;

use std::process::{Command, ExitCode};
use std::time::Duration;

use gtk::prelude::*;
use odc_app::ui::window::Window;
use odc_ipc::CoolerState;

use common::{Check, base, close, run_for, shown};

/// The outputs: a large one, one a little lower than the window wants, and a tiny one.
const SIZES: [(u32, u32); 3] = [(2560, 1600), (1280, 700), (640, 480)];

/// What the cap leaves of the output (`ui::size`), and the header bar's height it leaves out.
const MARGIN: i32 = 48;
const HEADER: i32 = 48;

/// What a hint's wrapping may change the height by: about a line of text, and a margin.
const LINE: i32 = 24;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    // One argument, `--size=WxH`: the harness takes any argument without a dash for a filter.
    let size = args
        .iter()
        .find_map(|a| a.strip_prefix("--size="))
        .and_then(|text| {
            let (w, h) = text.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        });
    if let Some(size) = size {
        return match size {
            (2560, 1600) => common::run_at(&LARGE, size),
            (1280, 700) => common::run_at(&SMALL, size),
            _ => common::run_at(&TINY, size),
        };
    }
    // The parent: each output in a process of its own.
    for (w, h) in SIZES {
        let status = Command::new(std::env::current_exe().expect("this test"))
            .args(args.iter().skip(1))
            .arg(format!("--size={w}x{h}"))
            .status()
            .expect("the checks start");
        if !status.success() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

const LARGE: [(&str, Check); 4] = [
    ("the_window_is_its_contents_natural_size", natural),
    (
        "it_has_no_size_of_its_own_and_cant_be_resized",
        not_resizable,
    ),
    ("the_not_available_message_is_a_small_window", not_available),
    ("a_banner_adds_its_height", banner),
];

const SMALL: [(&str, Check); 1] = [(
    "a_lower_output_caps_the_window_and_the_page_scrolls",
    capped,
)];

const TINY: [(&str, Check); 2] = [
    ("a_tiny_output_scrolls_both_ways", tiny),
    ("a_focused_control_scrolls_into_view", focus_into_view),
];

/// Whether `scroller` scrolls down or sideways: its content is taller or wider than it.
fn scrolls(scroller: &gtk::ScrolledWindow) -> (bool, bool) {
    let (h, v) = (scroller.hadjustment(), scroller.vadjustment());
    (
        h.upper() > h.page_size() + 1.0,
        v.upper() > v.page_size() + 1.0,
    )
}

/// The size of the window's content.
fn size_of(window: &Window) -> (i32, i32) {
    (window.widget().width(), window.widget().height())
}

fn open_configuration(window: &Window) {
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(250));
}

// On a large output the window is its content's natural size: nothing scrolls anywhere, and it
// is the same on Display, on Configuration and on the off screen, so it never resizes with them.
fn natural(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(base());
    run_for(Duration::from_millis(300));
    let size = size_of(&window);
    assert!(size.0 > 0 && size.1 > 0, "{size:?}");
    assert!(
        size.0 < 2560 - MARGIN && size.1 < 1600 - MARGIN - HEADER,
        "{size:?} isn't capped on a large output"
    );
    let (all, column) = window.scrollers();
    for (name, scroller) in [
        ("the window", all.clone()),
        ("the column", column.clone()),
        ("Display", window.display_page().scroller()),
        ("Configuration", window.config_page().scroller()),
    ] {
        assert_eq!(scrolls(&scroller), (false, false), "{name} scrolls");
    }
    open_configuration(&window);
    assert_eq!(size_of(&window), size, "the size on Configuration");

    // The service stops and the Startup card goes to the off screen: the page keeps the card's
    // room, so the window doesn't shrink by the card's height. A hint's wrapping can change by a
    // line, which is why the height is compared with a tolerance of one.
    window.down();
    run_for(Duration::from_millis(250));
    let down = size_of(&window);
    assert_eq!(down.0, size.0, "the width on the off screen");
    assert!(
        (down.1 - size.1).abs() <= LINE,
        "{size:?}, then {down:?} on the off screen"
    );
    window.state(base());
    run_for(Duration::from_millis(250));
    let back = size_of(&window);
    assert_eq!(back.0, size.0, "the width back on Configuration");
    assert!(
        (back.1 - size.1).abs() <= LINE,
        "{size:?}, then {back:?} back"
    );
    close(window);
}

// The window has no default size, no size request and can't be resized (Floating).
fn not_resizable(app: &gtk::Application) {
    let (window, _) = shown(app);
    let w = window.widget();
    assert!(!w.is_resizable());
    assert_eq!(w.default_size(), (0, 0));
    assert_eq!(w.size_request(), (-1, -1));
    close(window);
}

// Not available in this session: a small window around the message, which grows when the
// service answers.
fn not_available(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.refused();
    run_for(Duration::from_millis(300));
    let small = size_of(&window);
    assert!(small.0 <= 480 && small.1 <= 220, "{small:?}");
    window.state(base());
    run_for(Duration::from_millis(300));
    let big = size_of(&window);
    assert!(big.0 > 480 && big.1 > 220, "{big:?}");
    close(window);
}

// A banner adds its own height to the window and no width, where the output has room.
fn banner(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(base());
    run_for(Duration::from_millis(300));
    let before = size_of(&window);
    let mut state = base();
    state.cooler_state = Some(CoolerState::NotFound);
    window.state(state);
    run_for(Duration::from_millis(300));
    let after = size_of(&window);
    assert!(after.1 > before.1, "{before:?} then {after:?}");
    assert!((after.0 - before.0).abs() <= 2, "{before:?} then {after:?}");
    close(window);
}

// An output lower than the window wants: the window is the output less the margin, the pages
// scroll down inside it, the preview and the page's bar stay where they are while they do, and
// nothing scrolls sideways (the window is narrow enough).
fn capped(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(base());
    open_configuration(&window);
    let want = 700 - MARGIN - HEADER;
    let (_, height) = size_of(&window);
    assert!((height - want).abs() <= 2, "{height} px high, not {want}");
    let (all, column) = window.scrollers();
    assert_eq!(scrolls(all), (false, false), "the window scroller");
    assert_eq!(scrolls(column), (false, false), "the column");
    let config = window.config_page().scroller();
    assert_eq!(
        scrolls(&config),
        (false, true),
        "Configuration scrolls down"
    );

    // Scrolled to the end, the preview and the page's bar haven't moved.
    let place = |widget: &gtk::Widget| {
        widget
            .compute_bounds(window.widget())
            .map(|b| (b.x(), b.y()))
            .expect("a place")
    };
    let preview = window.preview().widget().clone();
    let back = window.config_page().back().clone();
    let (preview_at, back_at) = (place(preview.upcast_ref()), place(back.upcast_ref()));
    let adjustment = config.vadjustment();
    adjustment.set_value(adjustment.upper());
    run_for(Duration::from_millis(200));
    assert!(adjustment.value() > 0.0, "the page scrolled");
    assert_eq!(place(preview.upcast_ref()), preview_at, "the preview moved");
    assert_eq!(place(back.upcast_ref()), back_at, "the page's bar moved");
    close(window);
}

// A tiny output: the window fits it, the pages scroll down and the window scrolls sideways.
fn tiny(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(base());
    run_for(Duration::from_millis(300));
    let (width, height) = size_of(&window);
    assert!(width <= 640 - MARGIN, "{width} px wide");
    assert!(height <= 480 - MARGIN - HEADER + 2, "{height} px high");
    let (all, _) = window.scrollers();
    assert!(scrolls(all).0, "the window scrolls sideways");
    assert!(
        scrolls(&window.display_page().scroller()).1,
        "Display scrolls down"
    );
    open_configuration(&window);
    assert!(
        scrolls(&window.config_page().scroller()).1,
        "Configuration scrolls down"
    );
    for (name, scroller) in [
        ("Display", window.display_page().scroller()),
        ("Configuration", window.config_page().scroller()),
    ] {
        assert!(!scrolls(&scroller).0, "{name} scrolls sideways itself");
    }
    close(window);
}

// Tabbing to the last control of the page brings it into view: the page scrolls to it. (The
// Start at boot switch is insensitive until systemd has been read, so the last control that takes
// the focus here is the Peak fall scale.)
fn focus_into_view(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(base());
    open_configuration(&window);
    let last: gtk::Widget = window
        .names()
        .find("Peak fall")
        .expect("the Peak fall scale");
    // Tab from the back icon until the scale has the focus, as a person would.
    for _ in 0..40 {
        if last.is_focus() {
            break;
        }
        assert!(
            window.widget().child_focus(gtk::DirectionType::TabForward),
            "Tab found no next control"
        );
        run_for(Duration::from_millis(30));
    }
    assert!(last.is_focus(), "Tab reached the scale");
    run_for(Duration::from_millis(300));
    let page = window.config_page().scroller();
    let bounds = last.compute_bounds(&page).expect("a place");
    assert!(
        bounds.y() >= 0.0 && bounds.y() + bounds.height() <= page.height() as f32 + 1.0,
        "the scale is at {} to {} of a {} px page",
        bounds.y(),
        bounds.y() + bounds.height(),
        page.height()
    );
    close(window);
}
