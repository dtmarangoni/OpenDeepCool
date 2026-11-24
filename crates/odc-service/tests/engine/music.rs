use super::*;

// PRD layer 3, "audio file → frames" (Phase 9 plan, Task 3, written against the test-only
// switch): every signal of `spectrum/signals.txt` through the engine, at H 12 in mode 13 and H 6
// in mode 14 with the default Music settings; the frames' spectrum rows are `expected.txt`'s,
// tick for tick.
#[test]
fn every_signal_through_the_engine_draws_its_expected_rows() {
    let sections = odc_testkit::spectrum::expected();
    let mut checked = 0;
    for (mode, h) in [(13, 12), (14, 6)] {
        let heading =
            |signal: &str| format!("{signal} · H {h} · peaks on · bars moderate · peaks slow");
        for signal in signals().iter().filter(|s| !s.is_beat_train()) {
            let want = sections
                .iter()
                .find(|e| e.heading == heading(&signal.name))
                .unwrap_or_else(|| panic!("no section {}", heading(&signal.name)));
            let audio = Arc::new(FakeAudio::new());
            let settings = Settings {
                mode,
                ..Settings::default()
            };
            let mut engine = Engine::with_audio(
                settings,
                snapshot(&usage(42)),
                Arc::clone(&audio) as Arc<dyn AudioSource>,
            );
            for (n, (window, rows)) in signal.windows().iter().zip(&want.ticks).enumerate() {
                audio.set(*window);
                let frame = engine.tick(None, None, false).frame.expect("a frame");
                assert_eq!(
                    spectrum_rows(&frame, h),
                    expected_rows(rows),
                    "mode {mode}, {}, tick {n}",
                    signal.name
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 2 * 81);
}

// The Dancer through the engine (Phase 9 plan, Task 9's "audio file → frames", here against the
// test-only switch): every sequence of `spectrum/dancer.txt`, a reset being Refresh.
#[test]
fn every_dancer_sequence_through_the_engine_draws_its_poses() {
    let poses: Vec<odc_protocol::Frame> = odc_render::Pose::all()
        .map(odc_render::Pose::frame)
        .collect();
    let settings = Settings {
        mode: 23,
        ..Settings::default()
    };
    for dance in odc_testkit::spectrum::dances() {
        let audio = Arc::new(FakeAudio::new());
        let mut engine = Engine::with_audio(
            settings.clone(),
            snapshot(&usage(42)),
            Arc::clone(&audio) as Arc<dyn AudioSource>,
        );
        for (n, (window, pose)) in windows()[&dance.signal]
            .iter()
            .zip(&dance.poses)
            .enumerate()
        {
            audio.set(*window);
            let reset = dance.resets.contains(&n);
            let apply = reset.then(|| Apply {
                refresh: true,
                asked: Some(n as u64 + 1),
                ..Apply::default()
            });
            let published = reset.then(|| fresh(&usage(42)));
            let frame = engine.tick(apply, published, false).frame.expect("a frame");
            assert_eq!(
                frame,
                poses[usize::from(*pose)],
                "{}, tick {n}",
                dance.heading
            );
        }
    }
}

// music.md point 8 (owner, 2026-11-05): a missed tick draws nothing and moves nothing; the next
// tick goes on from where the bars, the emoji loop and the Dancer were.
#[test]
fn a_missed_tick_in_music_moves_nothing() {
    // One tick of a tone at column 7's centre, then silence.
    let tone = windows()["burst train, column 7, -20 dBFS, 100 ms on, 400 ms off"][0];
    let sound: Vec<[f32; WINDOW]> = std::iter::once(tone)
        .chain(std::iter::repeat_n([0.0; WINDOW], 7))
        .collect();
    for mode in [13, 14, 16, 18, 23, 25] {
        let settings = Settings {
            mode,
            ..Settings::default()
        };
        let engine = |audio: &Arc<FakeAudio>| {
            Engine::with_audio(
                settings.clone(),
                snapshot(&usage(42)),
                Arc::clone(audio) as Arc<dyn AudioSource>,
            )
        };
        let (on_time, late) = (Arc::new(FakeAudio::new()), Arc::new(FakeAudio::new()));
        let (mut steady, mut missing) = (engine(&on_time), engine(&late));
        // The tone, then silence: the bars fall, the loops step.
        let mut frames = Vec::new();
        for window in &sound {
            on_time.set(*window);
            frames.push(steady.tick(None, None, false).frame.expect("a frame"));
        }
        for (n, window) in sound[..3].iter().enumerate() {
            late.set(*window);
            assert_eq!(missing.tick(None, None, false).frame, Some(frames[n]));
        }
        // Tick 3 is missed: each later tick of the late engine is the steady one's tick before, as
        // the sound is the same silence.
        late.set(sound[3]);
        assert_eq!(missing.tick(None, None, true).frame, None, "mode {mode}");
        for (n, window) in sound[4..8].iter().enumerate() {
            late.set(*window);
            let frame = missing.tick(None, None, false).frame;
            assert_eq!(frame, Some(frames[n + 3]), "mode {mode}, tick {}", n + 4);
        }
    }
}

// Phase 11a Task 13: a Music mode's tick carries the 14 levels analysed from the sound window the
// frame was drawn from (the very `analyse` the spectrum takes), every other mode's tick none, and
// a missed tick none. Every mode, every signal of `spectrum/signals.txt`, every tick of each.
#[test]
fn a_music_tick_carries_the_levels_its_frame_was_drawn_from_and_no_other_does() {
    let mut checked = 0;
    for mode in Mode::all().map(Mode::number) {
        // Every Music mode takes the sound, the emoji alone too (they run only while it plays).
        let music = (13..=26).contains(&mode);
        for signal in signals().iter().filter(|s| !s.is_beat_train()).take(12) {
            let audio = Arc::new(FakeAudio::new());
            let mut engine = Engine::with_audio(
                Settings {
                    mode,
                    ..Settings::default()
                },
                snapshot(&usage(42)),
                Arc::clone(&audio) as Arc<dyn AudioSource>,
            );
            for (n, window) in signal.windows().iter().enumerate() {
                audio.set(*window);
                let tick = engine.tick(None, None, false);
                let want = music.then(|| analyse(window));
                assert_eq!(tick.levels, want, "mode {mode}, {}, tick {n}", signal.name);
                if let Some(levels) = tick.levels {
                    assert!(
                        levels
                            .iter()
                            .all(|l| l.is_finite() && (0.0..=1.0).contains(l)),
                        "mode {mode}, {}, tick {n}: {levels:?}",
                        signal.name
                    );
                }
                checked += 1;
            }
            // A missed deadline draws nothing, so it analyses nothing.
            audio.set(signal.windows()[0]);
            let missed = engine.tick(None, None, true);
            assert_eq!((missed.frame, missed.levels), (None, None), "mode {mode}");
        }
    }
    assert!(checked > 1000, "{checked} ticks");
}

// The analysis is bounded whatever the window holds: every level is finite and within 0 to 1 for
// silence, a constant, the largest samples either way round, and a square wave at each column's
// centre. (Samples reach the window sanitised, never NaN or infinite: `audio::sanitised`.)
#[test]
fn the_levels_are_finite_and_within_zero_to_one_for_any_window() {
    let mut windows: Vec<[f32; WINDOW]> = vec![
        [0.0; WINDOW],
        [1.0; WINDOW],
        [-1.0; WINDOW],
        [f32::MAX; WINDOW],
        [f32::MIN; WINDOW],
        [f32::MIN_POSITIVE; WINDOW],
        std::array::from_fn(|i| if i % 2 == 0 { f32::MAX } else { f32::MIN }),
    ];
    for period in [2, 3, 5, 8, 13, 21, 34, 55, 89, 144] {
        windows.push(std::array::from_fn(|i| {
            if (i / period) % 2 == 0 { 1.0 } else { -1.0 }
        }));
    }
    for (n, window) in windows.iter().enumerate() {
        let levels = analyse(window);
        assert_eq!(levels.len(), COLUMNS);
        assert!(
            levels
                .iter()
                .all(|l| l.is_finite() && (0.0..=1.0).contains(l)),
            "window {n}: {levels:?}"
        );
    }
}
