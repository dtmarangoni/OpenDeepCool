//! The Display page in a headless weston (Phase 10 plan, Task 5; layer 3): each tile's dots
//! are `odc_render`'s picture of its mode with the sample values, in each unit and peak setting;
//! a unit change redraws only the temperature tiles and a peak change only the spectrum tiles;
//! focus never asks for a mode, and each tile's activation asks for its own; the saved mode's
//! tile alone is marked; unavailable tiles and the lost reading; and the unit toggles. Against a
//! fake client, through `common::run`. Behind the `weston-tests` feature.

mod common;

use std::collections::HashMap;
use std::process::ExitCode;
use std::time::Duration;

use gtk::prelude::*;
use odc_app::theme::Tokens;
use odc_app::ui::Request;
use odc_app::ui::tiles::{self, TileWidget};
use odc_app::ui::window::Window;
use odc_ipc::{State, TemperatureUnit};
use odc_protocol::Frame;
use odc_render::{Mode, Unit, spectrum_height};

use common::{Check, Renderer, base, close, dots_of, near, run_for, run_until, save_png, shown};

fn main() -> ExitCode {
    common::run(&CHECKS)
}

const CHECKS: [(&str, Check); 7] = [
    ("each_tile_shows_odc_render_s_picture", pictures),
    ("a_change_redraws_only_the_tiles_it_changes", redraws),
    (
        "focus_asks_for_nothing_and_each_tile_for_its_mode",
        activation,
    ),
    ("only_the_saved_mode_s_tile_is_marked", selection),
    ("unavailable_tiles_and_the_lost_reading", unavailable),
    ("the_unit_changes_once_the_service_says_so", unit_toggles),
    ("the_page_saved_as_a_picture", picture_file),
];

/// The design's tile table, typed apart from the app's: each tile's name and mode.
const TILES: [(&str, u16); 23] = [
    ("Load: CPU", 1),
    ("Load: GPU", 2),
    ("Load: RAM", 3),
    ("Load: CPU + GPU", 4),
    ("Load: CPU + RAM", 5),
    ("Load: GPU + RAM", 6),
    ("Temperature: CPU", 7),
    ("Temperature: GPU", 8),
    ("Temperature: CPU + GPU", 9),
    ("Power: CPU", 10),
    ("Power: GPU", 11),
    ("Power: CPU + GPU", 12),
    ("Music: Spectrum", 13),
    ("Music: Emoji 1", 14),
    ("Music: Emoji 2", 15),
    ("Music: Emoji 3", 16),
    ("Music: Emoji 4", 17),
    ("Music: CPU temperature", 18),
    ("Music: GPU temperature", 19),
    ("Music: CPU usage", 20),
    ("Music: GPU usage", 21),
    ("Music: RAM usage", 22),
    ("Music: Dancer", 23),
];

fn tiles_of(window: &Window) -> &[TileWidget] {
    window.display_page().tiles()
}

fn draws(window: &Window) -> Vec<u32> {
    tiles_of(window).iter().map(TileWidget::draws).collect()
}

/// The modes whose pictures drew since `before`.
fn redrawn(window: &Window, before: &[u32]) -> Vec<u16> {
    tiles_of(window)
        .iter()
        .zip(before)
        .filter(|(tile, before)| tile.draws() > **before)
        .map(|(tile, _)| tile.mode.number())
        .collect()
}

fn with(change: impl FnOnce(&mut State)) -> State {
    let mut state = base();
    change(&mut state);
    state
}

/// Checks each tile's dots against `odc_render`'s picture: lit dots opaque in `lit`, unlit ones
/// faint.
fn check_dots(window: &Window, renderer: &Renderer, unit: Unit, peaks: bool) {
    let fg = Tokens::default_theme(true).fg;
    for tile in tiles_of(window) {
        let want = tiles::picture(tile.mode, unit, peaks);
        assert_eq!(tile.frame(), want, "mode {}", tile.mode.number());
        let colours = dots_of(&renderer.0, tile.picture_widget(), tiles::PITCH);
        for row in 0..Frame::SIZE {
            for col in 0..Frame::SIZE {
                let [r, g, b, a] = colours[row * Frame::SIZE + col];
                let what = format!(
                    "mode {} ({unit:?}, peaks {peaks}): dot {row},{col}",
                    tile.mode.number()
                );
                if want.get(row + 1, col + 1) {
                    assert!(
                        a > 200 && near([r, g, b], fg),
                        "{what} isn't lit: {r},{g},{b},{a}"
                    );
                } else {
                    assert!(a < 80, "{what} isn't unlit: {r},{g},{b},{a}");
                }
            }
        }
    }
}

// Each tile, in both units and with the peak dots on and off: its picture is `odc_render`'s for
// its mode and the sample values, and its dots are drawn as the picture says.
fn pictures(app: &gtk::Application) {
    let (window, _) = shown(app);
    let renderer = Renderer::new();
    for (unit, peaks, ipc_unit) in [
        (Unit::Celsius, true, TemperatureUnit::Celsius),
        (Unit::Fahrenheit, true, TemperatureUnit::Fahrenheit),
        (Unit::Fahrenheit, false, TemperatureUnit::Fahrenheit),
        (Unit::Celsius, false, TemperatureUnit::Celsius),
    ] {
        window.state(with(|s| {
            s.temperature_unit = Some(ipc_unit);
            s.music_peaks = Some(peaks);
        }));
        // Every tile has drawn, and those whose picture changed drew again.
        run_until("the tiles draw", || {
            tiles_of(&window).iter().all(|t| t.draws() > 0)
        });
        run_for(Duration::from_millis(150));
        check_dots(&window, &renderer, unit, peaks);
    }
    drop(renderer);
    close(window);
}

// A unit change redraws only the tiles showing a temperature, a peak change only those with a
// spectrum, and nothing else (a new mode, a cooler state) redraws a picture.
fn redraws(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(base());
    run_until("the tiles draw", || draws(&window).iter().all(|d| *d > 0));
    run_for(Duration::from_millis(100));
    let before = draws(&window);
    window.state(with(|s| {
        s.temperature_unit = Some(TemperatureUnit::Fahrenheit)
    }));
    run_for(Duration::from_millis(200));
    assert_eq!(redrawn(&window, &before), [7, 8, 9, 18, 19]);
    let before = draws(&window);
    window.state(with(|s| {
        s.temperature_unit = Some(TemperatureUnit::Fahrenheit);
        s.music_peaks = Some(false);
    }));
    run_for(Duration::from_millis(200));
    let spectra: Vec<u16> = Mode::all()
        .filter(|m| spectrum_height(*m).is_some())
        .map(Mode::number)
        .collect();
    assert_eq!(redrawn(&window, &before), spectra);
    let before = draws(&window);
    window.state(with(|s| {
        s.temperature_unit = Some(TemperatureUnit::Fahrenheit);
        s.music_peaks = Some(false);
        s.mode = Some(15);
        s.cooler_state = Some(odc_ipc::CoolerState::NotFound);
    }));
    run_for(Duration::from_millis(200));
    assert_eq!(redrawn(&window, &before), Vec::<u16>::new());
    close(window);
}

// Focus on each tile, and arrow keys within each group, ask for nothing; each tile's activation
// asks for its own mode, once.
fn activation(app: &gtk::Application) {
    let (window, asked) = shown(app);
    window.state(base());
    asked.borrow_mut().clear();
    for (name, _) in TILES {
        let child = window.names().find(name).expect(name);
        child.grab_focus();
        run_for(Duration::from_millis(5));
    }
    for group in [
        "Load options",
        "Temperature options",
        "Power options",
        "Music options",
    ] {
        let flowbox = window
            .names()
            .find(group)
            .and_then(|w| w.downcast::<gtk::FlowBox>().ok())
            .expect(group);
        for step in [1, 1, -1] {
            flowbox.emit_move_cursor(gtk::MovementStep::VisualPositions, step, false, false);
        }
    }
    run_for(Duration::from_millis(20));
    assert_eq!(*asked.borrow(), [], "focus asked for a mode");
    for (name, mode) in TILES {
        let child = window
            .names()
            .find(name)
            .and_then(|w| w.downcast::<gtk::FlowBoxChild>().ok())
            .expect(name);
        child.emit_activate();
        run_for(Duration::from_millis(5));
        assert_eq!(
            asked.borrow_mut().drain(..).collect::<Vec<_>>(),
            [Request::SetMode(u32::from(mode))],
            "{name}"
        );
    }
    close(window);
}

fn marked(window: &Window) -> Vec<u16> {
    tiles_of(window)
        .iter()
        .filter(|t| t.child.has_css_class("selected"))
        .map(|t| t.mode.number())
        .collect()
}

// The saved mode's tile alone is marked, across the four groups; activating another asks for it
// and marks nothing until the service's state says so.
fn selection(app: &gtk::Application) {
    let (window, asked) = shown(app);
    window.state(with(|s| s.mode = Some(9)));
    assert_eq!(marked(&window), [9]);
    let child = window
        .names()
        .find("Load: RAM")
        .and_then(|w| w.downcast::<gtk::FlowBoxChild>().ok())
        .expect("a tile");
    child.emit_activate();
    run_for(Duration::from_millis(20));
    assert!(asked.borrow().contains(&Request::SetMode(3)));
    assert_eq!(marked(&window), [9], "not before the service says so");
    window.state(with(|s| s.mode = Some(3)));
    assert_eq!(marked(&window), [3]);
    close(window);
}

// The GPU's metrics unavailable while mode 9, which shows the GPU temperature, is saved: its tile
// stays sensitive, flagged "Reading lost"; every other GPU tile is insensitive and "Unavailable",
// drawn muted; each group lists its reasons once.
fn unavailable(app: &gtk::Application) {
    let (window, _) = shown(app);
    let reasons: HashMap<u32, String> = [
        (2, "Gpu usage"),
        (4, "Gpu usage"),
        (6, "Gpu usage"),
        (8, "Gpu temperature"),
        (9, "Gpu temperature"),
        (11, "Gpu power"),
        (12, "Gpu power"),
        (19, "Gpu temperature"),
        (21, "Gpu usage"),
    ]
    .into_iter()
    .map(|(m, metric)| (m, format!("{metric}: no device found")))
    .collect();
    window.state(with(|s| {
        s.mode = Some(9);
        s.unavailable_reasons = Some(reasons.clone());
        s.available_modes = Some((1..=23).filter(|m| !reasons.contains_key(m)).collect());
    }));
    for tile in tiles_of(&window) {
        let mode = u32::from(tile.mode.number());
        let (sensitive, flag) = match mode {
            9 => (true, Some("Reading lost")),
            m if reasons.contains_key(&m) => (false, Some("Unavailable")),
            _ => (true, None),
        };
        assert_eq!(tile.child.is_sensitive(), sensitive, "mode {mode}");
        assert_eq!(tile.flag().as_deref(), flag, "mode {mode}");
    }
    let name = |n: &str| window.names().find(n).expect(n);
    let text = |n: &str| {
        name(n)
            .downcast::<gtk::Label>()
            .ok()
            .filter(|l| l.is_visible())
            .map(|l| l.text().to_string())
    };
    assert_eq!(
        text("Load unavailable reasons").as_deref(),
        Some("GPU usage: no device found")
    );
    assert_eq!(
        text("Temperature unavailable reasons").as_deref(),
        Some("GPU temperature: no device found")
    );
    assert_eq!(
        text("Power unavailable reasons").as_deref(),
        Some("GPU power: no device found")
    );
    assert_eq!(
        text("Music unavailable reasons").as_deref(),
        Some("GPU temperature: no device found\nGPU usage: no device found")
    );
    assert_eq!(
        window
            .names()
            .description("Temperature: CPU + GPU")
            .as_deref(),
        Some("Reading lost: GPU temperature: no device found")
    );
    assert_eq!(
        window.names().description("Load: GPU").as_deref(),
        Some("GPU usage: no device found")
    );
    assert_eq!(window.names().description("Load: CPU"), None);
    // An unavailable tile draws its lit dots muted.
    run_until("the tiles draw", || draws(&window).iter().all(|d| *d > 0));
    run_for(Duration::from_millis(150));
    let renderer = Renderer::new();
    let tile = &tiles_of(&window)[1];
    let muted = Tokens::default_theme(true).muted;
    let colours = dots_of(&renderer.0, tile.picture_widget(), tiles::PITCH);
    let lit = tile.frame();
    let mut lit_dots = 0;
    for row in 0..Frame::SIZE {
        for col in 0..Frame::SIZE {
            if lit.get(row + 1, col + 1) {
                let [r, g, b, _] = colours[row * Frame::SIZE + col];
                assert!(near([r, g, b], muted), "dot {row},{col}: {r},{g},{b}");
                lit_dots += 1;
            }
        }
    }
    assert!(lit_dots > 0);
    // Back: everything available again.
    window.state(with(|s| s.mode = Some(9)));
    assert!(
        tiles_of(&window)
            .iter()
            .all(|t| t.child.is_sensitive() && t.flag().is_none())
    );
    assert_eq!(text("Load unavailable reasons"), None);
    drop(renderer);
    close(window);
}

// A unit toggle asks for its unit, and shows it once the service's state says so.
fn unit_toggles(app: &gtk::Application) {
    let (window, asked) = shown(app);
    window.state(base());
    asked.borrow_mut().clear();
    let toggle = |name: &str| {
        window
            .names()
            .find(name)
            .and_then(|w| w.downcast::<gtk::ToggleButton>().ok())
            .expect(name)
    };
    toggle("Fahrenheit").emit_clicked();
    run_for(Duration::from_millis(20));
    assert_eq!(
        *asked.borrow(),
        [Request::SetTemperatureUnit(TemperatureUnit::Fahrenheit)]
    );
    assert_eq!(window.display_page().unit_shown(), TemperatureUnit::Celsius);
    window.state(with(|s| {
        s.temperature_unit = Some(TemperatureUnit::Fahrenheit)
    }));
    assert_eq!(
        window.display_page().unit_shown(),
        TemperatureUnit::Fahrenheit
    );
    assert_eq!(
        asked.borrow().len(),
        1,
        "the service's own change asks for nothing"
    );
    toggle("Fahrenheit").emit_clicked();
    run_for(Duration::from_millis(20));
    assert_eq!(asked.borrow().len(), 1, "the unit already shown");
    close(window);
}

// The whole window on the Display page, saved for a person to look at: mode 9 with the GPU's
// metrics unavailable, so the flags and reason lines show.
fn picture_file(app: &gtk::Application) {
    let (window, _) = shown(app);
    let reasons: HashMap<u32, String> = [
        (2, "Gpu usage"),
        (8, "Gpu temperature"),
        (9, "Gpu temperature"),
    ]
    .into_iter()
    .map(|(m, metric)| (m, format!("{metric}: no device found")))
    .collect();
    window.state(with(|s| {
        s.mode = Some(9);
        s.unavailable_reasons = Some(reasons);
    }));
    run_until("the tiles draw", || draws(&window).iter().all(|d| *d > 0));
    run_for(Duration::from_millis(200));
    let renderer = Renderer::new();
    let path = save_png(&renderer.0, window.widget(), "display-page");
    assert!(path.exists());
    print!("{}; ", path.display());
    drop(renderer);
    close(window);
}
