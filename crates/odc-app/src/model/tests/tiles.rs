use super::*;

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
        (21, true, "Music · GPU load", ("Sound connected", Dot::Good)),
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
        want.display.live.restart.0 = mode as u16;
        // The sound can be live when a helper's is attached while a Music mode is shown.
        want.display.live.sound = connected;
        assert_eq!(view_of(state), want, "mode {mode}");
    }
}

// Row "A value of 100 or more, °F": the preview mirrors the service's frames, and the tiles
// draw in °F; the view carries the unit, for the tiles and for the Configuration page's toggles.
#[test]
fn fahrenheit() {
    let mut state = base();
    state.temperature_unit = Some(TemperatureUnit::Fahrenheit);
    let mut want = base_view();
    want.display.unit = TemperatureUnit::Fahrenheit;
    want.display.live.restart.1 = TemperatureUnit::Fahrenheit;
    want.config.unit = TemperatureUnit::Fahrenheit;
    assert_eq!(view_of(state), want);
}

// Row "First start": Load · CPU, 0°, °C, peak dots on, Moderate / Slow, the GPU on Automatic.
#[test]
fn first_start() {
    let mut state = base();
    state.mode = Some(1);
    let mut want = base_view();
    want.column.caption = "Load · CPU".to_owned();
    want.display.groups = expected_groups(1, &[]);
    want.display.live.restart.0 = 1;
    assert_eq!(view_of(state), want);
}
