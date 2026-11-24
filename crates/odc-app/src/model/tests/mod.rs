//! The view model, one test per row of the design's States table, each asserting the whole
//! `View`; the `write-failed` wait on a fake clock; and the properties of every view.

use std::collections::{BTreeMap, HashMap};

use proptest::prelude::*;

use super::*;

use crate::boot::{Facts, Known, Run, UnitFile};

use crate::live::sample_values;

use odc_render::Stop;

/// The Start at boot switch fully on: all four facts hold.
fn startup_on() -> Boot {
    Boot {
        known: Known::Facts(Facts {
            service_file: UnitFile::Enabled,
            service: Run::Running,
            sound_file: UnitFile::Enabled,
            sound: true,
        }),
        ..Boot::default()
    }
}

/// Its switch on the Startup card: on, sensitive, with no note.
const STARTUP_ON: boot::Switch = boot::Switch {
    handle: true,
    trough: true,
    sensitive: true,
    note: None,
};

/// The design's tile table (Display page), typed here apart from the model's own.
const TILES: [(u16, &str, &str); 25] = [
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
    // The Music group stands in three rows of five (owner, 2026-11-23).
    (13, "Music", "Spectrum"),
    (24, "Music", "Lub-dub"),
    (25, "Music", "Coffee break"),
    (26, "Music", "Cat tail"),
    (23, "Music", "Dancer"),
    (20, "Music", "CPU load"),
    (21, "Music", "GPU load"),
    (22, "Music", "RAM load"),
    (18, "Music", "CPU temperature"),
    (19, "Music", "GPU temperature"),
    (14, "Music", "Lub-dub + Spectrum"),
    (15, "Music", "Coffee break + Spectrum"),
    (16, "Music", "Cat tail + Spectrum"),
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

/// The "Normal, several GPUs" row's state: Temperature · CPU + GPU, everything there.
fn base() -> State {
    State {
        mode: Some(9),
        rotation: Some(0),
        temperature_unit: Some(TemperatureUnit::Celsius),
        cooler_connected: Some(true),
        cooler_state: Some(CoolerState::Connected),
        available_modes: Some(Mode::all().map(|m| u32::from(m.number())).collect()),
        unavailable_reasons: Some(HashMap::new()),
        gpus: Some(gpus()),
        chosen_gpu: Some(String::new()),
        gpu_in_use: Some(DEDICATED.to_owned()),
        music_peaks: Some(true),
        music_bar_fall: Some(FallStop::Moderate),
        music_peak_fall: Some(FallStop::Slow),
        readings: Some(HashMap::new()),
        use_system_theme: Some(true),
        dark_theme: Some(true),
        window_opacity: Some(odc_ipc::WindowOpacity::Opaque),
        window_corners: Some(odc_ipc::WindowCorners::Rounded),
        audio_connected: Some(false),
        audio_wanted: Some(false),
    }
}

fn view_of(state: State) -> Normal {
    view_with(state, Ui::default())
}

fn view_with(state: State, ui: Ui) -> Normal {
    match view(
        &Service::Up(Box::new(state)),
        ui,
        &products(),
        &startup_on(),
    ) {
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

/// The "Normal, several GPUs" row's whole view.
fn base_view() -> Normal {
    Normal {
        page: Page::Display,
        banner: None,
        column: Column {
            caption: "Temperature · CPU + GPU".to_owned(),
            away: None,
            reason: None,
            status: vec![CONNECTED, RUNNING],
        },
        display: DisplayPage {
            unit: TemperatureUnit::Celsius,
            groups: expected_groups(9, &[]),
            live: LiveInput {
                // The base state's `Readings` is empty: no metric has a value.
                values: Values::new(),
                music: MusicSettings {
                    peaks: true,
                    bar_fall: Stop::Moderate,
                    peak_fall: Stop::Slow,
                },
                sound: false,
                restart: (9, TemperatureUnit::Celsius, DEDICATED.to_owned()),
            },
        },
        config: ConfigPage {
            // No Omarchy theme and no preference: no system theme can be found (state c).
            theme: ThemeCard {
                use_system: false,
                use_system_sensitive: false,
                dark: true,
                toggles_sensitive: true,
                // The service's defaults, which the state here doesn't override.
                opacity: WindowOpacity::Opaque,
                corners: WindowCorners::Rounded,
                line: ThemeLine::NotFound,
            },
            rotation: 0,
            unit: TemperatureUnit::Celsius,
            gpu: Some(GpuRow {
                entries: all_gpu_entries(),
                chosen: 0,
                in_use: Some(format!("Using {DEDICATED_LABEL}.")),
                hint: None,
                description: format!("Using {DEDICATED_LABEL}"),
            }),
            peaks: true,
            bar_fall: FallStop::Moderate,
            peak_fall: FallStop::Slow,
        },
        startup: STARTUP_ON,
    }
}

/// The base view with the cooler shown as `status`, `away` (why the picture isn't on the
/// cooler, in the preview's description) and `banner`.
fn cooler_view(status: (&'static str, Dot), away: &'static str, banner: Option<Banner>) -> Normal {
    let mut want = base_view();
    want.banner = banner;
    want.column.away = Some(away);
    want.column.status[0] = Status {
        name: "Cooler status",
        text: status.0,
        dot: status.1,
    };
    want
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
        (2, "GPU load"),
        (4, "GPU load"),
        (6, "GPU load"),
        (8, "GPU temperature"),
        (9, "GPU temperature"),
        (11, "GPU power"),
        (12, "GPU power"),
        (19, "GPU temperature"),
        (21, "GPU load"),
    ]
    .into_iter()
    .map(|(mode, metric)| (mode, format!("{metric}: {reason}")))
    .collect()
}

fn borrowed(reasons: &[(u16, String)]) -> Vec<(u16, &str)> {
    reasons.iter().map(|(m, r)| (*m, r.as_str())).collect()
}

/// Every text of a view.
/// The switch's note.
fn switch_texts(switch: &boot::Switch) -> Vec<String> {
    switch.note.iter().map(|n| n.text.clone()).collect()
}

/// Some of the switch's states: not read, a failed read, on, a new install, partly on, flipping.
fn startup_states() -> Vec<Boot> {
    let facts = |service_file, service, sound_file, sound| {
        Known::Facts(Facts {
            service_file,
            service,
            sound_file,
            sound,
        })
    };
    let mut flipping = Boot {
        known: facts(UnitFile::Disabled, Run::Stopped, UnitFile::Disabled, false),
        ..Boot::default()
    };
    flipping.begin(boot::Direction::On);
    vec![
        Boot::default(),
        Boot {
            known: Known::Failed,
            ..Boot::default()
        },
        startup_on(),
        Boot {
            known: facts(UnitFile::Disabled, Run::Stopped, UnitFile::Disabled, false),
            ..Boot::default()
        },
        Boot {
            known: facts(UnitFile::Disabled, Run::Running, UnitFile::Enabled, false),
            may_flip: false,
            ..Boot::default()
        },
        flipping,
    ]
}

fn texts(view: &View) -> Vec<String> {
    let mut out = Vec::new();
    let mut column = |c: &Column| {
        out.push(c.caption.clone());
        out.extend(c.away.map(str::to_owned));
        out.extend(c.reason.clone());
        out.extend(c.status.iter().map(|s| s.text.to_owned()));
    };
    match view {
        View::NotAvailable => {
            return vec![
                NOT_AVAILABLE_HEADING.to_owned(),
                NOT_AVAILABLE_TEXT.to_owned(),
            ];
        }
        View::ServiceDown(down) => {
            column(&down.column);
            out.extend(down.screen.heading.map(str::to_owned));
            out.extend(switch_texts(&down.screen.switch));
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
            out.extend(switch_texts(&n.startup));
        }
    }
    out
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
        mode in prop::sample::select(Mode::all().map(|m| u32::from(m.number())).collect::<Vec<_>>()),
        rotation in 0u8..=3,
        fahrenheit: bool,
        cooler in cooler_state(),
        reasons in prop::collection::btree_map(prop::sample::select(Mode::all().map(Mode::number).collect::<Vec<_>>()), (0usize..2, 0usize..4), 0..12),
        gpu_list in prop::collection::vec((gpu_kind(), any::<bool>(), 0usize..3), 0..4),
        choice in (prop::option::of(0usize..4), prop::option::of(0usize..4)),
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
            available_modes: Some(Mode::all().map(|m| u32::from(m.number())).filter(|m| !unavailable.contains_key(m)).collect()),
            unavailable_reasons: Some(unavailable),
            chosen_gpu: Some(pci(choice.0)),
            gpu_in_use: Some(pci(choice.1)),
            gpus: Some(gpus),
            music_peaks: Some(music.0),
            music_bar_fall: Some(stops[music.1]),
            music_peak_fall: Some(stops[music.2]),
            readings: Some(HashMap::new()),
            use_system_theme: Some(true),
            dark_theme: Some(true),
            window_opacity: Some(odc_ipc::WindowOpacity::Opaque),
            window_corners: Some(odc_ipc::WindowCorners::Rounded),
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
    // Exactly one tile per mode (1–26 but 17), in four groups, in order; and exactly one selected.
    #[test]
    fn one_tile_per_mode(state in any_state(), long: bool) {
        let ui = Ui { sending_failed_long: long, ..Ui::default() };
        let View::Normal(normal) = view(&Service::Up(Box::new(state.clone())), ui, &products(), &startup_on()) else {
            panic!("not the normal screens");
        };
        let modes: Vec<u16> = normal.display.groups.iter().flat_map(|g| g.tiles.iter().map(|t| t.mode)).collect();
        let order: Vec<u16> = GROUPS.iter().flat_map(|&(_, kind)| group_modes(kind)).map(Mode::number).collect();
        prop_assert_eq!(modes, order);
        let kinds: Vec<Kind> = normal.display.groups.iter().map(|g| g.kind).collect();
        prop_assert_eq!(kinds, GROUPS.map(|(_, k)| k).to_vec());
        let selected: Vec<u16> = normal.display.groups.iter().flat_map(|g| &g.tiles).filter(|t| t.selected).map(|t| t.mode).collect();
        prop_assert_eq!(selected, vec![state.mode.unwrap() as u16]);
    }

    // Rule 11: a tile names exactly the devices its mode reads, and its reason names a metric
    // its mode shows; a CPU tile never maps to a GPU mode.
    #[test]
    fn every_tile_shows_its_own_metrics(state in any_state()) {
        let View::Normal(normal) = view(&Service::Up(Box::new(state)), Ui::default(), &products(), &startup_on()) else {
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

    // No view shows a hardware value: no text carries a %, a temperature, a power or a speed.
    #[test]
    fn no_view_shows_a_hardware_value(state in any_state(), long: bool, down: bool, startup in 0..6_usize) {
        let service = if down { Service::Down } else { Service::Up(Box::new(state)) };
        let ui = Ui { sending_failed_long: long, ..Ui::default() };
        for text in texts(&view(&service, ui, &products(), &startup_states()[startup])) {
            prop_assert!(!text.contains('%') && !text.contains('°') && !text.contains("RPM"), "{}", text);
        }
    }
}

mod cooler;
mod gpus;
mod theme;
mod tiles;
mod unavailable;
