//! The Configuration page in a headless weston (Phase 10 plan, Task 6; layer 3): every control
//! asks for its call with the right argument and shows the saved value until the service
//! confirms; the GPU row only with more than one GPU, Automatic first, naming the GPU it reads;
//! the dropdowns' entries, their names and their notes, an open list's RPM updating in place;
//! a missing input and no fan sensors. Against a fake client, through `common::run`. Behind the
//! `weston-tests` feature.

mod common;

use std::process::ExitCode;
use std::time::Duration;

use gtk::prelude::*;
use odc_app::model::{INPUTS_HINT, NO_SENSORS_HINT};
use odc_app::ui::dropdowns::{Dropdown, Entry};
use odc_app::ui::window::Window;
use odc_app::ui::{Reason, Refused, Request};
use odc_ipc::{FallStop, FanSpeed, Gpu, GpuKind, Input, State};

use common::{Check, Renderer, base, close, run_for, save_png, shown};

fn main() -> ExitCode {
    common::run(&CHECKS)
}

const CHECKS: [(&str, Check); 6] = [
    ("every_control_asks_for_its_call", calls),
    ("the_gpu_row_with_one_gpu_and_several", gpu_row),
    ("the_dropdowns_entries_names_and_live_rpm", entries),
    ("a_refused_change_shows_the_saved_value_again", refused),
    ("a_missing_input_and_no_fan_sensors", missing),
    ("the_page_saved_as_a_picture", picture_file),
];

// Made-up GPUs (AGENTS.md rule 10), in the service's order.
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

fn input(n: u32) -> Input {
    Input {
        chip: "acme6000".to_owned(),
        device: "devices/platform/acme6000.2592".to_owned(),
        input: n,
    }
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

// Each rotation, each GPU entry and Automatic, each input entry and none, the switch and each
// scale's stops: the right call with the right argument, once, and the saved value shown until
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

    // The inputs: None, then each input.
    pick(page.fans(), 0);
    assert_eq!(taken(&asked), [Request::SetCoolerFanInput(None)]);
    pick(page.fans(), 2);
    assert_eq!(taken(&asked), [Request::SetCoolerFanInput(Some(input(2)))]);
    pick(page.fans(), 1);
    assert_eq!(taken(&asked), [], "the saved input");
    pick(page.pump(), 1);
    assert_eq!(taken(&asked), [Request::SetPumpInput(Some(input(1)))]);
    pick(page.pump(), 0);
    assert_eq!(taken(&asked), [Request::SetPumpInput(None)]);
    assert_eq!(
        (
            page.fans().widget().selected(),
            page.pump().widget().selected()
        ),
        (1, 2)
    );

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
        s.pump_input = Some(None);
    }));
    run_for(Duration::from_millis(30));
    assert_eq!(page.shown(), (2, false, 3.0, 2.0));
    assert_eq!(page.gpu().chosen(), Some(3));
    assert_eq!(page.pump().chosen(), Some(0));
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

// The fan picker's entries, each item named by its label and described by its RPM; the open
// list's rows with the check on the saved entry; new speeds change the open rows in place.
fn entries(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(with(|_| {}));
    let fans = window.config_page().fans();
    let entry = |label: &str, note: &str| Entry {
        label: label.to_owned(),
        note: note.to_owned(),
    };
    assert_eq!(
        fans.entries(),
        [
            entry("None", ""),
            entry("acme6000 · fan1", "1,180 RPM"),
            entry("acme6000 · fan2", "3,400 RPM"),
        ]
    );
    assert_eq!(fans.button_text(), "acme6000 · fan1");
    fans.widget().emit_activate();
    run_for(Duration::from_millis(200));
    let rows = |dropdown: &Dropdown| dropdown.rows();
    let row = |position: u32, label: &str, note: &str, check: bool| {
        (position, label.to_owned(), note.to_owned(), check)
    };
    assert_eq!(
        rows(fans),
        [
            row(0, "None", "", false),
            row(1, "acme6000 · fan1", "1,180 RPM", true),
            row(2, "acme6000 · fan2", "3,400 RPM", false),
        ]
    );
    window.state(with(|s| {
        if let Some(inputs) = s.fan_inputs.as_mut() {
            inputs[0].rpm = FanSpeed::Rpm(1250);
            inputs[1].rpm = FanSpeed::Unavailable;
        }
    }));
    run_for(Duration::from_millis(50));
    assert_eq!(
        rows(fans),
        [
            row(0, "None", "", false),
            row(1, "acme6000 · fan1", "1,250 RPM", true),
            row(2, "acme6000 · fan2", "no reading", false),
        ],
        "the open list's speeds change in place"
    );
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

// A saved input missing, found at start or not: "— not found", the warning border, its hint and
// its description. No fan sensors: both pickers insensitive with the note, and the hint.
fn missing(app: &gtk::Application) {
    let (window, _) = shown(app);
    let page = window.config_page();
    window.state(with(|s| {
        s.cooler_fan_input_present = Some(false);
        s.pump_input = Some(Some(input(9)));
        s.pump_input_present = Some(false);
    }));
    for (dropdown, name, button) in [
        (
            page.fans(),
            "Cooler fans input",
            "acme6000 · fan1 — not found",
        ),
        (page.pump(), "Pump input", "acme6000 · fan9 — not found"),
    ] {
        assert_eq!(dropdown.button_text(), button);
        assert!(dropdown.widget().has_css_class("warn"), "{name}");
        assert_eq!(
            window.names().description(name).as_deref(),
            Some("not found")
        );
    }
    assert_eq!(
        page.pump().entries().last().map(|e| e.note.as_str()),
        Some("not found")
    );
    assert_eq!(page.input_hints(), (true, true, INPUTS_HINT.to_owned()));

    window.state(with(|s| {
        s.fan_inputs = Some(Vec::new());
        s.cooler_fan_input = Some(None);
        s.pump_input = Some(None);
    }));
    for dropdown in [page.fans(), page.pump()] {
        assert!(!dropdown.widget().is_sensitive());
        assert_eq!(dropdown.button_text(), "No fan sensors found");
        assert!(!dropdown.widget().has_css_class("warn"));
    }
    assert_eq!(
        page.input_hints(),
        (false, false, NO_SENSORS_HINT.to_owned())
    );
    close(window);
}

// The whole window on the Configuration page, saved for a person to look at: several GPUs on
// Automatic, the pump's input missing.
fn picture_file(app: &gtk::Application) {
    let (window, _) = shown(app);
    window.state(with(|s| {
        s.pump_input = Some(Some(input(9)));
        s.pump_input_present = Some(false);
    }));
    WidgetExt::activate_action(window.widget(), "win.page-config", None).expect("the page");
    run_for(Duration::from_millis(300));
    let renderer = Renderer::new();
    let path = save_png(&renderer.0, window.widget(), "configuration-page");
    assert!(path.exists());
    print!("{}; ", path.display());
    drop(renderer);
    close(window);
}
