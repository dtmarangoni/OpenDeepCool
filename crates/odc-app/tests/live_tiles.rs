//! The live tiles in a headless weston (Phase 11a Task 14; layer 3; design: Live tiles): the tick
//! exists exactly while a tile is on screen, the window isn't minimised or suspended and the
//! system hasn't asked for reduced motion, and starts again at home; a tile redraws only when its
//! picture changes; the Music tiles move with `Levels`, once each, and only while the sound can
//! be live; a restart takes every tile home together. Behind the `weston-tests` feature.

mod common;

use std::collections::HashMap;
use std::process::ExitCode;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use odc_app::live::{Live, sample_values, values_from};
use odc_app::ui::tiles::TileWidget;
use odc_app::ui::window::{Window, away, reduced_motion};
use odc_ipc::State;
use odc_protocol::Frame;
use odc_render::{COLUMNS, Mode, MusicSettings, Unit, Values};
use odc_sensors::Metric;

use common::{Check, animations, base, close, run_for, run_until, sample_readings, shown};

fn main() -> ExitCode {
    common::run(&CHECKS)
}

const CHECKS: [(&str, Check); 10] = [
    (
        "the_tick_exists_exactly_while_a_tile_is_on_screen",
        tick_follows_the_screen,
    ),
    (
        "the_tick_stops_for_a_window_that_is_away_and_for_reduced_motion",
        tick_stops_for_away_and_reduced,
    ),
    (
        "a_toplevel_state_is_away_when_minimised_or_suspended",
        states,
    ),
    (
        "a_tile_redraws_only_when_its_picture_changes",
        redraws_only_on_change,
    ),
    (
        "a_reading_that_reaches_100_scrolls_its_tiles_at_the_tick",
        scrolling,
    ),
    (
        "levels_move_the_music_tiles_once_each_and_only_while_the_sound_can_be_live",
        levels,
    ),
    ("a_restart_takes_every_tile_home_together", restart_events),
    ("a_metric_without_a_value_draws_dashes", dashes),
    (
        "the_tick_follows_a_tile_that_is_mapped_again_at_home",
        mapped_again,
    ),
    (
        "nothing_is_drawn_while_no_tile_is_on_screen_and_the_page_catches_up",
        nothing_off_screen,
    ),
];

fn tiles(window: &Window) -> &[TileWidget] {
    window.display_page().tiles()
}

fn frames(window: &Window) -> Vec<Frame> {
    tiles(window).iter().map(TileWidget::frame).collect()
}

fn draws(window: &Window) -> Vec<u32> {
    tiles(window).iter().map(TileWidget::draws).collect()
}

fn with(change: impl FnOnce(&mut State)) -> State {
    let mut state = base();
    change(&mut state);
    state
}

/// The tiles as a fresh `Live` shows them at home, in °C with the default Music settings.
fn at_home(values: Values) -> Vec<Frame> {
    at_home_in(Unit::Celsius, values)
}

/// [`at_home`] in `unit`.
fn at_home_in(unit: Unit, values: Values) -> Vec<Frame> {
    let live = Live::new(unit, MusicSettings::default(), values);
    Mode::all().map(|mode| *live.frame(mode)).collect()
}

/// A `Readings` map with `metric` at `n`, over the sample readings.
fn readings_with(metric: Metric, n: i64) -> HashMap<String, (i64, u64)> {
    let mut readings = sample_readings();
    readings.insert(metric.spec_name().to_owned(), (n, 1));
    readings
}

fn steps_over(window: &Window, wait: Duration) -> u64 {
    let before = window.display_page().steps();
    run_for(wait);
    window.display_page().steps() - before
}

// The tick exists while the Display page is the visible page of a shown window, and not on the
// Configuration page, with the window hidden or with the service gone; it counts the cooler's
// 100 ms, so ten steps a second; and it starts again with every tile at home.
fn tick_follows_the_screen(app: &gtk::Application) {
    animations(true);
    let (window, _) = shown(app);
    window.state(base());
    run_until("the tick starts", || window.display_page().moving());
    let steps = steps_over(&window, Duration::from_millis(1000));
    assert!((8..=12).contains(&steps), "{steps} steps in a second");
    // The Configuration page: the tiles are off screen.
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(100));
    assert!(!window.display_page().moving(), "no tick on Configuration");
    assert_eq!(steps_over(&window, Duration::from_millis(500)), 0);
    // Back: every tile at home at once.
    WidgetExt::activate_action(window.widget(), "win.back", None).expect("back");
    run_for(Duration::from_millis(30));
    assert!(window.display_page().moving(), "the tick starts again");
    // A hidden window.
    window.widget().set_visible(false);
    run_for(Duration::from_millis(100));
    assert!(
        !window.display_page().moving(),
        "no tick for a hidden window"
    );
    assert_eq!(steps_over(&window, Duration::from_millis(500)), 0);
    window.widget().present();
    run_until("the tick starts again", || window.display_page().moving());
    // The service goes: the off screen replaces the pages.
    window.down();
    run_for(Duration::from_millis(100));
    assert!(!window.display_page().moving(), "no tick with no service");
    window.state(base());
    run_until("the tick starts again", || window.display_page().moving());
    close(window);
}

// A minimised or suspended window, and each of GTK's two settings for reduced motion, stop the
// tick and leave the tiles at rest; each is followed live; the app never writes either.
fn tick_stops_for_away_and_reduced(app: &gtk::Application) {
    animations(true);
    let (window, _) = shown(app);
    window.state(base());
    run_until("the tick starts", || window.display_page().moving());
    let page = window.display_page();
    // Away.
    page.set_away(true);
    assert!(!page.moving(), "away");
    assert_eq!(steps_over(&window, Duration::from_millis(400)), 0);
    page.set_away(false);
    assert!(page.moving(), "back");
    // Animations off.
    let settings = gtk::Settings::default().expect("settings");
    assert!(!reduced_motion(&settings));
    animations(false);
    run_for(Duration::from_millis(30));
    assert!(reduced_motion(&settings));
    assert!(!page.moving(), "gtk-enable-animations off");
    assert_eq!(steps_over(&window, Duration::from_millis(400)), 0);
    // At rest: the tiles are the first pictures, the sound not live.
    assert_eq!(frames(&window), at_home(sample_values()));
    animations(true);
    run_for(Duration::from_millis(30));
    assert!(page.moving(), "gtk-enable-animations on again");
    // The portal's preference, from GTK 4.22: by name, where the property exists.
    if let Some(spec) = settings.find_property("gtk-interface-reduced-motion") {
        let enum_class = glib::EnumClass::with_type(spec.value_type()).expect("an enum");
        let reduce = enum_class.to_value_by_nick("reduce").expect("reduce");
        let none = enum_class
            .to_value_by_nick("no-preference")
            .expect("no-preference");
        settings.set_property_from_value("gtk-interface-reduced-motion", &reduce);
        run_for(Duration::from_millis(30));
        assert!(!page.moving(), "gtk-interface-reduced-motion reduce");
        settings.set_property_from_value("gtk-interface-reduced-motion", &none);
        run_for(Duration::from_millis(30));
        assert!(page.moving(), "gtk-interface-reduced-motion no preference");
    }
    close(window);
}

// What counts as away: minimised, or suspended; nothing else.
fn states(_app: &gtk::Application) {
    use gtk::gdk::ToplevelState as S;
    assert!(away(S::MINIMIZED));
    assert!(away(S::SUSPENDED));
    assert!(away(S::MINIMIZED | S::FOCUSED));
    assert!(away(S::SUSPENDED | S::MAXIMIZED));
    for state in [
        S::empty(),
        S::FOCUSED,
        S::MAXIMIZED | S::FOCUSED,
        S::FULLSCREEN,
        S::TILED,
    ] {
        assert!(!away(state), "{state:?}");
    }
}

// With values below 100 and no sound, only the four emoji tiles change their picture, so only
// they draw; the others draw nothing while the tick runs.
fn redraws_only_on_change(app: &gtk::Application) {
    animations(true);
    let (window, _) = shown(app);
    window.state(base());
    run_until("the tick starts", || window.display_page().moving());
    run_for(Duration::from_millis(200));
    let before = draws(&window);
    run_for(Duration::from_millis(2200));
    let after = draws(&window);
    let drew: Vec<u16> = tiles(&window)
        .iter()
        .zip(before.iter().zip(&after))
        .filter(|(_, (b, a))| a > b)
        .map(|(tile, _)| tile.mode.number())
        .collect();
    assert_eq!(drew, [14, 15, 16, 17], "the emoji loops alone");
    // A loop changes its picture on some of its 9 or 11 ticks, so a tile draws less than once a
    // tick: in 2.2 s, at most 22 times.
    for (n, (b, a)) in before.iter().zip(&after).enumerate() {
        assert!(a - b <= 22, "mode {} drew {} times", n + 1, a - b);
    }
    close(window);
}

// A value of 100 or more scrolls: the tiles that show it move a column at each tick, and the
// others don't.
fn scrolling(app: &gtk::Application) {
    animations(true);
    let (window, _) = shown(app);
    window.state(base());
    run_until("the tick starts", || window.display_page().moving());
    window.state(with(|s| {
        s.readings = Some(readings_with(Metric::CpuUsage, 250))
    }));
    run_for(Duration::from_millis(200));
    let before = draws(&window);
    let steps = steps_over(&window, Duration::from_millis(1000));
    let after = draws(&window);
    // CPU usage is shown by modes 1, 4, 5 and 20.
    for mode in [1_usize, 4, 5, 20] {
        let n = mode - 1;
        let drew = after[n] - before[n];
        assert!(
            drew + 2 >= u32::try_from(steps).expect("a few steps"),
            "mode {mode}: {drew} draws in {steps} steps"
        );
    }
    for mode in [2_usize, 3, 6, 7, 8, 9, 10, 11, 12] {
        assert_eq!(
            after[mode - 1],
            before[mode - 1],
            "mode {mode} doesn't move"
        );
    }
    close(window);
}

/// `levels` of one tick: column `c` at `(c + 1) / 15`.
fn rising() -> [f64; COLUMNS] {
    std::array::from_fn(|c| (c as f64 + 1.0) / 15.0)
}

// While a Music mode is shown and a helper's sound is attached, each `Levels` steps the shared
// spectrum and the Dancer once, in the tiles of 13 to 23, as `Live` does; one that comes with no
// sound attached, outside a Music mode, off screen or at rest is dropped.
fn levels(app: &gtk::Application) {
    animations(true);
    let (window, _) = shown(app);
    let music = |connected: bool, mode: u32| {
        with(|s| {
            s.mode = Some(mode);
            s.audio_connected = Some(connected);
        })
    };
    window.state(music(true, 13));
    run_until("the tick starts", || window.display_page().moving());
    let still = at_home(sample_values());
    // Until the first `Levels` the Music tiles show the still sample.
    let music_tiles: Vec<usize> = (12..23).collect();
    for &n in &music_tiles {
        assert_eq!(tiles(&window)[n].frame(), still[n], "mode {}", n + 1);
    }
    // Two `Levels`: the spectrum and the Dancer each stepped twice, from rest.
    window.levels(&rising());
    window.levels(&rising());
    let mut reference = Live::new(Unit::Celsius, MusicSettings::default(), sample_values());
    reference.set_sound(true);
    reference.levels(&rising());
    reference.levels(&rising());
    let moved: Vec<Mode> = Mode::all()
        .filter(|mode| {
            tiles(&window)[usize::from(mode.number()) - 1].frame()
                != still[usize::from(mode.number()) - 1]
        })
        .collect();
    assert!(
        moved.iter().all(|m| (13..=23).contains(&m.number())),
        "only the Music tiles moved"
    );
    for mode in [13_u16, 18, 19, 20, 21, 22] {
        let n = usize::from(mode) - 1;
        assert_ne!(tiles(&window)[n].frame(), still[n], "mode {mode} is live");
    }
    // The spectrum's rows are the reference's: the tick between moved only the strips and loops,
    // so compare the spectrum of mode 13, which has no top.
    assert_eq!(
        tiles(&window)[12].frame(),
        *reference.frame(Mode::new(13).expect("a mode")),
        "mode 13: the spectrum after two `Levels`"
    );
    // No sound attached: the tiles go back to the still sample at once, and `Levels` are dropped.
    window.state(music(false, 13));
    assert_eq!(tiles(&window)[12].frame(), still[12], "no sound");
    window.levels(&rising());
    assert_eq!(tiles(&window)[12].frame(), still[12], "dropped, no sound");
    // Sound, but a mode that isn't Music.
    window.state(music(true, 1));
    window.levels(&rising());
    assert_eq!(tiles(&window)[12].frame(), still[12], "dropped, not Music");
    // Sound and Music again: the first `Levels` starts the spectrum from rest.
    window.state(music(true, 13));
    window.levels(&rising());
    let mut first = Live::new(Unit::Celsius, MusicSettings::default(), sample_values());
    first.set_sound(true);
    first.levels(&rising());
    assert_eq!(
        tiles(&window)[12].frame(),
        *first.frame(Mode::new(13).expect("a mode")),
        "the first `Levels` after a start"
    );
    // Off screen: dropped, and the tiles at rest on return.
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(100));
    let drawn = draws(&window);
    window.levels(&rising());
    assert_eq!(draws(&window), drawn, "dropped, off screen");
    close(window);
}

// A confirmed mode change and a successful Refresh take every tile home together, at once.
fn restart_events(app: &gtk::Application) {
    animations(true);
    let (window, _) = shown(app);
    window.state(with(|s| {
        s.readings = Some(readings_with(Metric::CpuUsage, 250));
    }));
    run_until("the tick starts", || window.display_page().moving());
    // Let the loops and strips move on.
    run_for(Duration::from_millis(600));
    let readings = readings_with(Metric::CpuUsage, 250);
    let values = values_from(Some(&readings));
    let home = at_home(values.clone());
    assert_ne!(frames(&window), home, "the tiles have moved on");
    window.state(with(|s| {
        s.readings = Some(readings_with(Metric::CpuUsage, 250));
        s.mode = Some(4);
    }));
    assert_eq!(frames(&window), home, "a mode change: every tile at home");
    run_for(Duration::from_millis(600));
    assert_ne!(frames(&window), home);
    window.refreshed();
    assert_eq!(frames(&window), home, "Refresh: every tile at home");
    // A confirmed unit change: every tile at home in the new unit, the temperature tiles
    // different from before.
    run_for(Duration::from_millis(600));
    let fahrenheit = at_home_in(Unit::Fahrenheit, values.clone());
    assert_ne!(fahrenheit, home, "the unit shows in the temperature tiles");
    window.state(with(|s| {
        s.readings = Some(readings_with(Metric::CpuUsage, 250));
        s.mode = Some(4);
        s.temperature_unit = Some(odc_ipc::TemperatureUnit::Fahrenheit);
    }));
    assert_eq!(
        frames(&window),
        fahrenheit,
        "a unit change: every tile at home"
    );
    // A confirmed GPU change: the same.
    run_for(Duration::from_millis(600));
    assert_ne!(frames(&window), fahrenheit);
    window.state(with(|s| {
        s.readings = Some(readings_with(Metric::CpuUsage, 250));
        s.mode = Some(4);
        s.temperature_unit = Some(odc_ipc::TemperatureUnit::Fahrenheit);
        s.gpu_in_use = Some("0000:0a:00.0".to_owned());
    }));
    assert_eq!(
        frames(&window),
        fahrenheit,
        "a GPU change: every tile at home"
    );
    // A Music setting restarts nothing: the pictures go on from where they are.
    run_for(Duration::from_millis(600));
    let moved = frames(&window);
    window.state(with(|s| {
        s.readings = Some(readings_with(Metric::CpuUsage, 250));
        s.mode = Some(4);
        s.temperature_unit = Some(odc_ipc::TemperatureUnit::Fahrenheit);
        s.gpu_in_use = Some("0000:0a:00.0".to_owned());
        s.music_peaks = Some(false);
    }));
    let after = frames(&window);
    for n in 0..12 {
        assert_eq!(
            after[n],
            moved[n],
            "mode {} doesn't restart for a Music setting",
            n + 1
        );
    }
    close(window);
}

// A metric the service has no value for is drawn as dashes, by the tiles that show it only; one
// that comes back is drawn as its number again.
fn dashes(app: &gtk::Application) {
    animations(false);
    let (window, _) = shown(app);
    let mut readings = sample_readings();
    readings.remove(Metric::GpuUsage.spec_name());
    window.state(with(|s| s.readings = Some(readings)));
    let sample = at_home(sample_values());
    let gap = {
        let mut values = sample_values();
        values.set(Metric::GpuUsage, None);
        at_home(values)
    };
    assert_eq!(frames(&window), gap);
    // GPU usage is shown by modes 2, 4 (a pair), 6 (a pair) and 21; no other tile differs.
    let differ: Vec<usize> = (0..23).filter(|&n| gap[n] != sample[n]).collect();
    assert_eq!(differ, [1, 3, 5, 20]);
    window.state(base());
    assert_eq!(frames(&window), sample, "the number is back");
    // No Readings at all: every metric dashes, not the sample.
    window.state(with(|s| s.readings = Some(HashMap::new())));
    assert_eq!(frames(&window), at_home(Values::new()));
    close(window);
}

// A tile that is mapped again starts at home with the rest, as one that was never away.
fn mapped_again(app: &gtk::Application) {
    animations(true);
    let (window, _) = shown(app);
    window.state(with(|s| {
        s.readings = Some(readings_with(Metric::CpuUsage, 250));
    }));
    run_until("the tick starts", || window.display_page().moving());
    run_for(Duration::from_millis(700));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(50));
    WidgetExt::activate_action(window.widget(), "win.back", None).expect("back");
    // At once, as the page maps: every tile at home, before the first step.
    let readings = readings_with(Metric::CpuUsage, 250);
    let values = values_from(Some(&readings));
    run_until("the tick starts", || window.display_page().moving());
    assert_eq!(frames(&window), at_home(values), "at home on return");
    close(window);
}

// While no tile can be seen, a new input is taken in and nothing is drawn: no picture changes and
// no tile queues a redraw. On return the tiles are drawn from what the input became, at home, with
// the tick moving them or, under reduced motion, at rest (design: Live tiles, When the tick runs).
fn nothing_off_screen(app: &gtk::Application) {
    for animations_on in [false, true] {
        animations(animations_on);
        let (window, _) = shown(app);
        window.state(base());
        run_for(Duration::from_millis(100));
        WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
        run_for(Duration::from_millis(100));
        assert!(!window.display_page().moving(), "no tick on Configuration");
        let (drawn, shown_before) = (draws(&window), frames(&window));
        // New readings, then another unit: the input changed twice off screen.
        window.state(with(|s| {
            s.readings = Some(readings_with(Metric::CpuUsage, 250))
        }));
        window.state(with(|s| {
            s.readings = Some(readings_with(Metric::CpuUsage, 250));
            s.temperature_unit = Some(odc_ipc::TemperatureUnit::Fahrenheit);
        }));
        run_for(Duration::from_millis(300));
        assert_eq!(
            draws(&window),
            drawn,
            "nothing drawn off screen (animations {animations_on})"
        );
        assert_eq!(
            frames(&window),
            shown_before,
            "no picture changed off screen"
        );
        // Back: drawn from the new input, at home.
        WidgetExt::activate_action(window.widget(), "win.back", None).expect("back");
        let values = values_from(Some(&readings_with(Metric::CpuUsage, 250)));
        assert_eq!(
            frames(&window),
            at_home_in(Unit::Fahrenheit, values),
            "on return, from the new input (animations {animations_on})"
        );
        assert_eq!(window.display_page().moving(), animations_on);
        close(window);
    }
    animations(true);
}
