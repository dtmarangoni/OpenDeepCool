use super::*;

#[test]
fn a_change_is_stored_applied_and_sent_to_each_watcher_alone() {
    let h = Harness::start(
        "change",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (one, two, other, changer) = (
            h.client().await,
            h.client().await,
            h.client().await,
            h.client().await,
        );
        one.main.watch().await.expect("watches");
        two.main.watch().await.expect("watches");
        let mut first = one.main.receive_state_changed().await.expect("a stream");
        let mut second = two.main.receive_state_changed().await.expect("a stream");
        // A match rule for StateChanged, but no Watch().
        let mut unwatched = other.main.receive_state_changed().await.expect("a stream");
        changer.main.set_mode(3).await.expect("mode 3");
        assert_eq!(h.saved_settings().mode, 3, "stored");
        let applied = h.apply.take().expect("handed to the engine");
        assert_eq!(applied.settings.map(|s| s.mode), Some(3));
        for stream in [&mut first, &mut second] {
            let signal = within(WAIT, next(stream))
                .await
                .flatten()
                .expect("StateChanged");
            let changes = signal.args().expect("its changes").changes;
            assert_eq!(keys(&changes), ["Mode"]);
            assert_eq!(State::from_dict(&changes).expect("a state").mode, Some(3));
        }
        assert!(
            within(QUIET, next(&mut unwatched)).await.is_none(),
            "a connection that doesn't watch gets nothing"
        );
        assert_eq!(h.polkit.asked(&changer.name()), [Action::ChangeSettings]);
        assert_eq!(h.polkit.asked(&one.name()), [Action::Use]);
    });
}

#[test]
fn a_refused_caller_changes_nothing() {
    let h = Harness::start(
        "refused",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        h.polkit.refuse(&caller.name());
        let before = h.saved();
        assert!(matches!(
            caller.main.set_mode(3).await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            caller.main.refresh().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            caller.main.rotate_screen().await,
            Err(Error::NotAuthorized(_))
        ));
        h.nothing_stored_or_applied(&before);
        assert!(h.journal_has(&format!("SetMode(3) from {} refused", caller.name())));
    });
}

#[test]
fn modes_are_checked_against_the_table_this_build_and_the_readings() {
    // Gpu usage has had no device for longer than the first two minutes.
    let no_gpu = status(readings(Reading::Unavailable(Unavailable::NoDevice)));
    let h = Harness::start("modes", Settings::default(), no_gpu, false);
    zbus::block_on(async {
        let caller = h.client().await;
        let before = h.saved();
        // A mode outside the table (Phase 9 plan, Task 9), and 17, which no mode has since
        // DeepCreative's Emoji 4 was dropped (Phase 11a, 2026-11-23).
        for mode in [0, 17, 27, 100, 70_000, u32::MAX] {
            assert!(
                matches!(
                    caller.main.set_mode(mode).await,
                    Err(Error::InvalidArgument(_))
                ),
                "{mode}"
            );
        }
        match caller.main.set_mode(2).await {
            Err(Error::ModeUnavailable(why)) => {
                assert_eq!(why, "Gpu usage: no device found");
            }
            other => panic!("{other:?}"),
        }
        h.nothing_stored_or_applied(&before);
        // Phase 8 plan, Task 4: Temperature · CPU is drawn, and its reading is there.
        caller.main.set_mode(7).await.expect("mode 7");
        assert_eq!(h.saved_settings().mode, 7, "stored");
        let applied = h.apply.take().expect("handed to the engine");
        assert_eq!(applied.settings.map(|s| s.mode), Some(7));
        // No mode is refused as not yet available (the error stays in the contract); only a
        // reading can refuse one.
        caller.main.set_mode(23).await.expect("mode 23");
        for mode in Mode::all() {
            let answer = caller.main.set_mode(u32::from(mode.number())).await;
            assert!(
                !matches!(answer, Err(Error::NotYetAvailable(_))),
                "mode {}: {answer:?}",
                mode.number()
            );
        }
    });
}

// Phase 8 plan, Task 7: Power · CPU is drawn and selectable; a unit change in it is stored and
// announced, and the engine gets the new settings, whose unit no Power picture reads (the engine's
// scenarios prove no frame changes, S15).
#[test]
fn mode_10_is_selectable_and_a_unit_change_in_it_is_only_stored() {
    let h = Harness::start(
        "power",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (watcher, caller) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        caller.main.set_mode(10).await.expect("mode 10");
        assert_eq!(h.saved_settings().mode, 10, "stored");
        let applied = h.apply.take().and_then(|a| a.settings);
        assert_eq!(applied.map(|s| s.mode), Some(10));
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("StateChanged");
        assert_eq!(keys(&signal.args().expect("its changes").changes), ["Mode"]);
        caller
            .main
            .set_temperature_unit("fahrenheit")
            .await
            .expect("stored");
        let saved = h.saved_settings();
        assert_eq!(
            (saved.mode, saved.temperature_unit),
            (10, TemperatureUnit::Fahrenheit)
        );
        let applied = h
            .apply
            .take()
            .and_then(|a| a.settings)
            .expect("the settings");
        assert_eq!(
            (applied.mode, applied.temperature_unit),
            (10, TemperatureUnit::Fahrenheit)
        );
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("StateChanged");
        assert_eq!(
            keys(&signal.args().expect("its changes").changes),
            ["TemperatureUnit"]
        );
    });
}

#[test]
fn arguments_outside_what_the_service_found_are_refused() {
    let h = Harness::start(
        "arguments",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let before = h.saved();
        let long = "x".repeat(65);
        let refusals = [
            caller.main.set_rotation(4).await,
            caller.main.set_temperature_unit("kelvin").await,
            caller.main.set_temperature_unit(&long).await,
            caller.main.choose_gpu("0000:0c:00.0").await,
            caller.main.choose_gpu(" ").await,
            caller.main.choose_gpu(&long).await,
        ];
        for (i, refusal) in refusals.into_iter().enumerate() {
            assert!(
                matches!(refusal, Err(Error::InvalidArgument(_))),
                "{i}: {refusal:?}"
            );
        }
        h.nothing_stored_or_applied(&before);
    });
}

#[test]
fn the_choices_listed_are_stored() {
    let h = Harness::start(
        "choices",
        Settings {
            rotation: 3,
            ..Settings::default()
        },
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        caller.main.rotate_screen().await.expect("turned");
        assert_eq!(h.saved_settings().rotation, 0, "3 wraps to 0");
        caller
            .main
            .choose_gpu("0000:00:02.0")
            .await
            .expect("chosen");
        caller
            .main
            .set_temperature_unit("fahrenheit")
            .await
            .expect("saved");
        let saved = h.saved_settings();
        assert_eq!(saved.gpu_choice.as_deref(), Some("0000:00:02.0"));
        assert_eq!(saved.temperature_unit, TemperatureUnit::Fahrenheit);
        let state =
            State::from_dict(&caller.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(state.chosen_gpu.as_deref(), Some("0000:00:02.0"));
        // Empty: back to Automatic (owner, 2026-11-04).
        caller.main.choose_gpu("").await.expect("automatic");
        assert_eq!(h.saved_settings().gpu_choice, None);
        let state =
            State::from_dict(&caller.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(state.chosen_gpu.as_deref(), Some(""));
    });
}

#[test]
fn choosing_automatic_while_no_gpu_is_chosen_changes_nothing() {
    let h = Harness::start(
        "automatic",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (watcher, caller) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let before = h.saved();
        caller.main.choose_gpu("").await.expect("success");
        h.nothing_stored_or_applied(&before);
        assert!(
            within(QUIET, next(&mut changes)).await.is_none(),
            "no signal"
        );
    });
}

#[test]
fn choosing_what_is_already_set_changes_nothing() {
    let settings = Settings {
        mode: 3,
        rotation: 2,
        temperature_unit: TemperatureUnit::Fahrenheit,
        gpu_choice: Some("0000:0a:00.0".to_owned()),
        ..Settings::default()
    };
    let h = Harness::start("already", settings, status(readings(value(51))), false);
    zbus::block_on(async {
        let (watcher, caller) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let before = h.saved();
        caller.main.set_mode(3).await.expect("success");
        caller.main.set_rotation(2).await.expect("success");
        caller
            .main
            .set_temperature_unit("fahrenheit")
            .await
            .expect("success");
        caller
            .main
            .choose_gpu("0000:0a:00.0")
            .await
            .expect("success");
        h.nothing_stored_or_applied(&before);
        assert!(
            within(QUIET, next(&mut changes)).await.is_none(),
            "no signal"
        );
        // A real change still reaches the watcher.
        caller.main.set_rotation(1).await.expect("rotation 1");
        assert!(within(WAIT, next(&mut changes)).await.flatten().is_some());
    });
}

// `docs/spec/dbus.md`, Errors: a settings file the service can't write answers `…Error.Failed`,
// which the proxy turns into `Error::Failed` by its name alone. A plain file where the state
// directory was fails every write, even root's, as in CI's container.
#[test]
fn a_settings_file_that_cant_be_written_changes_nothing() {
    let h = Harness::start(
        "unwritable",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    fs::remove_dir_all(&h.state).expect("the state directory goes");
    fs::write(&h.state, "").expect("a plain file in its place");
    zbus::block_on(async {
        let caller = h.client().await;
        match caller.main.set_mode(3).await {
            Err(Error::Failed(text)) => {
                assert!(text.starts_with("the settings can't be saved: "), "{text}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(h.apply.take(), None, "nothing applied");
        let state =
            State::from_dict(&caller.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(state.mode, Some(1), "nothing changed");
        assert!(h.journal_has("the settings can't be saved"));
    });
}

#[test]
fn refresh_is_handed_to_the_engine_and_saves_nothing() {
    let h = Harness::start(
        "refresh",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let before = h.saved();
        caller.main.refresh().await.expect("refreshed");
        let applied = h.apply.take().expect("handed to the engine");
        assert!(applied.refresh);
        assert_eq!(applied.settings, None);
        assert_eq!(h.saved(), before);
    });
}

// S26 (owner, 2026-11-06): Refresh asks the sensor thread for its snapshot as the call arrives,
// so the answer is there before the engine's next tick, which takes the Refresh and resets on it:
// the picture at home on the next frame, with the fresh readings. Each Refresh asks anew.
#[test]
fn refresh_asks_the_sensor_thread_at_once_and_resets_on_the_next_frame() {
    let h = Harness::start(
        "refresh-asks",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        caller.main.refresh().await.expect("refreshed");
        let applied = h.apply.take().expect("handed to the engine");
        let asked = applied.asked.expect("a snapshot request");
        assert_eq!(h.sent(), [SensorRequest::Snapshot(asked)]);
        // The sensor thread's answer, then the engine's next tick, mid-scroll at 100%.
        let mut sensors = SensorLoop::new(FixedSensors(readings(value(101))));
        assert!(sensors.handle(SensorRequest::Snapshot(asked)), "at once");
        let answer = sensors.snapshot();
        let mut engine = Engine::new(Settings::default(), readings(value(100)));
        for _ in 0..5 {
            engine.tick(None, None, false);
        }
        let next = engine.tick(Some(applied), Some(answer), false);
        assert_eq!(next.frame, Some(home(readings(value(101)))));
        caller.main.refresh().await.expect("refreshed");
        let again = h.apply.take().expect("handed to the engine");
        assert!(again.asked > Some(asked), "a new request");
        assert_eq!(
            h.sent(),
            [SensorRequest::Snapshot(again.asked.expect("one"))]
        );
    });
}

// S25 (owner, 2026-11-06): another GPU reaches the sensor thread as the call arrives, then a
// request for a snapshot of it, whose number the engine waits for.
#[test]
fn another_gpu_reaches_the_sensor_thread_at_once_with_a_snapshot_request() {
    let h = Harness::start(
        "gpu-asks",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        caller
            .main
            .choose_gpu("0000:00:02.0")
            .await
            .expect("chosen");
        let applied = h.apply.take().expect("handed to the engine");
        let settings = applied.settings.expect("the new settings");
        assert_eq!(settings.gpu_choice.as_deref(), Some("0000:00:02.0"));
        let asked = applied.asked.expect("a snapshot request");
        assert_eq!(
            h.sent(),
            [
                SensorRequest::Choices(Choices::of(&settings)),
                SensorRequest::Snapshot(asked)
            ],
            "the GPU first, so the answer reads it"
        );
        // Another setting: nothing for the sensor thread.
        caller.main.set_rotation(1).await.expect("turned");
        assert_eq!(h.apply.take().and_then(|a| a.asked), None);
        assert_eq!(h.sent(), []);
    });
}

#[test]
fn reads_need_polkit_s_use_and_a_refused_caller_registers_nothing() {
    let h = Harness::start(
        "reads",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (refused, changer) = (h.client().await, h.client().await);
        h.polkit.refuse(&refused.name());
        let mut changes = refused
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        assert!(matches!(
            refused.main.get_state().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            refused.main.watch().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            refused.main.diagnostics().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            refused.main.unwatch().await,
            Err(Error::NotAuthorized(_))
        ));
        assert_eq!(h.polkit.asked(&refused.name()), [Action::Use; 4]);
        // It wasn't registered: a change sends it nothing, even once polkit lets it read.
        h.polkit.allow(&refused.name());
        changer.main.set_mode(3).await.expect("mode 3");
        assert!(within(QUIET, next(&mut changes)).await.is_none());
    });
}
