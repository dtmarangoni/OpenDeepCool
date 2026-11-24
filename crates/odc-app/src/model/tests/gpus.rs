use super::*;

// Row "Normal, several GPUs": the Display page, the cooler connected, the service running,
// the GPU on Automatic, naming the GPU it reads.
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
    want.display.live.restart.2 = INTEGRATED.to_owned();
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
    want.display.live.restart.2 = QUERY_FAILED.to_owned();
    want.config.gpu = Some(GpuRow {
        entries: all_gpu_entries(),
        chosen: 3,
        in_use: None,
        hint: None,
        description: "driver couldn't be asked".to_owned(),
    });
    assert_eq!(view_of(state), want);
}

// Phase 11a Task 14: what the live tiles draw from. No `Readings` in the state is the design's
// sample; `Readings` are each metric by its spec name, one left out has no value; the Music
// settings are the state's; the sound can be live only with sound attached in a Music mode; and
// what restarts the tiles together is the mode, the unit and the GPU the readings come from.
#[test]
fn the_live_tiles_draw_from_the_states_readings_and_music_settings() {
    // No `Readings` key yet.
    let mut state = base();
    state.readings = None;
    assert_eq!(view_of(state).display.live.values, sample_values());
    // Some, with one left out and a fraction.
    let mut state = base();
    state.readings = Some(HashMap::from([
        ("CPU Usage".to_owned(), (42_500, 1000)),
        ("Gpu power".to_owned(), (88, 1)),
    ]));
    let values = view_of(state).display.live.values;
    assert_eq!(
        values.get(Metric::CpuUsage),
        odc_metrics::Exact::new(42_500, 1000)
    );
    assert_eq!(values.get(Metric::GpuPower), odc_metrics::Exact::new(88, 1));
    assert_eq!(values.get(Metric::MemoryLoad), None);
    // The Music settings.
    let mut state = base();
    state.music_peaks = Some(false);
    state.music_bar_fall = Some(FallStop::Faster);
    state.music_peak_fall = Some(FallStop::Slower);
    assert_eq!(
        view_of(state).display.live.music,
        MusicSettings {
            peaks: false,
            bar_fall: Stop::Faster,
            peak_fall: Stop::Slower,
        }
    );
    // The sound can be live only with a helper's sound attached in a Music mode.
    for (mode, connected, live) in [
        (1, true, false),
        (12, true, false),
        (13, true, true),
        (14, true, true),
        (23, true, true),
        (13, false, false),
        (23, false, false),
        // The emoji alone are Music modes like the others: they run while sound plays.
        (24, true, true),
        (26, true, true),
    ] {
        let mut state = base();
        state.mode = Some(mode);
        state.audio_connected = Some(connected);
        assert_eq!(
            view_of(state).display.live.sound,
            live,
            "mode {mode}, connected {connected}"
        );
    }
    // What restarts the tiles.
    let mut state = base();
    state.mode = Some(4);
    state.temperature_unit = Some(TemperatureUnit::Fahrenheit);
    state.gpu_in_use = Some(NO_RULE.to_owned());
    assert_eq!(
        view_of(state).display.live.restart,
        (4, TemperatureUnit::Fahrenheit, NO_RULE.to_owned())
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
    want.display.live.restart = (1, TemperatureUnit::Celsius, NO_RULE.to_owned());
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
                "Not on the cooler: it isn't connected.",
                Some(Banner::NotConnected)
            ),
            "{state_value:?}"
        );
    }
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
