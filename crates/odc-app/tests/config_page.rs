//! The Configuration page in a headless weston (Phase 10 plan, Task 6; layer 3): every control
//! asks for its call with the right argument and shows the saved value until the service
//! confirms; the GPU row only with more than one GPU, Automatic first, naming the GPU it reads;
//! the dropdown's entries, its names and its notes. Against a fake client, through
//! `common::run`. Behind the `weston-tests` feature.

mod common;

use std::process::ExitCode;
use std::time::Duration;

use gtk::prelude::*;
use odc_app::ui::dropdowns::Dropdown;
use odc_app::ui::window::Window;
use odc_app::ui::{Reason, Refused, Request};
use odc_ipc::{FallStop, Gpu, GpuKind, State, TemperatureUnit};

use common::{Check, Picture, Renderer, base, close, label_texts, near, run_for, save_png, shown};

fn main() -> ExitCode {
    common::run(&CHECKS)
}

const CHECKS: [(&str, Check); 11] = [
    ("the_rows_sit_left_with_their_controls_in_one_column", rows),
    ("every_control_asks_for_its_call", calls),
    (
        "the_unit_sits_after_rotation_and_changes_once_the_service_says_so",
        unit_toggles,
    ),
    ("the_gpu_row_with_one_gpu_and_several", gpu_row),
    ("a_refused_change_shows_the_saved_value_again", refused),
    (
        "the_sentences_the_owner_removed_stay_removed",
        removed_lines,
    ),
    (
        "the_theme_line_sits_under_use_system_theme_in_every_state",
        theme_line_place,
    ),
    ("the_cards_are_6_px_apart", card_gaps),
    (
        "the_space_under_the_last_card_is_the_space_above_the_first",
        page_ends,
    ),
    ("the_scales_and_the_switches_share_one_knob", knob_colours),
    ("the_page_saved_as_a_picture", picture_file),
];

// The rows (Rows): each label block and its control side by side, 24 px apart, left-aligned;
// the controls of every card start at one x, with or without the GPU row; a long hint wraps at
// its 28 characters; the switches are 32 x 18 and the scales' knobs 16; the Startup card's label block shares the
// group's width on the page and has its own on the off screen.
fn rows(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(with(|_| {}));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(250));
    let page = window.config_page().cards().clone();
    let place = |widget: &gtk::Widget| {
        widget
            .compute_bounds(&page)
            .expect("the widget has a place")
    };
    let parent = |name: &str| {
        find::<gtk::Widget>(&window, name)
            .parent()
            .expect("a parent")
    };
    // Each row's control: the box around the toggles, the dropdown's box, the switch, the scale.
    let controls: Vec<(&str, gtk::Widget)> = vec![
        ("Rotation", parent("Rotation 0°")),
        ("Temperature unit", parent("Celsius")),
        ("GPU", parent("GPU")),
        ("Peak dots", find::<gtk::Widget>(&window, "Peak dots")),
        ("Bar fall", find::<gtk::Widget>(&window, "Bar fall")),
        ("Peak fall", find::<gtk::Widget>(&window, "Peak fall")),
        (
            "Start at boot",
            find::<gtk::Widget>(&window, "Start at boot"),
        ),
    ];
    let x_of = |name: &str| place(&controls.iter().find(|(n, _)| *n == name).expect(name).1).x();
    for (name, control) in &controls {
        let block = control.prev_sibling().expect(name);
        let (block_box, control_box) = (place(&block), place(control));
        let gap = control_box.x() - (block_box.x() + block_box.width());
        assert!(
            (gap - 24.0).abs() <= 1.0,
            "{name}: {gap} px between label and control"
        );
        assert!(!control.hexpands(), "{name} doesn't expand");
    }
    let x0 = x_of("Rotation");
    for (name, _) in &controls {
        assert!(
            (x_of(name) - x0).abs() <= 1.0,
            "{name} starts at {}, not at {x0}",
            x_of(name)
        );
    }
    // A hint wraps at 28 characters (Rows): the Peak dots', the one row with a hint under its
    // title now that the owner removed the others (2026-11-19). Its text fits a line at that
    // width, so the rule is checked on the label, not on a count of lines.
    let hint = find::<gtk::Widget>(&window, "Peak dots")
        .prev_sibling()
        .and_then(|block| block.last_child())
        .and_then(|w| w.downcast::<gtk::Label>().ok())
        .expect("the Peak dots' hint");
    assert_eq!(hint.text(), "A dot above each bar that falls slowly");
    assert!(hint.wraps());
    assert_eq!((hint.width_chars(), hint.max_width_chars()), (28, 28));
    // The scales' knobs are round and 16 px, and the bars keep their 4 px (The scales' knobs).
    for name in ["Bar fall", "Peak fall"] {
        let scale = find::<gtk::Scale>(&window, name);
        let (start, end) = scale.slider_range();
        assert_eq!(end - start, 16, "{name}: the knob");
        assert_eq!(scale.range_rect().height(), 4, "{name}: the bar");
    }
    // Each switch is level with the top of its label block (owner, 2026-11-22), the Peak dots' one
    // too, whose block is taller than the switch because of the hint under its title.
    for name in ["Use system theme", "Peak dots", "Start at boot"] {
        let switch = find::<gtk::Switch>(&window, name);
        let block = switch.prev_sibling().expect("a label block");
        let (switch_box, block_box) = (place(switch.upcast_ref()), place(&block));
        assert!(
            (switch_box.y() - block_box.y()).abs() <= 1.0,
            "{name}: the switch's top {} and the block's {}",
            switch_box.y(),
            block_box.y()
        );
    }
    let peaks = find::<gtk::Switch>(&window, "Peak dots");
    let block = peaks.prev_sibling().expect("the Peak dots' block");
    assert!(
        place(&block).height() > place(peaks.upcast_ref()).height() + 8.0,
        "the hint makes the block taller than the switch, so centring would show"
    );
    // The switches.
    for name in ["Peak dots", "Start at boot"] {
        let switch = find::<gtk::Switch>(&window, name);
        // The outside size, border included: `width()` is the content box, 2 px less.
        let bounds = place(switch.upcast_ref());
        assert_eq!((bounds.width(), bounds.height()), (32.0, 18.0), "{name}");
    }
    // The Startup card's block: the group's width here, its own on the off screen.
    let startup_block = |window: &Window| {
        window
            .startup()
            .switch()
            .parent()
            .and_then(|row| row.first_child())
            .expect("the Startup row's block")
    };
    let group_width = place(&controls[0].1.prev_sibling().expect("a block")).width();
    let on_page = startup_block(&window).width() as f32;
    assert!(
        (on_page - group_width).abs() <= 1.0,
        "{on_page} px on the page, the group {group_width}"
    );
    // The same columns without the GPU row.
    window.state(base());
    run_for(Duration::from_millis(250));
    assert!(
        window.config_page().gpu_row().is_none(),
        "one GPU: no GPU row"
    );
    for (name, control) in controls.iter().filter(|(n, _)| *n != "GPU") {
        let moved = place(control).x();
        assert!((moved - x0).abs() <= 1.0, "{name} moved to {moved}");
    }
    close(window);
}

// The temperature unit is a row of the Display card after Rotation, with the same kind of
// control; a toggle asks for its unit and shows it once the service's state says so.
fn unit_toggles(app: &gtk::Application) {
    let (window, asked) = shown(app);
    window.state(with(|_| {}));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(100));
    let toggle = |name: &str| find::<gtk::ToggleButton>(&window, name);
    // Rotation first, then the unit, then the GPU, each below the one before.
    let card = find::<gtk::Box>(&window, "Display settings");
    let top = |widget: &gtk::Widget| {
        widget
            .compute_bounds(&card)
            .map(|b| b.y())
            .expect("the widget has a place")
    };
    let rotation = top(toggle("Rotation 0°").upcast_ref());
    let unit = top(toggle("Celsius").upcast_ref());
    let gpu = top(find::<gtk::Widget>(&window, "GPU").upcast_ref());
    assert!(rotation < unit && unit < gpu, "{rotation} {unit} {gpu}");

    asked.borrow_mut().clear();
    toggle("Fahrenheit").emit_clicked();
    run_for(Duration::from_millis(20));
    assert_eq!(
        *asked.borrow(),
        [Request::SetTemperatureUnit(TemperatureUnit::Fahrenheit)]
    );
    assert_eq!(window.config_page().unit_shown(), TemperatureUnit::Celsius);
    window.state(with(|s| {
        s.temperature_unit = Some(TemperatureUnit::Fahrenheit)
    }));
    assert_eq!(
        window.config_page().unit_shown(),
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

// Made-up GPUs (AGENTS.md rule 10), in the service's order.
/// The sentences the owner took off the Configuration page (2026-11-19: Tasks 19 to 24 of Phase
/// 11a) and the words each began with: none of them is on the page, in any card.
const REMOVED: [(&str, &str); 3] = [
    ("except Dancer", "the Music card's closing sentence"),
    ("Turns the picture clockwise", "the Rotation row's hint"),
    (
        "Used wherever the display shows",
        "the Temperature unit row's hint",
    ),
];

// The page holds none of the sentences the owner removed.
fn removed_lines(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(with(|_| {}));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(250));
    let mut texts = Vec::new();
    label_texts(window.config_page().widget().upcast_ref(), &mut texts);
    for (words, what) in REMOVED {
        assert!(
            texts.iter().all(|t| !t.contains(words)),
            "{what} is back: {texts:?}"
        );
    }
    close(window);
}

// The cards are 6 px apart, one under the other (owner, 2026-11-19: 18 before; 2026-11-22: 10
// before): the gap between a card and the next, and nothing else of the page's margins.
fn card_gaps(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(with(|_| {}));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(250));
    let page = window.config_page().cards().clone();
    let place = |name: &str| {
        find::<gtk::Widget>(&window, name)
            .compute_bounds(&page)
            .map(|b| (b.y(), b.y() + b.height()))
            .unwrap_or_else(|| panic!("{name} has a place"))
    };
    let names = [
        "Theme settings",
        "Display settings",
        "Music settings",
        "Startup settings",
    ];
    for pair in names.windows(2) {
        let (above, below) = (place(pair[0]), place(pair[1]));
        assert_eq!(
            below.0 - above.1,
            6.0,
            "{} to {}: {above:?}, {below:?}",
            pair[0],
            pair[1]
        );
    }
    close(window);
}

// The page has as much room under its last card as from its top to its first card (owner,
// 2026-11-22): the page bar's 12, 32 and 18, and the content's 6, 68 px, under the Startup card
// too.
fn page_ends(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(with(|_| {}));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(300));
    let page = window.config_page().widget().clone();
    let place = |widget: &gtk::Widget| widget.compute_bounds(&page).expect("a place in the page");
    let (first, last) = (
        place(&find::<gtk::Widget>(&window, "Theme settings")),
        place(&find::<gtk::Widget>(&window, "Startup settings")),
    );
    let page_foot = f32::from(page.height() as i16);
    let (above, below) = (first.y(), page_foot - (last.y() + last.height()));
    assert!(
        (above - 68.0).abs() <= 1.0 && (below - 68.0).abs() <= 1.0,
        "{above} px above the first card, {below} under the last, the page {page_foot} high"
    );
    close(window);
}

// The line that says which theme is used sits directly under the Use system theme row and above
// Appearance, in every state of the card (owner, 2026-11-19; it was at the card's foot).
fn theme_line_place(app: &gtk::Application) {
    let (window, _) = shown(app);
    // The page opens once the service has answered.
    window.state(with(|_| {}));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    for (what, state) in [
        ("the system's theme, followed", with(|_| {})),
        (
            "OpenDeepCool's own dark theme",
            with(|s| s.use_system_theme = Some(false)),
        ),
        (
            "OpenDeepCool's own light theme",
            with(|s| {
                s.use_system_theme = Some(false);
                s.dark_theme = Some(false);
            }),
        ),
    ] {
        window.state(state);
        run_for(Duration::from_millis(250));
        let page = window.config_page().cards().clone();
        let place = |name: &str| {
            find::<gtk::Widget>(&window, name)
                .compute_bounds(&page)
                .map(|b| (b.y(), b.y() + b.height()))
                .unwrap_or_else(|| panic!("{name} has a place"))
        };
        let (switch, line, dark) = (
            place("Use system theme"),
            place("Theme status"),
            place("Dark theme"),
        );
        assert!(
            switch.1 <= line.0 + 1.0 && line.1 <= dark.0 + 1.0,
            "{what}: the switch {switch:?}, the line {line:?}, the toggles {dark:?}"
        );
        // The line is as far under its title as a hint is under its title (owner, 2026-11-22):
        // the label block's 2 px, here and in the Peak dots' row.
        let gap_under_title = |control: &str, under: &str| {
            let title = find::<gtk::Widget>(&window, control)
                .prev_sibling()
                .and_then(|block| block.first_child())
                .expect("the row's title");
            let (title_at, under_at) = (
                title.compute_bounds(&page).expect("a place"),
                find::<gtk::Widget>(&window, under)
                    .compute_bounds(&page)
                    .expect("a place"),
            );
            under_at.y() - (title_at.y() + title_at.height())
        };
        let theme_gap = gap_under_title("Use system theme", "Theme status");
        let hint_gap = {
            let peaks = find::<gtk::Widget>(&window, "Peak dots");
            let block = peaks.prev_sibling().expect("the block");
            let (title, hint) = (
                block.first_child().expect("the title"),
                block.last_child().expect("the hint"),
            );
            let (t, h) = (
                title.compute_bounds(&page).expect("a place"),
                hint.compute_bounds(&page).expect("a place"),
            );
            h.y() - (t.y() + t.height())
        };
        assert!(
            (theme_gap - hint_gap).abs() <= 0.5 && (theme_gap - 2.0).abs() <= 0.5,
            "{what}: {theme_gap} px under the Use system theme title, {hint_gap} under Peak dots'"
        );
    }
    close(window);
}

fn gpus() -> Vec<Gpu> {
    let gpu = |pci: &str, kind, has_rule| Gpu {
        pci: pci.to_owned(),
        driver: "contosogpu".to_owned(),
        kind,
        vendor: 0x0a0a,
        device: 0x0001,
        has_rule,
    };
    vec![
        gpu("0000:0a:00.0", GpuKind::Dedicated, true),
        gpu("0000:00:02.0", GpuKind::Integrated, true),
        gpu("0000:07:00.0", GpuKind::Unknown, false),
    ]
}

fn with(change: impl FnOnce(&mut State)) -> State {
    let mut state = base();
    state.gpus = Some(gpus());
    state.gpu_in_use = Some("0000:0a:00.0".to_owned());
    change(&mut state);
    state
}

fn find<T: IsA<gtk::Widget>>(window: &Window, name: &str) -> T {
    window
        .names()
        .find(name)
        .and_then(|w| w.downcast::<T>().ok())
        .unwrap_or_else(|| panic!("{name}"))
}

/// What the window asked since the last call.
fn taken(asked: &common::Log<Request>) -> Vec<Request> {
    asked.borrow_mut().drain(..).collect()
}

/// Picks `position` as a user would, and lets GTK settle.
fn pick(dropdown: &Dropdown, position: u32) {
    dropdown.widget().set_selected(position);
    run_for(Duration::from_millis(30));
}

// Each rotation, each GPU entry and Automatic, the switch and each scale's stops: the right call with the right argument, once, and the saved value shown until
// the service says otherwise.
fn calls(app: &gtk::Application) {
    let (window, asked) = shown(app);
    window.state(with(|s| s.chosen_gpu = Some("0000:00:02.0".to_owned())));
    taken(&asked);
    let page = window.config_page();

    for (r, name) in [
        (1, "Rotation 90°"),
        (2, "Rotation 180°"),
        (3, "Rotation 270°"),
    ] {
        find::<gtk::ToggleButton>(&window, name).set_active(true);
        run_for(Duration::from_millis(10));
        assert_eq!(taken(&asked), [Request::SetRotation(r)], "{name}");
        assert_eq!(page.shown().0, 0, "the saved rotation still shows");
    }

    // The GPU list: Automatic, then the GPUs, the one without a rule last.
    let pcis = ["", "0000:0a:00.0", "0000:00:02.0", "0000:07:00.0"];
    assert_eq!(page.gpu().chosen(), Some(2));
    for (position, pci) in pcis.iter().enumerate() {
        pick(page.gpu(), position as u32);
        let want: Vec<Request> = if position == 2 {
            Vec::new()
        } else {
            vec![Request::ChooseGpu((*pci).to_owned())]
        };
        assert_eq!(taken(&asked), want, "GPU entry {position}");
        assert_eq!(
            page.gpu().widget().selected(),
            2,
            "the saved GPU still shows"
        );
    }

    // The peak dots: the call, and the switch's state kept until confirmed.
    let peaks = find::<gtk::Switch>(&window, "Peak dots");
    peaks.set_active(false);
    run_for(Duration::from_millis(10));
    assert_eq!(taken(&asked), [Request::SetMusicPeaks(false)]);
    assert!(peaks.state(), "the saved state");

    // The falls: each new stop once, the saved one never.
    let bar = find::<gtk::Scale>(&window, "Bar fall");
    for (value, want) in [
        (4.0, vec![Request::SetMusicBarFall(FallStop::Faster)]),
        (4.0, vec![]),
        (2.0, vec![]),
        (0.0, vec![Request::SetMusicBarFall(FallStop::Slower)]),
        (3.0, vec![Request::SetMusicBarFall(FallStop::Fast)]),
    ] {
        bar.set_value(value);
        assert_eq!(taken(&asked), want, "bar fall at {value}");
    }
    let peak = find::<gtk::Scale>(&window, "Peak fall");
    peak.set_value(2.0);
    assert_eq!(
        taken(&asked),
        [Request::SetMusicPeakFall(FallStop::Moderate)]
    );

    // The service confirms: every control shows the new values, asking for nothing.
    window.state(with(|s| {
        s.chosen_gpu = Some("0000:07:00.0".to_owned());
        s.rotation = Some(2);
        s.music_peaks = Some(false);
        s.music_bar_fall = Some(FallStop::Fast);
        s.music_peak_fall = Some(FallStop::Moderate);
    }));
    run_for(Duration::from_millis(30));
    assert_eq!(page.shown(), (2, false, 3.0, 2.0));
    assert_eq!(page.gpu().chosen(), Some(3));
    assert!(!peaks.state());
    assert_eq!(
        taken(&asked),
        [],
        "showing the service's state asks for nothing"
    );
    close(window);
}

// One GPU: no GPU row. Several: Automatic first and chosen, the line naming the GPU it reads;
// a GPU without a reading rule in use: the hint and its note.
fn gpu_row(app: &gtk::Application) {
    let (window, _) = shown(app);
    let page = window.config_page();
    window.state(with(|s| s.gpus = Some(gpus()[..1].to_vec())));
    assert_eq!(page.gpu_row(), None);
    window.state(with(|_| {}));
    assert_eq!(
        page.gpu_row(),
        Some((
            Some("Using Dedicated · contosogpu · 0000:0a:00.0.".to_owned()),
            None
        ))
    );
    let labels: Vec<String> = page.gpu().entries().into_iter().map(|e| e.label).collect();
    assert_eq!(
        labels,
        [
            "Automatic",
            "Dedicated · contosogpu · 0000:0a:00.0",
            "Integrated · contosogpu · 0000:00:02.0",
            "Unknown type · contosogpu · 0000:07:00.0"
        ]
    );
    assert_eq!(page.gpu().button_text(), "Automatic");
    assert_eq!(
        window.names().description("GPU").as_deref(),
        Some("Using Dedicated · contosogpu · 0000:0a:00.0")
    );
    window.state(with(|s| s.gpu_in_use = Some("0000:07:00.0".to_owned())));
    assert_eq!(
        page.gpu_row(),
        Some((
            Some("Using Unknown type · contosogpu · 0000:07:00.0.".to_owned()),
            Some("No reading for this GPU's driver: the GPU options are unavailable.".to_owned())
        ))
    );
    assert_eq!(page.gpu().entries()[3].note, "no readings");
    close(window);
}

// A refused change: the scale, the rotation and the dropdown show the saved values again.
fn refused(app: &gtk::Application) {
    let (window, asked) = shown(app);
    window.state(with(|_| {}));
    taken(&asked);
    let page = window.config_page();
    find::<gtk::Scale>(&window, "Bar fall").set_value(4.0);
    assert_eq!(taken(&asked), [Request::SetMusicBarFall(FallStop::Faster)]);
    assert_eq!(page.shown().2, 4.0, "the scale follows the drag");
    window.refusal(Refused::Change, &Reason::NotAuthorized);
    run_for(Duration::from_millis(30));
    assert_eq!(page.shown().2, 2.0, "back to Moderate");
    // Asking for the same stop again, after the refusal, asks again.
    find::<gtk::Scale>(&window, "Bar fall").set_value(4.0);
    assert_eq!(taken(&asked), [Request::SetMusicBarFall(FallStop::Faster)]);
    close(window);
}

// The knob of a scale is the colour of a switch's knob, on and off (owner, 2026-11-22): the
// theme's foreground, read from the pixels GTK draws, since a stock gradient or border would
// show nowhere else. The test theme's tokens are `Tokens::default_theme(true)`.
fn knob_colours(app: &gtk::Application) {
    let tokens = odc_app::theme::Tokens::default_theme(true);
    let (window, _) = shown(app);
    window.state(with(|s| s.music_peaks = Some(true)));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(300));
    let root = window.widget().clone();
    let renderer = Renderer::new();
    let bounds = |widget: &gtk::Widget| widget.compute_bounds(&root).expect("a place");
    // A switch is 32 x 18 with a 14 px knob: at the right end when on, the left when off.
    let peaks = find::<gtk::Switch>(&window, "Peak dots");
    let place = bounds(peaks.upcast_ref());
    let picture = Picture::of(&renderer.0, &root);
    let on = picture.at((place.x() + 24.0) as i32, (place.y() + 9.0) as i32);
    assert!(near(on, tokens.fg), "the on switch's knob is {on:?}");
    window.state(with(|s| s.music_peaks = Some(false)));
    run_for(Duration::from_millis(300));
    let picture = Picture::of(&renderer.0, &root);
    let off = picture.at((place.x() + 8.0) as i32, (place.y() + 9.0) as i32);
    assert!(near(off, tokens.fg), "the off switch's knob is {off:?}");
    // A scale's knob is a disc of about 200 px in the theme's foreground: that many pixels of the
    // scale's area have its colour (the bar is the accent, the marks and the ring are not).
    for name in ["Bar fall", "Peak fall"] {
        let scale = find::<gtk::Scale>(&window, name);
        let place = bounds(scale.upcast_ref());
        let (left, top) = (place.x() as i32, place.y() as i32);
        let mut count = 0;
        for y in 0..place.height() as i32 {
            for x in 0..place.width() as i32 {
                if near(picture.at(left + x, top + y), tokens.fg) {
                    count += 1;
                }
            }
        }
        assert!(
            (120..=260).contains(&count),
            "{name}: {count} px of the foreground colour, a 16 px knob has about 200"
        );
    }
    drop(renderer);
    close(window);
}

// The whole window on the Configuration page, saved for a person to look at: several GPUs on
// Automatic.
fn picture_file(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(with(|_| {}));
    WidgetExt::activate_action(window.widget(), "win.configuration", None).expect("the page");
    run_for(Duration::from_millis(300));
    let renderer = Renderer::new();
    let path = save_png(&renderer.0, window.widget(), "configuration-page");
    assert!(path.exists());
    print!("{}; ", path.display());
    drop(renderer);
    close(window);
}
