//! The window in a headless weston (Phase 10 plan, Task 4; layer 3): the preview's pixels for
//! the expected frames of modes 1, 10 and 13 in every rotation, no redraw for an unchanged frame,
//! the banners and states in the design's order and words, the feedback label, the requests,
//! and the accessible names. Against a fake client: the test gives the window the service's
//! state and records what it asks.
//!
//! GTK runs on the main thread, so this test has a harness of its own. It starts weston, then
//! runs itself again inside it, with an environment that reaches nothing of the user's session:
//! weston's socket, the Cairo renderer, GTK's test accessibility backend, no session bus, and
//! settings and configuration of its own. Behind the `weston-tests` feature.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, gio, glib, gsk};
use odc_app::model::{SERVICE_BANNER, View};
use odc_app::theme::{Rgb, Tokens};
use odc_app::ui::window::Window;
use odc_app::ui::{Reason, Refused, Request, dots, preview};
use odc_ipc::{CoolerState, FallStop, FanInput, FanSpeed, Input, State, TemperatureUnit};
use odc_protocol::Frame;
use odc_testkit::expected::load_mode;
use odc_testkit::weston::Weston;

const IN_WESTON: &str = "--in-weston";

fn main() -> ExitCode {
    if std::env::args().any(|a| a == IN_WESTON) {
        return checks();
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("window");
    let weston = Weston::headless(&dir.join("weston")).expect("weston starts");
    let own = |name: &str| {
        let path = dir.join(name);
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&path)
            .expect("a folder of the test's own");
        path
    };
    let status = Command::new(std::env::current_exe().expect("this test"))
        .arg(IN_WESTON)
        .args(std::env::args().skip(1))
        .env("WAYLAND_DISPLAY", weston.display())
        .env("GDK_BACKEND", "wayland")
        .env("GSK_RENDERER", "cairo")
        .env("GTK_A11Y", "test")
        .env("GSETTINGS_BACKEND", "memory")
        .env("XDG_RUNTIME_DIR", own("runtime"))
        .env("XDG_CONFIG_HOME", own("config"))
        .env("XDG_CACHE_HOME", own("cache"))
        .env("XDG_STATE_HOME", own("state"))
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("GTK_THEME")
        .env_remove("GTK_MODULES")
        .env_remove("GDK_SCALE")
        .env_remove("GDK_DPI_SCALE")
        .status()
        .expect("the checks start");
    if status.success() {
        ExitCode::SUCCESS
    } else {
        eprintln!("the checks ended: {status}");
        ExitCode::FAILURE
    }
}

type Check = fn(&gtk::Application);

const CHECKS: [(&str, Check); 7] = [
    ("the_preview_shows_every_expected_frame", preview_pixels),
    ("an_unchanged_frame_draws_nothing", no_redraw),
    ("banners_and_states_follow_the_design", banners_and_states),
    ("sending_fails_shows_after_2_s", sending_fails),
    ("the_feedback_label", feedback),
    ("the_window_s_requests", requests),
    ("every_name_of_the_design_s_table", names),
];

fn checks() -> ExitCode {
    gtk::init().expect("GTK starts in weston");
    let app = gtk::Application::builder()
        .application_id("io.github.dtmarangoni.OpenDeepCool.Tests")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>)
        .expect("the test application registers");
    let filter: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    let mut ran = 0;
    for (name, check) in CHECKS {
        if !filter.is_empty() && !filter.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        let start = Instant::now();
        print!("test {name} ... ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        check(&app);
        println!("ok ({:.1} s)", start.elapsed().as_secs_f64());
        ran += 1;
    }
    println!("\ntest result: ok. {ran} passed; 0 failed");
    ExitCode::SUCCESS
}

/// Runs GTK's main loop for `duration`.
fn run_for(duration: Duration) {
    let context = glib::MainContext::default();
    let until = Instant::now() + duration;
    while Instant::now() < until {
        while context.pending() {
            context.iteration(false);
        }
        thread::sleep(Duration::from_millis(2));
    }
}

/// Runs GTK's main loop until `done`, at most 5 s.
fn run_until(what: &str, done: impl Fn() -> bool) {
    let context = glib::MainContext::default();
    let until = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < until, "{what}: not within 5 s");
        while context.pending() {
            context.iteration(false);
        }
        thread::sleep(Duration::from_millis(2));
    }
}

type Log<T> = Rc<RefCell<Vec<T>>>;

/// A window shown in weston, and what it asked.
fn shown(app: &gtk::Application) -> (Window, Log<Request>) {
    shown_saying(app).0
}

/// The same, and what it announced: GTK's test accessibility backend has no announcements
/// (decisions log D49), so they're recorded here.
fn shown_saying(app: &gtk::Application) -> ((Window, Log<Request>), Log<String>) {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::clone(&asked);
    let window = Window::new(
        app,
        move |request| log.borrow_mut().push(request),
        PathBuf::from("/nonexistent/pci.ids"),
        &Tokens::default_theme(true),
    );
    let said = Rc::new(RefCell::new(Vec::new()));
    let say = Rc::clone(&said);
    window.announce_with(move |text| say.borrow_mut().push(text.to_owned()));
    window.widget().present();
    run_until("the window maps", || {
        window.preview().widget().is_mapped() && window.preview().widget().width() > 0
    });
    ((window, asked), said)
}

fn close(window: Window) {
    window.widget().destroy();
    run_for(Duration::from_millis(20));
}

/// A made-up state: Load · CPU, everything there.
fn base() -> State {
    let input = |n: u32| Input {
        chip: "acme6000".to_owned(),
        device: "devices/platform/acme6000.2592".to_owned(),
        input: n,
    };
    State {
        mode: Some(1),
        rotation: Some(0),
        temperature_unit: Some(TemperatureUnit::Celsius),
        cooler_connected: Some(true),
        cooler_state: Some(CoolerState::Connected),
        available_modes: Some((1..=23).collect()),
        unavailable_reasons: Some(HashMap::new()),
        gpus: Some(Vec::new()),
        chosen_gpu: Some(String::new()),
        gpu_in_use: Some(String::new()),
        fan_inputs: Some(vec![
            FanInput {
                chip: "acme6000".to_owned(),
                device: "devices/platform/acme6000.2592".to_owned(),
                input: 1,
                label: String::new(),
                rpm: FanSpeed::Rpm(1180),
            },
            FanInput {
                chip: "acme6000".to_owned(),
                device: "devices/platform/acme6000.2592".to_owned(),
                input: 2,
                label: String::new(),
                rpm: FanSpeed::Rpm(3400),
            },
        ]),
        cooler_fan_input: Some(Some(input(1))),
        pump_input: Some(Some(input(2))),
        cooler_fan_input_present: Some(true),
        pump_input_present: Some(true),
        cooler_fan_rpm: Some(FanSpeed::Rpm(1180)),
        pump_rpm: Some(FanSpeed::Rpm(3400)),
        music_peaks: Some(true),
        music_bar_fall: Some(FallStop::Moderate),
        music_peak_fall: Some(FallStop::Slow),
        audio_connected: Some(true),
        audio_wanted: Some(false),
    }
}

/// A realized Cairo renderer, unrealized when dropped, as GSK asks.
struct Renderer(gsk::CairoRenderer);

impl Renderer {
    fn new() -> Renderer {
        let renderer = gsk::CairoRenderer::new();
        renderer
            .realize_for_display(&gdk::Display::default().expect("weston's display"))
            .expect("the Cairo renderer");
        Renderer(renderer)
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.0.unrealize();
    }
}

/// The colour at the centre of each dot of the preview, as GTK last drew it: a widget's
/// paintable serves the render node of its last draw.
fn dot_colours(renderer: &gsk::CairoRenderer, area: &gtk::DrawingArea) -> Vec<[u8; 3]> {
    let (width, height) = (f64::from(area.width()), f64::from(area.height()));
    let paintable = gtk::WidgetPaintable::new(Some(area));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width, height);
    let node = snapshot.to_node().expect("the preview draws something");
    let texture = renderer.render_texture(&node, None);
    let mut downloader = gdk::TextureDownloader::new(&texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let mut colours = Vec::with_capacity(Frame::SIZE * Frame::SIZE);
    for row in 0..Frame::SIZE {
        for col in 0..Frame::SIZE {
            let (x, y) = dots::centre(row, col, preview::PITCH);
            let at = y as usize * stride + x as usize * 4;
            colours.push([bytes[at], bytes[at + 1], bytes[at + 2]]);
        }
    }
    colours
}

fn near(got: [u8; 3], want: Rgb) -> bool {
    got.iter()
        .zip(want.0)
        .all(|(g, w)| (i16::from(*g) - i16::from(w)).abs() <= 2)
}

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
                let colours = dot_colours(&renderer.0, &area);
                for row in 0..Frame::SIZE {
                    for col in 0..Frame::SIZE {
                        let colour = colours[row * Frame::SIZE + col];
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

// Every name of the design's table this task builds, typed here apart from the window's: each
// set on its widget, with the widget's role.
fn names(app: &gtk::Application) {
    use gtk::AccessibleRole as R;
    let (window, _) = shown(app);
    let want: [(&str, R); 17] = [
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
    ];
    let labels: BTreeSet<String> = window.names().labels().into_iter().collect();
    let typed: BTreeSet<String> = want.iter().map(|(n, _)| (*n).to_owned()).collect();
    assert_eq!(labels, typed);
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
