use super::*;

// The dev copy's override (`docs/spec/dbus.md`, Dev interface; owner, 2026-10-29): it's drawn from
// the tick that takes it, which the engine reports; real snapshots replace only what's behind it,
// so they still answer Refresh, carrying the override's values; its end brings the last real
// readings back at once.
#[test]
fn an_override_is_drawn_from_the_tick_that_takes_it_until_its_end() {
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(42)));
    engine.tick(None, None, false);
    let taken = engine.tick(readings_override(1, Some(&usage(77))), None, false);
    assert_eq!(engine.adopted(), Some((1, taken.number)));
    assert_eq!(taken.frame, Some(home(1, &usage(77))), "from that tick");
    // A real snapshot changes nothing drawn.
    let real = engine.tick(None, Some(fresh(&usage(43))), false);
    assert_eq!(real.frame, Some(home(1, &usage(77))));
    // Refresh still waits for a real snapshot answering it, and resets with the override's values.
    engine.tick(Some(refresh(1)), None, false);
    let answered = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&usage(44)),
            answers: 1,
        }),
        false,
    );
    assert_eq!(answered.frame, Some(home(1, &usage(77))));
    // The end: the last real readings, at once.
    let ended = engine.tick(readings_override(2, None), None, false);
    assert_eq!(engine.adopted(), Some((2, ended.number)));
    assert_eq!(ended.frame, Some(home(1, &usage(44))));
}

// Two overrides before one tick: the newer is drawn, and its number reported, so the caller of
// the older one learns the tick too.
#[test]
fn only_the_newest_override_is_taken() {
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(42)));
    let older = readings_override(1, Some(&usage(55))).expect("an override");
    let newer = readings_override(2, Some(&usage(66))).expect("an override");
    let out = engine.tick(Some(older.then(newer)), None, false);
    assert_eq!(engine.adopted(), Some((2, out.number)));
    assert_eq!(out.frame, Some(home(1, &usage(66))));
}

// P7-15: two changes before one tick: only the second is adopted; a change and a change back
// adopt nothing new, so the strip keeps its place.
#[test]
fn only_the_newest_settings_are_adopted() {
    let mode = |m: u16| Apply {
        settings: Some(Settings {
            mode: m,
            ..Settings::default()
        }),
        ..Apply::default()
    };
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(42)));
    engine.tick(None, None, false);
    let out = engine.tick(Some(mode(2).then(mode(3))), None, false);
    assert_eq!(engine.settings().mode, 3);
    assert_eq!(out.frame, Some(home(3, &usage(42))));

    // 1 → 2 → 1 while 100% scrolls: no reset; the strip moves on.
    let mut scrolling = Engine::new(Settings::default(), snapshot(&usage(100)));
    let mut alone = Engine::new(Settings::default(), snapshot(&usage(100)));
    for _ in 0..5 {
        scrolling.tick(None, None, false);
        alone.tick(None, None, false);
    }
    let back = scrolling.tick(Some(mode(2).then(mode(1))), None, false);
    assert_eq!(back.frame, alone.tick(None, None, false).frame);
}
