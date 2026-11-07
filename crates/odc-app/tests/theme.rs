//! The theme in the window, in a headless weston (Phase 10 plan, Task 7; layer 3): a sample
//! Omarchy theme's colours where the design puts them; a theme switch done as Omarchy does it,
//! taken within 1 s with the page and scroll position kept and the reload logged; no Omarchy
//! folder, the default dark theme, then a light theme with a key missing appearing later; and
//! the window opaque throughout. Made-up colours only. On a 900 × 600 output, so the Display
//! page scrolls. Through `common::run_at`; behind the `weston-tests` feature.

mod common;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use odc_app::theme::{Rgb, Tokens};
use odc_app::ui::theme::{LiveTheme, theme_dir};
use odc_app::ui::window::Window;
use odc_app::ui::{Request, preview};
use odc_protocol::Frame;

use common::{Check, Log, Renderer, base, close, dots_of, near, run_for, run_until};

fn main() -> ExitCode {
    common::run_at(&CHECKS, (900, 600))
}

const CHECKS: [(&str, Check); 3] = [
    (
        "a_sample_omarchy_theme_s_colours_where_the_design_puts_them",
        sample,
    ),
    (
        "an_omarchy_switch_is_taken_within_1_s_keeping_page_and_scroll",
        switch,
    ),
    (
        "no_omarchy_folder_then_a_light_theme_missing_a_key",
        no_folder,
    ),
];

/// A made-up dark theme: the mockups' sample colours.
const DARK: &str = r##"
mode = "dark"
accent = "#d9a35b"
selection = "#4a4235"
muted = "#9c9585"
background = "#1f2120"
lighter_background = "#2a2c2a"
foreground = "#e4dccb"
red = "#e07a6e"
yellow = "#d9b45b"
green = "#98bf7c"
"##;

/// A made-up light theme.
const LIGHT: &str = r##"
mode = "light"
accent = "#8a4b16"
selection = "#e6d6bf"
muted = "#6b6255"
background = "#f4efe6"
lighter_background = "#fffaf2"
foreground = "#2b2620"
red = "#a3302a"
yellow = "#7a5a00"
green = "#2f6b2a"
"##;

const SHELL: &str = r##"
[controls]
normal-color        = "#808080"
normal-fill-alpha   = 0.04
normal-border       = "#808080"
normal-border-width = 1
normal-border-alpha = 0.4
hover-cursor-color        = "#808080"
hover-cursor-fill-alpha   = 0.08
hover-cursor-border       = "#808080"
hover-cursor-border-alpha = 0.25
focus-color        = "#808080"
focus-fill-alpha   = 0.08
focus-border       = "#808080"
focus-border-alpha = 0.25
selected-color        = "#808080"
selected-fill-alpha   = 0.18
selected-border       = "#808080"
selected-border-alpha = 1.0
pressed-fill-alpha   = 0.22
"##;

fn rgb(hex: &str) -> Rgb {
    let byte = |i: usize| u8::from_str_radix(&hex[1 + 2 * i..3 + 2 * i], 16).expect("hex");
    Rgb([byte(0), byte(1), byte(2)])
}

/// A clean state folder for a check.
fn state(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("theme")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a folder");
    dir.join("omarchy")
}

/// Writes a theme folder at `dir`.
fn write_theme(dir: &Path, colors: &str) {
    fs::create_dir_all(dir).expect("the theme folder");
    fs::write(dir.join("colors.toml"), colors).expect("colors.toml");
    fs::write(dir.join("shell.toml"), SHELL).expect("shell.toml");
}

/// A window following the theme in `state`, and what the theme logged.
fn themed(app: &gtk::Application, state: &Path) -> (Window, LiveTheme, Log<String>) {
    let logged: Log<String> = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&logged);
    let log = move |line: &str| sink.borrow_mut().push(line.to_owned());
    let theme = LiveTheme::first(Some(state), &log);
    let window = Window::new(
        app,
        |_: Request| {},
        PathBuf::from("/nonexistent/pci.ids"),
        &theme.tokens,
    );
    window.announce_with(|_| {});
    let live = LiveTheme::follow(&window, Some(state.to_path_buf()), theme.tokens, log)
        .expect("a state folder");
    window.widget().present();
    run_until("the window maps", || {
        window.preview().widget().is_mapped() && window.preview().widget().width() > 0
    });
    let mut state = base();
    state.mode = Some(9);
    window.state(state);
    let mut frame = Frame::blank();
    frame.set(1, 1, true);
    window.frame(frame);
    run_for(Duration::from_millis(200));
    (window, live, logged)
}

/// The colour at `(x, y)` of `widget` as GTK last drew it.
fn pixel(renderer: &Renderer, widget: &impl IsA<gtk::Widget>, x: i32, y: i32) -> [u8; 3] {
    let widget = widget.as_ref();
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(
        &snapshot,
        f64::from(widget.width()),
        f64::from(widget.height()),
    );
    let node = snapshot.to_node().expect("the widget draws");
    let texture = renderer.0.render_texture(&node, None);
    let mut downloader = gtk::gdk::TextureDownloader::new(&texture);
    downloader.set_format(gtk::gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let at = y as usize * stride + x as usize * 4;
    [bytes[at], bytes[at + 1], bytes[at + 2]]
}

/// The window's background, the selected tile's border and the lit preview dot are `tokens`'.
fn check_colours(window: &Window, tokens: &Tokens, what: &str) {
    let renderer = Renderer::new();
    let w = window.widget();
    let corner = pixel(&renderer, w, 3, w.height() - 3);
    assert!(
        near(corner, tokens.bg),
        "{what}: the background is {corner:?}"
    );
    let tile = &window.display_page().tiles()[8];
    let border = pixel(&renderer, &tile.child, 0, tile.child.height() / 2);
    // A rounded border is an antialiased path: a few levels off on its straight edge too.
    let off = border
        .iter()
        .zip(tokens.accent.0)
        .map(|(g, w)| (i16::from(*g) - i16::from(w)).abs())
        .max()
        .unwrap_or(0);
    assert!(off <= 6, "{what}: the selected tile's border is {border:?}");
    let [r, g, b, _] = dots_of(&renderer.0, window.preview().widget(), preview::PITCH)[0];
    assert!(
        near([r, g, b], tokens.fg),
        "{what}: the lit dot is {r},{g},{b}"
    );
    assert_eq!(w.opacity(), 1.0, "{what}: the window is opaque");
}

// The sample theme's background, card surface, accent and foreground where the design puts
// them, and GTK's dark scheme.
fn sample(app: &gtk::Application) {
    let state = state("sample");
    write_theme(&theme_dir(&state), DARK);
    let (window, live, logged) = themed(app, &state);
    assert_eq!(*logged.borrow(), ["theme: the Omarchy theme"]);
    let tokens = live.tokens();
    assert_eq!(
        (tokens.bg, tokens.surface, tokens.fg, tokens.accent),
        (
            rgb("#1f2120"),
            rgb("#2a2c2a"),
            rgb("#e4dccb"),
            rgb("#d9a35b")
        )
    );
    check_colours(&window, &tokens, "the sample theme");
    assert!(window.prefers_dark());
    WidgetExt::activate_action(window.widget(), "win.page-config", None).expect("the page");
    run_for(Duration::from_millis(200));
    let card = window.names().find("Display settings").expect("a card");
    let renderer = Renderer::new();
    let surface = pixel(&renderer, &card, 10, 10);
    assert!(near(surface, tokens.surface), "the card is {surface:?}");
    drop(renderer);
    close(window);
}

// Omarchy's switch: the new theme staged in `current/next-theme`, the old folder removed and the
// staged one moved in. The window takes it within 1 s, on the same page at the same scroll.
fn switch(app: &gtk::Application) {
    let state = state("switch");
    write_theme(&theme_dir(&state), DARK);
    let (window, live, logged) = themed(app, &state);
    let scroller = window
        .display_page()
        .widget()
        .ancestor(gtk::ScrolledWindow::static_type())
        .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok())
        .expect("the page scrolls");
    let adjustment = scroller.vadjustment();
    assert!(
        adjustment.upper() - adjustment.page_size() > 60.0,
        "the page is taller than the window"
    );
    adjustment.set_value(60.0);
    run_for(Duration::from_millis(100));
    let next = state.join("current").join("next-theme");
    write_theme(&next, LIGHT);
    fs::remove_dir_all(theme_dir(&state)).expect("the old theme removed");
    fs::rename(&next, theme_dir(&state)).expect("the new one moved in");
    let started = std::time::Instant::now();
    run_until("the reload", || {
        logged
            .borrow()
            .iter()
            .any(|l| l == "theme: reloaded, the Omarchy theme")
    });
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "taken in {:?}",
        started.elapsed()
    );
    run_for(Duration::from_millis(200));
    let tokens = live.tokens();
    assert_eq!((tokens.bg, tokens.accent), (rgb("#f4efe6"), rgb("#8a4b16")));
    check_colours(&window, &tokens, "after the switch");
    assert!(!window.prefers_dark(), "GTK's light scheme");
    assert_eq!(window.screen().1, "display", "the same page");
    assert_eq!(adjustment.value(), 60.0, "the same scroll position");
    close(window);
}

// No Omarchy folder: the default dark theme. A light theme missing its accent appearing later is
// found, the light default's accent fills it, and both are logged.
fn no_folder(app: &gtk::Application) {
    let state = state("none");
    let (window, live, logged) = themed(app, &state);
    assert_eq!(*logged.borrow(), ["theme: the default theme"]);
    assert_eq!(live.tokens(), Tokens::default_theme(true));
    check_colours(&window, &Tokens::default_theme(true), "the default theme");
    let without_accent: String = LIGHT
        .lines()
        .filter(|l| !l.starts_with("accent"))
        .map(|l| format!("{l}\n"))
        .collect();
    write_theme(&theme_dir(&state), &without_accent);
    run_until("the theme appearing", || {
        logged
            .borrow()
            .iter()
            .any(|l| l == "theme: reloaded, the Omarchy theme")
    });
    assert!(
        logged
            .borrow()
            .iter()
            .any(|l| l
                == "theme: colors.toml: `accent` is missing; the default theme's value is used")
    );
    let tokens = live.tokens();
    assert_eq!(tokens.accent, Tokens::default_theme(false).accent);
    assert_eq!(tokens.bg, rgb("#f4efe6"));
    run_for(Duration::from_millis(200));
    check_colours(&window, &tokens, "the light theme");
    assert!(!window.prefers_dark());
    close(window);
}
