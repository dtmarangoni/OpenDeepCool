//! The view model, one test per row of the design's States table, each asserting the whole
//! `View`; the `write-failed` wait on a fake clock; and the properties of every view.

use std::collections::{BTreeMap, HashMap};

use proptest::prelude::*;

use super::*;

/// The design's tile table (Display page), typed here apart from the model's own.
const TILES: [(u16, &str, &str); 23] = [
    (1, "Load", "CPU"),
    (2, "Load", "GPU"),
    (3, "Load", "RAM"),
    (4, "Load", "CPU + GPU"),
    (5, "Load", "CPU + RAM"),
    (6, "Load", "GPU + RAM"),
    (7, "Temperature", "CPU"),
    (8, "Temperature", "GPU"),
    (9, "Temperature", "CPU + GPU"),
    (10, "Power", "CPU"),
    (11, "Power", "GPU"),
    (12, "Power", "CPU + GPU"),
    (13, "Music", "Spectrum"),
    (14, "Music", "Emoji 1"),
    (15, "Music", "Emoji 2"),
    (16, "Music", "Emoji 3"),
    (17, "Music", "Emoji 4"),
    (18, "Music", "CPU temperature"),
    (19, "Music", "GPU temperature"),
    (20, "Music", "CPU usage"),
    (21, "Music", "GPU usage"),
    (22, "Music", "RAM usage"),
    (23, "Music", "Dancer"),
];

const GROUPS: [(&str, Kind); 4] = [
    ("Load", Kind::Load),
    ("Temperature", Kind::Temperature),
    ("Power", Kind::Power),
    ("Music", Kind::Music),
];

// Made-up GPUs, chips and PCI IDs (AGENTS.md rule 10).
const DEDICATED: &str = "0000:0a:00.0";
const INTEGRATED: &str = "0000:00:02.0";
const QUERY_FAILED: &str = "0000:05:00.0";
const NO_RULE: &str = "0000:07:00.0";

fn gpu(pci: &str, driver: &str, kind: GpuKind, ids: (u16, u16), has_rule: bool) -> Gpu {
    Gpu {
        pci: pci.to_owned(),
        driver: driver.to_owned(),
        kind,
        vendor: ids.0,
        device: ids.1,
        has_rule,
    }
}

/// In the service's order: dedicated, integrated, then unknown, each by address.
fn gpus() -> Vec<Gpu> {
    vec![
        gpu(
            DEDICATED,
            "contosogpu",
            GpuKind::Dedicated,
            (0x0a0a, 0x0001),
            true,
        ),
        gpu(
            INTEGRATED,
            "fabrikamgpu",
            GpuKind::Integrated,
            (0x0b0b, 0x0002),
            true,
        ),
        gpu(
            QUERY_FAILED,
            "contosogpu",
            GpuKind::QueryFailed,
            (0x0a0a, 0x0003),
            true,
        ),
        gpu(NO_RULE, "othergpu", GpuKind::Unknown, (0, 0), false),
    ]
}

/// The PCI ID list's names, as `pci_ids::names` gives them.
fn products() -> HashMap<(u16, u16), String> {
    HashMap::from([
        ((0x0a0a, 0x0001), "C870 [Contoso Graphics R870]".to_owned()),
        ((0x0b0b, 0x0002), "Fabrikam Display Engine".to_owned()),
    ])
}

const DEDICATED_LABEL: &str = "Dedicated · Contoso Graphics R870";
const INTEGRATED_LABEL: &str = "Integrated · Fabrikam Display Engine";
const QUERY_FAILED_LABEL: &str = "Unknown type · contosogpu · 0000:05:00.0";
const NO_RULE_LABEL: &str = "Unknown type · othergpu · 0000:07:00.0";

fn fan(input: u32, label: &str, rpm: FanSpeed) -> FanInput {
    FanInput {
        chip: "acme6000".to_owned(),
        device: "devices/platform/acme6000.2592".to_owned(),
        input,
        label: label.to_owned(),
        rpm,
    }
}

fn fans() -> Vec<FanInput> {
    vec![
        fan(1, "", FanSpeed::Rpm(1180)),
        fan(2, "", FanSpeed::Rpm(3400)),
        fan(3, "Rear", FanSpeed::Unavailable),
    ]
}

fn chosen(input: u32) -> Option<Input> {
    Some(Input {
        chip: "acme6000".to_owned(),
        device: "devices/platform/acme6000.2592".to_owned(),
        input,
    })
}

/// The "Normal, several GPUs" row's state: Temperature · CPU + GPU, everything there.
fn base() -> State {
    State {
        mode: Some(9),
        rotation: Some(0),
        temperature_unit: Some(TemperatureUnit::Celsius),
        cooler_connected: Some(true),
        cooler_state: Some(CoolerState::Connected),
        available_modes: Some((1..=23).collect()),
        unavailable_reasons: Some(HashMap::new()),
        gpus: Some(gpus()),
        chosen_gpu: Some(String::new()),
        gpu_in_use: Some(DEDICATED.to_owned()),
        fan_inputs: Some(fans()),
        cooler_fan_input: Some(chosen(1)),
        pump_input: Some(chosen(2)),
        cooler_fan_input_present: Some(true),
        pump_input_present: Some(true),
        cooler_fan_rpm: Some(FanSpeed::Rpm(1180)),
        pump_rpm: Some(FanSpeed::Rpm(3400)),
        music_peaks: Some(true),
        music_bar_fall: Some(FallStop::Moderate),
        music_peak_fall: Some(FallStop::Slow),
        audio_connected: Some(false),
        audio_wanted: Some(false),
    }
}

fn view_of(state: State) -> Normal {
    view_with(state, Ui::default())
}

fn view_with(state: State, ui: Ui) -> Normal {
    match view(&Service::Up(Box::new(state)), ui, &products()) {
        View::Normal(normal) => *normal,
        other => panic!("not the normal screens: {other:?}"),
    }
}

const CONNECTED: Status = Status {
    name: "Cooler status",
    text: "Cooler connected",
    dot: Dot::Good,
};
const RUNNING: Status = Status {
    name: "Service status",
    text: "Service running",
    dot: Dot::Good,
};

/// The tiles as the design's rules give them, for the saved mode and the reasons by mode,
/// already as the app writes them.
fn expected_groups(saved: u16, reasons: &[(u16, &str)]) -> Vec<Group> {
    let reasons: BTreeMap<u16, &str> = reasons.iter().copied().collect();
    GROUPS
        .iter()
        .map(|&(name, kind)| {
            let tiles: Vec<Tile> = TILES
                .iter()
                .filter(|(_, group, _)| *group == name)
                .map(|&(mode, group, caption)| {
                    let selected = mode == saved;
                    let reason = reasons.get(&mode);
                    Tile {
                        mode,
                        caption: caption.to_owned(),
                        label: format!("{group}: {caption}"),
                        selected,
                        sensitive: selected || reason.is_none(),
                        flag: reason.map(|_| {
                            if selected {
                                Flag::ReadingLost
                            } else {
                                Flag::Unavailable
                            }
                        }),
                        description: reason.map(|r| {
                            if selected {
                                format!("Reading lost: {r}")
                            } else {
                                (*r).to_owned()
                            }
                        }),
                    }
                })
                .collect();
            let mut group_reasons: Vec<String> = Vec::new();
            for tile in &tiles {
                if let Some(r) = reasons.get(&tile.mode)
                    && !group_reasons.iter().any(|g| g == r)
                {
                    group_reasons.push((*r).to_owned());
                }
            }
            Group {
                kind,
                reasons: group_reasons,
                tiles,
            }
        })
        .collect()
}

fn gpu_entries(order: &[(&str, &str, Option<&'static str>)]) -> Vec<GpuEntry> {
    std::iter::once(("", "Automatic", None))
        .chain(order.iter().copied())
        .map(|(pci, label, note)| GpuEntry {
            pci: pci.to_owned(),
            label: label.to_owned(),
            note,
        })
        .collect()
}

fn all_gpu_entries() -> Vec<GpuEntry> {
    gpu_entries(&[
        (DEDICATED, DEDICATED_LABEL, None),
        (INTEGRATED, INTEGRATED_LABEL, None),
        (
            QUERY_FAILED,
            QUERY_FAILED_LABEL,
            Some("driver couldn't be asked"),
        ),
        (NO_RULE, NO_RULE_LABEL, Some("no readings")),
    ])
}

fn input_entries(chosen_missing: Option<u32>) -> Vec<InputEntry> {
    let entry = |input: u32, label: &str, rpm: &str| InputEntry {
        input: chosen(input),
        label: label.to_owned(),
        rpm: if chosen_missing == Some(input) {
            "not found".to_owned()
        } else {
            rpm.to_owned()
        },
    };
    vec![
        InputEntry {
            input: None,
            label: "None".to_owned(),
            rpm: String::new(),
        },
        entry(1, "acme6000 · fan1", "1,180 RPM"),
        entry(2, "acme6000 · fan2", "3,400 RPM"),
        entry(3, "acme6000 · Rear", "no reading"),
    ]
}

fn input_row(chosen: Option<usize>, button: &str) -> InputRow {
    InputRow {
        entries: input_entries(None),
        chosen,
        button: button.to_owned(),
        sensitive: true,
        missing: false,
    }
}

/// The "Normal, several GPUs" row's whole view.
fn base_view() -> Normal {
    Normal {
        page: Page::Display,
        banner: None,
        column: Column {
            caption: "Temperature · CPU + GPU".to_owned(),
            line: "Shown on the cooler now",
            reason: None,
            status: vec![CONNECTED, RUNNING],
            fans: Speed::Rpm(1180),
            pump: Speed::Rpm(3400),
        },
        display: DisplayPage {
            unit: TemperatureUnit::Celsius,
            groups: expected_groups(9, &[]),
        },
        config: ConfigPage {
            rotation: 0,
            gpu: Some(GpuRow {
                entries: all_gpu_entries(),
                chosen: 0,
                in_use: Some(format!("Using {DEDICATED_LABEL}.")),
                hint: None,
                description: format!("Using {DEDICATED_LABEL}"),
            }),
            cooler_fans: input_row(Some(1), "acme6000 · fan1"),
            pump: input_row(Some(2), "acme6000 · fan2"),
            inputs_hint: INPUTS_HINT,
            peaks: true,
            bar_fall: FallStop::Moderate,
            peak_fall: FallStop::Slow,
        },
    }
}

// Row "Normal, several GPUs": the Display page, the cooler connected, the service running,
// the fan and pump RPM; the GPU on Automatic, naming the GPU it reads.
#[test]
fn normal_with_several_gpus() {
    assert_eq!(view_of(base()), base_view());
    let mut config = base_view();
    config.page = Page::Configuration;
    assert_eq!(
        view_with(
            base(),
            Ui {
                page: Page::Configuration,
                ..Ui::default()
            }
        ),
        config
    );
}

// Row "GPU list open": Automatic first, a GPU whose query failed as "Unknown type · …" with its
// note, one without a reading rule last with "no readings"; the line names the integrated GPU
// Automatic reads.
#[test]
fn the_gpu_list() {
    let mut state = base();
    // The service's order has the GPU without a rule before the one whose query failed; the
    // list moves it last.
    state.gpus = Some(vec![
        gpus()[1].clone(),
        gpu(NO_RULE, "othergpu", GpuKind::Unknown, (0, 0), false),
        gpus()[2].clone(),
    ]);
    state.gpu_in_use = Some(INTEGRATED.to_owned());
    let mut want = base_view();
    want.config.gpu = Some(GpuRow {
        entries: gpu_entries(&[
            (INTEGRATED, INTEGRATED_LABEL, None),
            (
                QUERY_FAILED,
                QUERY_FAILED_LABEL,
                Some("driver couldn't be asked"),
            ),
            (NO_RULE, NO_RULE_LABEL, Some("no readings")),
        ]),
        chosen: 0,
        in_use: Some(format!("Using {INTEGRATED_LABEL}.")),
        hint: None,
        description: format!("Using {INTEGRATED_LABEL}"),
    });
    assert_eq!(view_of(state), want);
}

// Row "A GPU added or removed since the last start": the service cleared the choice, so the
// dropdown is on Automatic with no note, and the line names the GPU in use.
#[test]
fn a_gpu_added_or_removed() {
    let mut state = base();
    state.gpus = Some(gpus()[..3].to_vec());
    let mut want = base_view();
    want.config.gpu = Some(GpuRow {
        entries: gpu_entries(&[
            (DEDICATED, DEDICATED_LABEL, None),
            (INTEGRATED, INTEGRATED_LABEL, None),
            (
                QUERY_FAILED,
                QUERY_FAILED_LABEL,
                Some("driver couldn't be asked"),
            ),
        ]),
        ..base_view().config.gpu.expect("a GPU row")
    });
    assert_eq!(view_of(state), want);
}

// A GPU chosen by hand: named on the button, so no line under it.
#[test]
fn a_gpu_chosen_by_hand() {
    let mut state = base();
    state.chosen_gpu = Some(QUERY_FAILED.to_owned());
    state.gpu_in_use = Some(QUERY_FAILED.to_owned());
    let mut want = base_view();
    want.config.gpu = Some(GpuRow {
        entries: all_gpu_entries(),
        chosen: 3,
        in_use: None,
        hint: None,
        description: "driver couldn't be asked".to_owned(),
    });
    assert_eq!(view_of(state), want);
}

// Row "A Music mode": the status adds the sound, connected or not.
#[test]
fn a_music_mode() {
    for (mode, connected, caption, sound) in [
        (13, true, "Music · Spectrum", ("Sound connected", Dot::Good)),
        (
            23,
            false,
            "Music · Dancer",
            ("No sound: spectrum is silent", Dot::Warn),
        ),
        (
            21,
            true,
            "Music · GPU usage",
            ("Sound connected", Dot::Good),
        ),
    ] {
        let mut state = base();
        state.mode = Some(mode);
        state.audio_connected = Some(connected);
        let mut want = base_view();
        want.column.caption = caption.to_owned();
        want.column.status.push(Status {
            name: "Music sound status",
            text: sound.0,
            dot: sound.1,
        });
        want.display.groups = expected_groups(mode as u16, &[]);
        assert_eq!(view_of(state), want, "mode {mode}");
    }
}

// Row "A value of 100 or more, °F": the preview mirrors the service's frames, and the tiles
// draw in °F; the view carries the unit.
#[test]
fn fahrenheit() {
    let mut state = base();
    state.temperature_unit = Some(TemperatureUnit::Fahrenheit);
    let mut want = base_view();
    want.display.unit = TemperatureUnit::Fahrenheit;
    assert_eq!(view_of(state), want);
}

// Row "Fan picker open": None first, then every input with its chip, label and live RPM, "no
// reading" where its speed can't be read.
#[test]
fn the_fan_picker() {
    let row = view_of(base()).config.cooler_fans;
    assert_eq!(row.entries, input_entries(None));
    assert_eq!(row.chosen, Some(1));
}

// Row "First start": Load · CPU, 0°, °C, no inputs ("Choose input"), peak dots on, Moderate /
// Slow, the GPU on Automatic.
#[test]
fn first_start() {
    let mut state = base();
    state.mode = Some(1);
    state.cooler_fan_input = Some(None);
    state.pump_input = Some(None);
    state.cooler_fan_rpm = Some(FanSpeed::Unavailable);
    state.pump_rpm = Some(FanSpeed::Unavailable);
    let mut want = base_view();
    want.column.caption = "Load · CPU".to_owned();
    want.column.fans = Speed::ChooseInput;
    want.column.pump = Speed::ChooseInput;
    want.display.groups = expected_groups(1, &[]);
    want.config.cooler_fans = input_row(Some(0), "None");
    want.config.pump = input_row(Some(0), "None");
    assert_eq!(view_of(state), want);
    assert_eq!(Speed::ChooseInput.text(), "Choose input");
}

/// The base view with the cooler shown as `status`, `line` and `banner`.
fn cooler_view(status: (&'static str, Dot), line: &'static str, banner: Option<Banner>) -> Normal {
    let mut want = base_view();
    want.banner = banner;
    want.column.line = line;
    want.column.status[0] = Status {
        name: "Cooler status",
        text: status.0,
        dot: status.1,
    };
    want
}

// Row "Cooler disconnected (`not-found`)": its banner and status; everything still changeable.
#[test]
fn cooler_not_found() {
    let mut state = base();
    state.cooler_state = Some(CoolerState::NotFound);
    assert_eq!(
        view_of(state),
        cooler_view(
            ("Cooler not connected", Dot::Warn),
            "Not on the cooler: it isn't connected",
            Some(Banner::NotConnected)
        )
    );
}

// Row "Cooler found, no access (`no-access`)": the error banner with the fix.
#[test]
fn cooler_no_access() {
    let mut state = base();
    state.cooler_state = Some(CoolerState::NoAccess);
    assert_eq!(
        view_of(state),
        cooler_view(
            ("Cooler: no access", Dot::Bad),
            "Not on the cooler: the service can't open it",
            Some(Banner::NoAccess)
        )
    );
    assert!(Banner::NoAccess.is_error());
    assert_eq!(
        Banner::NoAccess.text().0,
        "The cooler is plugged in, but the service can't open it."
    );
}

// Row "Sending to the cooler fails, 2 s or more": the warning banner.
#[test]
fn cooler_sending_fails_for_2_s() {
    let mut state = base();
    state.cooler_state = Some(CoolerState::WriteFailed);
    let ui = Ui {
        sending_failed_long: true,
        ..Ui::default()
    };
    assert_eq!(
        view_with(state, ui),
        cooler_view(
            ("Cooler: sending fails", Dot::Warn),
            "Not on the cooler: sending it fails",
            Some(Banner::SendingFails)
        )
    );
    assert!(!Banner::SendingFails.is_error());
}

// Row "Sending fails, under 2 s": as connected, no banner.
#[test]
fn cooler_sending_fails_under_2_s() {
    let mut state = base();
    state.cooler_state = Some(CoolerState::WriteFailed);
    assert_eq!(view_of(state), base_view());
}

// The 2 s on a fake clock: the wait starts when the state becomes `write-failed`, a timer is
// asked for once, and any other state ends it.
#[test]
fn the_sending_fails_wait_on_a_fake_clock() {
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut wait = SendingFails::default();
    assert_eq!(wait.follow(Some(CoolerState::Connected), at(0)), None);
    assert!(!wait.lasted(at(5000)));
    assert_eq!(
        wait.follow(Some(CoolerState::WriteFailed), at(100)),
        Some(SENDING_FAILS_AFTER)
    );
    // Still `write-failed` in a later StateChanged: the same wait.
    assert_eq!(wait.follow(Some(CoolerState::WriteFailed), at(900)), None);
    assert!(!wait.lasted(at(2099)));
    assert!(wait.lasted(at(2100)));
    // One bad write, then connected: the wait ends.
    assert_eq!(wait.follow(Some(CoolerState::Connected), at(2200)), None);
    assert!(!wait.lasted(at(9000)));
    // Unplugged: `write-failed` for a tick, then `not-found`; no banner of its own.
    assert!(
        wait.follow(Some(CoolerState::WriteFailed), at(10_000))
            .is_some()
    );
    assert_eq!(wait.follow(Some(CoolerState::NotFound), at(10_100)), None);
    assert!(!wait.lasted(at(12_100)));
    // A clock that seems to go back never reads as lasted.
    assert!(
        wait.follow(Some(CoolerState::WriteFailed), at(20_000))
            .is_some()
    );
    assert!(!wait.lasted(at(19_000)));
}

// Row "Service not running": the column says so, and nothing else is shown.
#[test]
fn service_not_running() {
    assert_eq!(
        view(&Service::Down, Ui::default(), &products()),
        View::ServiceDown(Column {
            caption: "No picture".to_owned(),
            line: "The service draws the picture",
            reason: None,
            status: vec![
                Status {
                    name: "Cooler status",
                    text: "Cooler: unknown",
                    dot: Dot::Off,
                },
                Status {
                    name: "Service status",
                    text: "Service not running",
                    dot: Dot::Bad,
                },
            ],
            fans: Speed::Unknown,
            pump: Speed::Unknown,
        })
    );
    assert_eq!(Speed::Unknown.text(), "—");
    assert_eq!(
        SERVICE_BANNER[2],
        "sudo systemctl start opendeepcool.service"
    );
}

/// The service's reasons for every GPU mode, as it writes them (Phase 7's `availability`).
fn gpu_reasons(reason: &str) -> HashMap<u32, String> {
    [
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
    .map(|(mode, metric)| (mode, format!("{metric}: {reason}")))
    .collect()
}

/// The same, as the app shows them.
fn gpu_reasons_shown(reason: &str) -> Vec<(u16, String)> {
    [
        (2, "GPU usage"),
        (4, "GPU usage"),
        (6, "GPU usage"),
        (8, "GPU temperature"),
        (9, "GPU temperature"),
        (11, "GPU power"),
        (12, "GPU power"),
        (19, "GPU temperature"),
        (21, "GPU usage"),
    ]
    .into_iter()
    .map(|(mode, metric)| (mode, format!("{metric}: {reason}")))
    .collect()
}

fn borrowed(reasons: &[(u16, String)]) -> Vec<(u16, &str)> {
    reasons.iter().map(|(m, r)| (*m, r.as_str())).collect()
}

// Row "Metric unavailable": the tiles disabled with "Unavailable", each group's reasons once.
#[test]
fn a_metric_unavailable() {
    let mut state = base();
    state.mode = Some(7);
    state.available_modes = Some(vec![1, 3, 5, 7, 10, 13, 14, 15, 16, 17, 18, 20, 22, 23]);
    state.unavailable_reasons = Some(gpu_reasons("no device found"));
    let shown = gpu_reasons_shown("no device found");
    let mut want = base_view();
    want.column.caption = "Temperature · CPU".to_owned();
    want.display.groups = expected_groups(7, &borrowed(&shown));
    let got = view_of(state);
    assert_eq!(got, want);
    // Each group lists each of its reasons once.
    let reasons: Vec<&Vec<String>> = got.display.groups.iter().map(|g| &g.reasons).collect();
    assert_eq!(*reasons[0], ["GPU usage: no device found"]);
    assert_eq!(*reasons[1], ["GPU temperature: no device found"]);
    assert_eq!(*reasons[2], ["GPU power: no device found"]);
    assert_eq!(
        *reasons[3],
        [
            "GPU temperature: no device found",
            "GPU usage: no device found"
        ]
    );
    let tile = &got.display.groups[0].tiles[1];
    assert_eq!(
        (tile.sensitive, tile.flag, tile.description.as_deref()),
        (
            false,
            Some(Flag::Unavailable),
            Some("GPU usage: no device found")
        )
    );
}

// Row "The saved mode lost a reading": its tile stays selected and sensitive, "Reading lost";
// the reason under the preview.
#[test]
fn the_saved_mode_lost_a_reading() {
    let mut state = base();
    state.available_modes = Some(vec![1, 3, 5, 7, 10, 13, 14, 15, 16, 17, 18, 20, 22, 23]);
    state.unavailable_reasons = Some(gpu_reasons("reading it failed"));
    let shown = gpu_reasons_shown("reading it failed");
    let mut want = base_view();
    want.column.reason = Some(
        "GPU temperature: reading it failed. The cooler shows dashes until it's back.".to_owned(),
    );
    want.display.groups = expected_groups(9, &borrowed(&shown));
    let got = view_of(state);
    assert_eq!(got, want);
    let tile = &got.display.groups[1].tiles[2];
    assert_eq!(
        (
            tile.mode,
            tile.selected,
            tile.sensitive,
            tile.flag,
            tile.description.as_deref()
        ),
        (
            9,
            true,
            true,
            Some(Flag::ReadingLost),
            Some("Reading lost: GPU temperature: reading it failed")
        )
    );
}

// Row "Not available in this session": only the message.
#[test]
fn not_available_in_this_session() {
    assert_eq!(
        view(&Service::Refused, Ui::default(), &products()),
        View::NotAvailable
    );
}

// Row "One GPU": no GPU row at all; nor with none.
#[test]
fn one_gpu() {
    for list in [gpus()[..1].to_vec(), Vec::new()] {
        let mut state = base();
        state.gpus = Some(list);
        let mut want = base_view();
        want.config.gpu = None;
        assert_eq!(view_of(state), want);
    }
}

// Row "Saved fan or pump input missing": "— not found" on its picker with the warning, "Input
// not found" under the preview, and no other input's RPM in its place.
#[test]
fn a_saved_input_missing() {
    let mut state = base();
    // The cooler fans' input was found at start and has gone; the pump's wasn't found at all.
    state.cooler_fan_input_present = Some(false);
    state.cooler_fan_rpm = Some(FanSpeed::Unavailable);
    state.pump_input = Some(chosen(9));
    state.pump_input_present = Some(false);
    state.pump_rpm = Some(FanSpeed::Unavailable);
    let mut want = base_view();
    want.column.fans = Speed::NotFound;
    want.column.pump = Speed::NotFound;
    want.config.cooler_fans = InputRow {
        entries: input_entries(Some(1)),
        chosen: Some(1),
        button: "acme6000 · fan1 — not found".to_owned(),
        sensitive: true,
        missing: true,
    };
    let mut entries = input_entries(None);
    entries.push(InputEntry {
        input: chosen(9),
        label: "acme6000 · fan9".to_owned(),
        rpm: "not found".to_owned(),
    });
    want.config.pump = InputRow {
        entries,
        chosen: Some(4),
        button: "acme6000 · fan9 — not found".to_owned(),
        sensitive: true,
        missing: true,
    };
    assert_eq!(view_of(state), want);
    assert_eq!(Speed::NotFound.text(), "Input not found");
}

// An input that's there but can't be read says so, and never shows another's speed (D34).
#[test]
fn an_input_without_a_reading() {
    let mut state = base();
    state.cooler_fan_input = Some(chosen(3));
    state.cooler_fan_rpm = Some(FanSpeed::Unavailable);
    let mut want = base_view();
    want.column.fans = Speed::NoReading;
    want.config.cooler_fans = input_row(Some(3), "acme6000 · Rear");
    assert_eq!(view_of(state), want);
    assert_eq!(Speed::NoReading.text(), "No reading");
}

// Row "No fan sensors": both pickers insensitive with the note; the column says so.
#[test]
fn no_fan_sensors() {
    let mut state = base();
    state.fan_inputs = Some(Vec::new());
    state.cooler_fan_input = Some(None);
    state.pump_input = Some(chosen(2));
    state.pump_input_present = Some(false);
    let mut want = base_view();
    want.column.fans = Speed::NoSensors;
    want.column.pump = Speed::NoSensors;
    let none = InputRow {
        entries: Vec::new(),
        chosen: None,
        button: "No fan sensors found".to_owned(),
        sensitive: false,
        missing: false,
    };
    want.config.cooler_fans = none.clone();
    want.config.pump = none;
    want.config.inputs_hint = NO_SENSORS_HINT;
    assert_eq!(view_of(state), want);
}

// Row "A GPU without a reading rule in use": the GPU tiles disabled with the service's reason,
// and the hint under the dropdown.
#[test]
fn a_gpu_without_a_reading_rule_in_use() {
    let mut state = base();
    state.chosen_gpu = Some(NO_RULE.to_owned());
    state.gpu_in_use = Some(NO_RULE.to_owned());
    state.mode = Some(1);
    state.unavailable_reasons = Some(gpu_reasons("no reading for this driver"));
    let shown = gpu_reasons_shown("no reading for this driver");
    let mut want = base_view();
    want.column.caption = "Load · CPU".to_owned();
    want.display.groups = expected_groups(1, &borrowed(&shown));
    want.config.gpu = Some(GpuRow {
        entries: all_gpu_entries(),
        chosen: 4,
        in_use: None,
        hint: Some(NO_GPU_RULE_HINT),
        description: "no readings".to_owned(),
    });
    assert_eq!(view_of(state.clone()), want);
    // Automatic reading it: both the line and the hint.
    state.chosen_gpu = Some(String::new());
    let gpu = view_of(state).config.gpu.expect("a GPU row");
    assert_eq!(gpu.in_use, Some(format!("Using {NO_RULE_LABEL}.")));
    assert_eq!(gpu.hint, Some(NO_GPU_RULE_HINT));
    assert_eq!(
        gpu.description,
        format!("Using {NO_RULE_LABEL}; no readings")
    );
}

// Row "Another `CoolerState`": `released`, and a value the app couldn't read (left out of its
// state, decisions log D33), show as `not-found`.
#[test]
fn another_cooler_state() {
    for state_value in [Some(CoolerState::Released), None] {
        let mut state = base();
        state.cooler_state = state_value;
        assert_eq!(
            view_of(state),
            cooler_view(
                ("Cooler not connected", Dot::Warn),
                "Not on the cooler: it isn't connected",
                Some(Banner::NotConnected)
            ),
            "{state_value:?}"
        );
    }
}

// Row "Short read failure, first second": the service sends no reason then, so nothing is
// disabled; the preview mirrors the cooler's dashes.
#[test]
fn a_short_read_failure() {
    let mut state = base();
    state.unavailable_reasons = Some(HashMap::new());
    assert_eq!(view_of(state), base_view());
}

// Rows "A change refused" and "Back from standby": no state of their own. A refused change
// leaves the service's state as it was, so the controls show the saved value; after standby
// the same state gives the same view. The theme rows are the theme's tests.
#[test]
fn rows_without_a_state_of_their_own() {
    let before = view_of(base());
    // The app asked for mode 4; the service refused it, and sent nothing.
    assert_eq!(view_of(base()), before);
    assert!(before.display.groups[1].tiles[2].selected);
}

// Every reason the service writes reads with the display's metric names.
#[test]
fn reasons_name_metrics_as_the_display_does() {
    for metric in Metric::ALL {
        let reason = format!("{}: no device found", metric.spec_name());
        assert_eq!(
            reason_text(&reason),
            format!("{}: no device found", metric_name(metric))
        );
    }
    assert_eq!(
        reason_text("comes in a later version"),
        "comes in a later version"
    );
    assert_eq!(reason_text("Something: else"), "Something: else");
}

// Identical GPUs get their addresses, so each entry can be told apart (D36).
#[test]
fn identical_gpus_get_their_addresses() {
    let mut state = base();
    let mut twin = gpus()[0].clone();
    twin.pci = "0000:0b:00.0".to_owned();
    state.gpus = Some(vec![gpus()[0].clone(), twin]);
    let gpu = view_of(state).config.gpu.expect("a GPU row");
    let labels: Vec<&str> = gpu.entries.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Automatic",
            "Dedicated · Contoso Graphics R870 · 0000:0a:00.0",
            "Dedicated · Contoso Graphics R870 · 0000:0b:00.0"
        ]
    );
}

#[test]
fn speeds_use_english_digit_grouping() {
    for (rpm, text) in [
        (0, "0 RPM"),
        (999, "999 RPM"),
        (1000, "1,000 RPM"),
        (1180, "1,180 RPM"),
        (9999, "9,999 RPM"),
    ] {
        assert_eq!(Speed::Rpm(rpm).text(), text);
    }
}

/// Every text of a view.
fn texts(view: &View) -> Vec<String> {
    let mut out = Vec::new();
    let mut column = |c: &Column| {
        out.push(c.caption.clone());
        out.push(c.line.to_owned());
        out.extend(c.reason.clone());
        out.extend(c.status.iter().map(|s| s.text.to_owned()));
        out.push(c.fans.text());
        out.push(c.pump.text());
    };
    match view {
        View::NotAvailable => {
            return vec![
                NOT_AVAILABLE_HEADING.to_owned(),
                NOT_AVAILABLE_TEXT.to_owned(),
            ];
        }
        View::ServiceDown(c) => {
            column(c);
            return out;
        }
        View::Normal(n) => {
            column(&n.column);
            if let Some(b) = n.banner {
                out.push(b.text().0.to_owned());
                out.push(b.text().1.to_owned());
            }
            for g in &n.display.groups {
                out.extend(g.reasons.iter().cloned());
                for t in &g.tiles {
                    out.push(t.caption.clone());
                    out.push(t.label.clone());
                    out.extend(t.description.clone());
                    out.extend(t.flag.map(|f| f.text().to_owned()));
                }
            }
            if let Some(gpu) = &n.config.gpu {
                for e in &gpu.entries {
                    out.push(e.label.clone());
                    out.extend(e.note.map(str::to_owned));
                }
                out.extend(gpu.in_use.clone());
                out.extend(gpu.hint.map(str::to_owned));
                out.push(gpu.description.clone());
            }
            for row in [&n.config.cooler_fans, &n.config.pump] {
                out.push(row.button.clone());
                for e in &row.entries {
                    out.push(e.label.clone());
                    out.push(e.rpm.clone());
                }
            }
            out.push(n.config.inputs_hint.to_owned());
        }
    }
    out
}

/// The numbers `text` shows as RPM.
fn rpms_in(text: &str) -> Vec<u16> {
    let mut found = Vec::new();
    for (end, _) in text.match_indices(" RPM") {
        let start = text[..end]
            .rfind(|c: char| !(c.is_ascii_digit() || c == ','))
            .map_or(0, |i| i + 1);
        let digits: String = text[start..end].chars().filter(|c| *c != ',').collect();
        found.push(digits.parse().expect("an RPM figure"));
    }
    found
}

fn speed() -> impl Strategy<Value = FanSpeed> {
    prop_oneof![
        (0u16..=9999).prop_map(FanSpeed::Rpm),
        Just(FanSpeed::Unavailable)
    ]
}

fn cooler_state() -> impl Strategy<Value = Option<CoolerState>> {
    prop::option::of(prop_oneof![
        Just(CoolerState::Connected),
        Just(CoolerState::NotFound),
        Just(CoolerState::NoAccess),
        Just(CoolerState::WriteFailed),
        Just(CoolerState::Released),
    ])
}

fn gpu_kind() -> impl Strategy<Value = GpuKind> {
    prop_oneof![
        Just(GpuKind::Dedicated),
        Just(GpuKind::Integrated),
        Just(GpuKind::Unknown),
        Just(GpuKind::QueryFailed),
    ]
}

const SERVICE_REASONS: [&str; 4] = [
    "no reading for this driver",
    "no device found",
    "no permission to read it",
    "the GPU driver was updated: restart the PC",
];

prop_compose! {
    /// Any state the service could send: the reasons only for modes with metrics, each naming
    /// one of its mode's metrics, as the service's `availability` does.
    fn any_state()(
        mode in 1u32..=23,
        rotation in 0u8..=3,
        fahrenheit: bool,
        cooler in cooler_state(),
        reasons in prop::collection::btree_map(1u16..=23, (0usize..2, 0usize..4), 0..12),
        gpu_list in prop::collection::vec((gpu_kind(), any::<bool>(), 0usize..3), 0..4),
        choice in (prop::option::of(0usize..4), prop::option::of(0usize..4)),
        fan_list in prop::collection::vec(speed(), 0..4),
        inputs in (prop::option::of(1u32..6), prop::option::of(1u32..6), any::<bool>(), any::<bool>()),
        rpms in (prop::option::of(speed()), prop::option::of(speed())),
        music in (any::<bool>(), 0usize..5, 0usize..5, any::<bool>()),
    ) -> State {
        let stops = [FallStop::Slower, FallStop::Slow, FallStop::Moderate, FallStop::Fast, FallStop::Faster];
        let unavailable: HashMap<u32, String> = reasons
            .into_iter()
            .filter_map(|(m, (metric, reason))| {
                let metrics = mode_info(Mode::new(m)?).metrics;
                let metric = metrics.get(metric % metrics.len().max(1))?;
                Some((u32::from(m), format!("{}: {}", metric.spec_name(), SERVICE_REASONS[reason])))
            })
            .collect();
        let gpus: Vec<Gpu> = gpu_list
            .iter()
            .enumerate()
            .map(|(i, (kind, has_rule, name))| {
                let ids = [(0x0a0a, 0x0001), (0x0b0b, 0x0002), (0, 0)][*name];
                gpu(&format!("0000:0{i}:00.0"), "contosogpu", *kind, ids, *has_rule)
            })
            .collect();
        let pci = |i: Option<usize>| i.and_then(|i| gpus.get(i)).map(|g| g.pci.clone()).unwrap_or_default();
        State {
            mode: Some(mode),
            rotation: Some(rotation),
            temperature_unit: Some(if fahrenheit { TemperatureUnit::Fahrenheit } else { TemperatureUnit::Celsius }),
            cooler_connected: Some(cooler == Some(CoolerState::Connected)),
            cooler_state: cooler,
            available_modes: Some((1..=23).filter(|m| !unavailable.contains_key(m)).collect()),
            unavailable_reasons: Some(unavailable),
            chosen_gpu: Some(pci(choice.0)),
            gpu_in_use: Some(pci(choice.1)),
            gpus: Some(gpus),
            fan_inputs: Some(fan_list.iter().enumerate().map(|(i, rpm)| fan(i as u32 + 1, "", *rpm)).collect()),
            cooler_fan_input: Some(inputs.0.and_then(chosen)),
            pump_input: Some(inputs.1.and_then(chosen)),
            cooler_fan_input_present: Some(inputs.2),
            pump_input_present: Some(inputs.3),
            cooler_fan_rpm: rpms.0,
            pump_rpm: rpms.1,
            music_peaks: Some(music.0),
            music_bar_fall: Some(stops[music.1]),
            music_peak_fall: Some(stops[music.2]),
            audio_connected: Some(music.3),
            audio_wanted: Some(music.3),
        }
    }
}

/// The devices a text names.
fn devices(text: &str) -> Vec<&'static str> {
    ["CPU", "GPU", "RAM"]
        .into_iter()
        .filter(|d| text.contains(d))
        .collect()
}

proptest! {
    // Exactly one tile per mode 1–23, in four groups, in order; and exactly one selected.
    #[test]
    fn one_tile_per_mode(state in any_state(), long: bool) {
        let ui = Ui { sending_failed_long: long, ..Ui::default() };
        let View::Normal(normal) = view(&Service::Up(Box::new(state.clone())), ui, &products()) else {
            panic!("not the normal screens");
        };
        let modes: Vec<u16> = normal.display.groups.iter().flat_map(|g| g.tiles.iter().map(|t| t.mode)).collect();
        prop_assert_eq!(modes, (1..=23).collect::<Vec<u16>>());
        let kinds: Vec<Kind> = normal.display.groups.iter().map(|g| g.kind).collect();
        prop_assert_eq!(kinds, GROUPS.map(|(_, k)| k).to_vec());
        let selected: Vec<u16> = normal.display.groups.iter().flat_map(|g| &g.tiles).filter(|t| t.selected).map(|t| t.mode).collect();
        prop_assert_eq!(selected, vec![state.mode.unwrap() as u16]);
    }

    // Rule 11: a tile names exactly the devices its mode reads, and its reason names a metric
    // its mode shows; a CPU tile never maps to a GPU mode.
    #[test]
    fn every_tile_shows_its_own_metrics(state in any_state()) {
        let View::Normal(normal) = view(&Service::Up(Box::new(state)), Ui::default(), &products()) else {
            panic!("not the normal screens");
        };
        for tile in normal.display.groups.iter().flat_map(|g| &g.tiles) {
            let metrics = mode_info(Mode::new(tile.mode).unwrap()).metrics;
            let mut want: Vec<&str> = metrics.iter().map(|&m| device(m)).collect();
            want.sort_unstable();
            want.dedup();
            let mut named = devices(&tile.caption);
            named.sort_unstable();
            prop_assert_eq!(&named, &want, "mode {}: {}", tile.mode, &tile.caption);
            if let Some(description) = &tile.description {
                prop_assert!(
                    metrics.iter().any(|&m| description.contains(metric_name(m))),
                    "mode {}: {}", tile.mode, description
                );
            }
        }
    }

    // No view shows a hardware value but the cooler's fan and pump RPM: every RPM figure is one
    // of the fan speeds the service sent, and nothing shows a %, a temperature or a power.
    #[test]
    fn only_the_cooler_s_rpm_is_shown(state in any_state(), long: bool, down: bool) {
        let mut sent: Vec<u16> = state.fan_inputs.iter().flatten().filter_map(|f| match f.rpm {
            FanSpeed::Rpm(r) => Some(r),
            FanSpeed::Unavailable => None,
        }).collect();
        for speed in [state.cooler_fan_rpm, state.pump_rpm].into_iter().flatten() {
            if let FanSpeed::Rpm(r) = speed {
                sent.push(r);
            }
        }
        let service = if down { Service::Down } else { Service::Up(Box::new(state)) };
        let ui = Ui { sending_failed_long: long, ..Ui::default() };
        for text in texts(&view(&service, ui, &products())) {
            prop_assert!(!text.contains('%') && !text.contains('°'), "{}", text);
            for rpm in rpms_in(&text) {
                prop_assert!(sent.contains(&rpm), "{} isn't a speed the service sent: {}", rpm, text);
            }
        }
    }
}
