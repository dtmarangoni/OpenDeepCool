//! The layer 3 tests' harness (Phase 10 plan, Tasks 4–7). GTK runs on the main thread, so each
//! test has a harness of its own: it starts weston, then runs itself again inside it, with an
//! environment that reaches nothing of the user's session (decisions log D52): weston's socket,
//! the Cairo renderer, GTK's test accessibility backend, no session bus, and settings and
//! configuration of its own. And the helpers the checks share: the main loop, a window shown
//! against a fake client, a made-up state, and the dots a drawing shows.

#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gdk, gio, glib, gsk};
use odc_app::theme::{Rgb, Tokens};
use odc_app::ui::Request;
use odc_app::ui::dots;
use odc_app::ui::window::Window;
use odc_ipc::{CoolerState, FallStop, FanInput, FanSpeed, Input, State, TemperatureUnit};
use odc_protocol::Frame;
use odc_testkit::weston::Weston;

pub const IN_WESTON: &str = "--in-weston";

/// Runs `checks` in a headless weston: the test's `main`.
pub fn run(checks: &[(&str, Check)]) -> ExitCode {
    run_at(checks, odc_testkit::weston::SIZE)
}

/// The same, on an output of `size` px.
pub fn run_at(checks: &[(&str, Check)], size: (u32, u32)) -> ExitCode {
    if std::env::args().any(|a| a == IN_WESTON) {
        return in_weston(checks);
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(env!("CARGO_CRATE_NAME"));
    let weston = Weston::headless_at(&dir.join("weston"), size).expect("weston starts");
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

pub type Check = fn(&gtk::Application);

fn in_weston(checks: &[(&str, Check)]) -> ExitCode {
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
    for &(name, check) in checks {
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
pub fn run_for(duration: Duration) {
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
pub fn run_until(what: &str, done: impl Fn() -> bool) {
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

pub type Log<T> = Rc<RefCell<Vec<T>>>;

/// A window shown in weston, and what it asked.
pub fn shown(app: &gtk::Application) -> (Window, Log<Request>) {
    shown_saying(app).0
}

/// The same, and what it announced: GTK's test accessibility backend has no announcements
/// (decisions log D49), so they're recorded here.
pub fn shown_saying(app: &gtk::Application) -> ((Window, Log<Request>), Log<String>) {
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

pub fn close(window: Window) {
    window.widget().destroy();
    run_for(Duration::from_millis(20));
}

/// A made-up state: Load · CPU, everything there.
pub fn base() -> State {
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
pub struct Renderer(pub gsk::CairoRenderer);

impl Renderer {
    pub fn new() -> Renderer {
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

/// The colour, with its alpha, at the centre of each dot of `area`, `pitch` px per dot, as GTK
/// last drew it: a widget's paintable serves the render node of its last draw.
pub fn dots_of(renderer: &gsk::CairoRenderer, area: &gtk::DrawingArea, pitch: f64) -> Vec<[u8; 4]> {
    let (width, height) = (f64::from(area.width()), f64::from(area.height()));
    let paintable = gtk::WidgetPaintable::new(Some(area));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width, height);
    let node = snapshot.to_node().expect("the drawing draws something");
    let texture = renderer.render_texture(&node, None);
    let mut downloader = gdk::TextureDownloader::new(&texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let mut colours = Vec::with_capacity(Frame::SIZE * Frame::SIZE);
    for row in 0..Frame::SIZE {
        for col in 0..Frame::SIZE {
            let (x, y) = dots::centre(row, col, pitch);
            let at = y as usize * stride + x as usize * 4;
            colours.push([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        }
    }
    colours
}

pub fn near(got: [u8; 3], want: Rgb) -> bool {
    got.iter()
        .zip(want.0)
        .all(|(g, w)| (i16::from(*g) - i16::from(w)).abs() <= 2)
}

/// Saves `widget` as GTK last drew it to `<target>/tmp/<test>/<name>.png`, for a person to look
/// at; returns the path.
pub fn save_png(
    renderer: &gsk::CairoRenderer,
    widget: &impl IsA<gtk::Widget>,
    name: &str,
) -> PathBuf {
    let widget = widget.as_ref();
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(
        &snapshot,
        f64::from(widget.width()),
        f64::from(widget.height()),
    );
    let node = snapshot.to_node().expect("the widget draws something");
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(env!("CARGO_CRATE_NAME"))
        .join(format!("{name}.png"));
    renderer
        .render_texture(&node, None)
        .save_to_png(&path)
        .expect("the picture is saved");
    path
}
