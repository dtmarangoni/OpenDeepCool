use super::*;

// Row "Metric unavailable": the tiles disabled with "Unavailable", each group's reasons once.
#[test]
fn a_metric_unavailable() {
    let mut state = base();
    state.mode = Some(7);
    state.available_modes = Some(vec![
        1, 3, 5, 7, 10, 13, 14, 15, 16, 18, 20, 22, 23, 24, 25, 26,
    ]);
    state.unavailable_reasons = Some(gpu_reasons("no device found"));
    let shown = gpu_reasons_shown("no device found");
    let mut want = base_view();
    want.column.caption = "Temperature · CPU".to_owned();
    want.display.groups = expected_groups(7, &borrowed(&shown));
    want.display.live.restart.0 = 7;
    let got = view_of(state);
    assert_eq!(got, want);
    // Each group lists each of its reasons once.
    let reasons: Vec<&Vec<String>> = got.display.groups.iter().map(|g| &g.reasons).collect();
    assert_eq!(*reasons[0], ["GPU load: no device found"]);
    assert_eq!(*reasons[1], ["GPU temperature: no device found"]);
    assert_eq!(*reasons[2], ["GPU power: no device found"]);
    assert_eq!(
        *reasons[3],
        [
            "GPU load: no device found",
            "GPU temperature: no device found"
        ]
    );
    let tile = &got.display.groups[0].tiles[1];
    assert_eq!(
        (tile.sensitive, tile.flag, tile.description.as_deref()),
        (
            false,
            Some(Flag::Unavailable),
            Some("GPU load: no device found")
        )
    );
}

// Row "The saved mode lost a reading": its tile stays selected and sensitive, "Reading lost";
// the reason under the preview.
#[test]
fn the_saved_mode_lost_a_reading() {
    let mut state = base();
    state.available_modes = Some(vec![
        1, 3, 5, 7, 10, 13, 14, 15, 16, 18, 20, 22, 23, 24, 25, 26,
    ]);
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
        view(&Service::Refused, Ui::default(), &products(), &startup_on()),
        View::NotAvailable
    );
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
    assert_eq!(reason_text("Something: else"), "Something: else");
}
