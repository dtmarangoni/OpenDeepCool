use super::*;

// docs/spec/dbus.md, AttachAudio (Phase 9 plan, Task 3): allowed in a Music mode, a pipe whose
// samples reach the engine's window; the attach is logged by the caller's user and PID, never with
// any sound; AudioConnected tells the watchers.
#[test]
fn an_allowed_attach_gets_a_pipe_whose_samples_reach_the_window() {
    let h = Harness::start("attach", music(13), music_status(), false);
    zbus::block_on(async {
        let (helper, watcher) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let before = state_of(&watcher.main.get_state().await.expect("the state"));
        assert_eq!(before.audio_wanted, Some(true));
        assert_eq!(before.audio_connected, Some(false));
        let mut pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        assert_eq!(h.polkit.asked(&helper.name()), [Action::FeedAudio]);
        let samples = [0.5_f32, -0.25, 0.125];
        pipe.write_all(&frames(&samples)).expect("written");
        assert!(
            soon(|| h.audio.window()[WINDOW - 3..] == samples).await,
            "the samples reach the window"
        );
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("StateChanged");
        let changed = signal.args().expect("its changes").changes;
        assert_eq!(keys(&changed), ["AudioConnected"]);
        assert_eq!(state_of(&changed).audio_connected, Some(true));
        assert!(h.journal_has(&format!(
            "sound attached from {} (user {USER}, PID {PID})",
            helper.name()
        )));
    });
}

#[test]
fn a_refused_or_unwanted_attach_gets_no_pipe() {
    let h = Harness::start("attach-refused", music(13), music_status(), false);
    zbus::block_on(async {
        let helper = h.client().await;
        h.polkit.refuse(&helper.name());
        assert!(matches!(
            helper.main.attach_audio().await,
            Err(Error::NotAuthorized(_))
        ));
        assert_eq!(h.audio.holder(), None);
    });
    // Outside the Music modes it's refused before polkit is asked (owner, 2026-10-26).
    let h = Harness::start(
        "attach-unwanted",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let helper = h.client().await;
        let state = state_of(&helper.main.get_state().await.expect("the state"));
        assert_eq!(state.audio_wanted, Some(false));
        assert!(matches!(
            helper.main.attach_audio().await,
            Err(Error::AudioNotWanted(_))
        ));
        assert_eq!(h.polkit.asked(&helper.name()), [Action::Use]);
        assert_eq!(h.audio.holder(), None);
        assert!(h.journal_has("AttachAudio from"));
    });
}

// The check every second (Phase 9 plan, Task 3): a caller that no longer passes polkit's
// feed-audio has its pipe closed, which its writer sees as EPIPE, and the journal says why.
#[test]
fn an_attached_caller_that_stops_passing_polkit_loses_its_pipe() {
    let h = Harness::start("attach-recheck", music(14), music_status(), false);
    zbus::block_on(async {
        let helper = h.client().await;
        let mut pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        sleep(Duration::from_millis(1200)).await;
        assert!(!closed(&mut pipe), "still allowed after a check");
        h.polkit.refuse(&helper.name());
        assert!(soon(|| closed(&mut pipe)).await, "closed within a second");
        assert_eq!(h.audio.holder(), None);
        assert!(
            h.journal_has("polkit no longer allows io.github.dtmarangoni.OpenDeepCool1.feed-audio")
        );
    });
}

// The latest authorized attach wins; a caller that leaves the bus loses its pipe.
#[test]
fn a_newer_attach_replaces_the_older_and_leaving_the_bus_closes_the_pipe() {
    let h = Harness::start("attach-replace", music(23), music_status(), false);
    zbus::block_on(async {
        let (first, second) = (h.client().await, h.client().await);
        let mut older = writer(first.main.attach_audio().await.expect("a pipe"));
        let mut newer = writer(second.main.attach_audio().await.expect("a pipe"));
        assert!(soon(|| closed(&mut older)).await, "the older pipe closed");
        assert!(!closed(&mut newer));
        assert_eq!(h.audio.holder().map(|(_, n)| n), Some(second.name()));
        assert!(h.journal_has(&format!("in place of {}'s", first.name())));
        let name = second.name();
        drop(second);
        assert!(soon(|| closed(&mut newer)).await, "closed when it left");
        assert_eq!(h.audio.holder(), None);
        assert!(h.journal_has(&format!("the sound from {name} is closed: it left the bus")));
    });
}

// Sound only in the Music modes (owner, 2026-10-26): leaving them closes the pipe at once and turns
// AudioWanted off; coming back turns it on, and the watchers hear both.
#[test]
fn leaving_music_closes_the_pipe_and_audio_wanted_follows_the_mode() {
    let h = Harness::start("attach-leave", music(13), music_status(), false);
    zbus::block_on(async {
        let (helper, watcher, changer) = (h.client().await, h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let mut pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        let connected = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal");
        assert_eq!(
            keys(&connected.args().expect("changes").changes),
            ["AudioConnected"]
        );
        changer.main.set_mode(1).await.expect("mode 1");
        assert_eq!(h.audio.holder(), None, "closed by the time SetMode returns");
        assert!(soon(|| closed(&mut pipe)).await);
        let left = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal");
        let left = left.args().expect("changes").changes;
        assert_eq!(keys(&left), ["AudioConnected", "AudioWanted", "Mode"]);
        assert_eq!(state_of(&left).audio_wanted, Some(false));
        assert!(h.journal_has("the display left the Music modes"));
        assert!(matches!(
            helper.main.attach_audio().await,
            Err(Error::AudioNotWanted(_))
        ));
        changer.main.set_mode(16).await.expect("mode 16");
        let back = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal");
        let back = back.args().expect("changes").changes;
        assert_eq!(keys(&back), ["AudioWanted", "Mode"]);
        assert_eq!(state_of(&back).audio_wanted, Some(true));
    });
}

// Every Music mode takes sound, the three emoji alone too (they run only while it plays; owner,
// 2026-11-23): moving between them keeps the pipe and AudioWanted on.
#[test]
fn an_emoji_alone_takes_sound_like_every_music_mode() {
    let h = Harness::start("attach-alone", music(14), music_status(), false);
    zbus::block_on(async {
        let (helper, changer) = (h.client().await, h.client().await);
        let mut pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        for mode in [25, 16, 24, 13] {
            changer.main.set_mode(mode).await.expect("a Music mode");
            let state = state_of(&helper.main.get_state().await.expect("the state"));
            assert_eq!(state.audio_wanted, Some(true), "mode {mode}");
            assert!(!closed(&mut pipe), "the pipe stays open in mode {mode}");
        }
    });
}

// docs/spec/dbus.md, the Music setters: each validates its stop, stores it, hands it to the engine
// and tells the watchers; what's already set changes nothing.
#[test]
fn each_music_setter_validates_stores_and_tells_the_watchers() {
    let h = Harness::start(
        "music-settings",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (changer, watcher) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let state = state_of(&watcher.main.get_state().await.expect("the state"));
        assert_eq!(
            (
                state.music_peaks,
                state.music_bar_fall,
                state.music_peak_fall
            ),
            (Some(true), Some(FallStop::Moderate), Some(FallStop::Slow)),
            "Winamp's defaults"
        );
        changer.main.set_music_peaks(false).await.expect("stored");
        changer
            .main
            .set_music_bar_fall("fast")
            .await
            .expect("stored");
        changer
            .main
            .set_music_peak_fall("slower")
            .await
            .expect("stored");
        let saved = h.saved_settings();
        assert_eq!(
            (
                saved.music_peaks,
                saved.music_bar_fall,
                saved.music_peak_fall
            ),
            (false, Fall::Fast, Fall::Slower)
        );
        let applied = h
            .apply
            .take()
            .and_then(|a| a.settings)
            .expect("handed to the engine");
        assert_eq!(applied, saved, "the newest settings");
        let mut seen = State::default();
        while seen.music_peak_fall.is_none() {
            let signal = within(WAIT, next(&mut changes))
                .await
                .flatten()
                .expect("a signal");
            seen.merge(state_of(&signal.args().expect("changes").changes));
        }
        assert_eq!(
            (seen.music_peaks, seen.music_bar_fall, seen.music_peak_fall),
            (Some(false), Some(FallStop::Fast), Some(FallStop::Slower))
        );
        assert_eq!(seen.mode, None, "no other key");
        let before = h.saved();
        for bad in ["fastest", "Moderate", "", &"x".repeat(65)] {
            assert!(
                matches!(
                    changer.main.set_music_bar_fall(bad).await,
                    Err(Error::InvalidArgument(_))
                ),
                "{bad:?}"
            );
            assert!(matches!(
                changer.main.set_music_peak_fall(bad).await,
                Err(Error::InvalidArgument(_))
            ));
        }
        changer
            .main
            .set_music_peaks(false)
            .await
            .expect("already set");
        changer
            .main
            .set_music_bar_fall("fast")
            .await
            .expect("already set");
        h.nothing_stored_or_applied(&before);
        h.polkit.refuse(&changer.name());
        assert!(matches!(
            changer.main.set_music_peaks(true).await,
            Err(Error::NotAuthorized(_))
        ));
        h.nothing_stored_or_applied(&before);
    });
}

// docs/spec/dbus.md, the theme setters (Phase 11a Task 10): each stores its boolean in the one
// settings file, tells the watchers, hands the settings to the engine and survives a restart; the
// same value again changes nothing; a caller polkit refuses gets NotAuthorized and changes nothing.
// The service never reads a theme: the cooler isn't touched, so the engine gets only the settings.
#[test]
fn each_theme_setter_stores_tells_the_watchers_and_survives_a_restart() {
    let h = Harness::start(
        "theme-settings",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (changer, watcher) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let state = state_of(&watcher.main.get_state().await.expect("the state"));
        assert_eq!(
            (state.use_system_theme, state.dark_theme),
            (Some(true), Some(true)),
            "both on by default"
        );
        // One at a time, so a swapped pair shows.
        changer
            .main
            .set_use_system_theme(false)
            .await
            .expect("stored");
        let saved = h.saved_settings();
        assert_eq!((saved.use_system_theme, saved.dark_theme), (false, true));
        assert_eq!(
            h.apply.take().and_then(|a| a.settings),
            Some(saved.clone()),
            "handed to the engine"
        );
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal");
        let seen = state_of(&signal.args().expect("changes").changes);
        assert_eq!(
            (seen.use_system_theme, seen.dark_theme, seen.mode),
            (Some(false), None, None),
            "only the key that changed"
        );
        changer.main.set_dark_theme(false).await.expect("stored");
        let saved = h.saved_settings();
        assert_eq!((saved.use_system_theme, saved.dark_theme), (false, false));
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal");
        let seen = state_of(&signal.args().expect("changes").changes);
        assert_eq!(
            (seen.use_system_theme, seen.dark_theme),
            (None, Some(false))
        );
        // A restart of the service reads them back.
        let (again, _) = SettingsStore::new(&h.state).load().expect("loads");
        assert_eq!((again.use_system_theme, again.dark_theme), (false, false));
        // What's already set changes nothing: nothing stored, applied or signalled.
        h.apply.take();
        let before = h.saved();
        changer
            .main
            .set_use_system_theme(false)
            .await
            .expect("already set");
        changer
            .main
            .set_dark_theme(false)
            .await
            .expect("already set");
        h.nothing_stored_or_applied(&before);
        // A caller polkit refuses changes nothing.
        h.polkit.refuse(&changer.name());
        assert!(matches!(
            changer.main.set_use_system_theme(true).await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            changer.main.set_dark_theme(true).await,
            Err(Error::NotAuthorized(_))
        ));
        h.nothing_stored_or_applied(&before);
    });
}
