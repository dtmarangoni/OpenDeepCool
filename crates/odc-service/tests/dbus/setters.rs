use super::*;

// docs/spec/dbus.md, the window setters (Phase 11a Task 49): each takes its words only, stores
// them in the one settings file, tells the watchers only the key that changed, hands the settings
// to the engine and survives a restart; the same word again changes nothing; a word that isn't
// one of theirs, or is too long, is refused and changes nothing; a caller polkit refuses gets
// NotAuthorized and changes nothing. Like the theme's, they are only stored.
#[test]
fn each_window_setter_validates_stores_tells_the_watchers_and_survives_a_restart() {
    let h = Harness::start(
        "window-settings",
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
            (state.window_opacity, state.window_corners),
            (
                Some(odc_ipc::WindowOpacity::Opaque),
                Some(odc_ipc::WindowCorners::Rounded)
            ),
            "opaque and rounded by default"
        );
        // One at a time, so a swapped pair shows.
        changer
            .main
            .set_window_opacity("glassy")
            .await
            .expect("stored");
        let saved = h.saved_settings();
        assert_eq!(
            (saved.window_opacity, saved.window_corners),
            (WindowOpacity::Glassy, WindowCorners::Rounded)
        );
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
            (seen.window_opacity, seen.window_corners, seen.mode),
            (Some(odc_ipc::WindowOpacity::Glassy), None, None),
            "only the key that changed"
        );
        changer
            .main
            .set_window_corners("sharp")
            .await
            .expect("stored");
        let saved = h.saved_settings();
        assert_eq!(
            (saved.window_opacity, saved.window_corners),
            (WindowOpacity::Glassy, WindowCorners::Sharp)
        );
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("a signal");
        let seen = state_of(&signal.args().expect("changes").changes);
        assert_eq!(
            (seen.window_opacity, seen.window_corners),
            (None, Some(odc_ipc::WindowCorners::Sharp))
        );
        changer
            .main
            .set_window_opacity("transparent")
            .await
            .expect("stored");
        assert_eq!(
            h.saved_settings().window_opacity,
            WindowOpacity::Transparent
        );
        // A restart of the service reads them back.
        let (again, _) = SettingsStore::new(&h.state).load().expect("loads");
        assert_eq!(
            (again.window_opacity, again.window_corners),
            (WindowOpacity::Transparent, WindowCorners::Sharp)
        );
        // Words that aren't theirs, and a word too long, are refused and nothing changes.
        h.apply.take();
        let before = h.saved();
        for bad in ["frosted", "Glassy", "", "Opaque ", &"x".repeat(65)] {
            assert!(
                matches!(
                    changer.main.set_window_opacity(bad).await,
                    Err(Error::InvalidArgument(_))
                ),
                "{bad:?}"
            );
        }
        for bad in ["round", "Sharp", "", "corners", &"x".repeat(65)] {
            assert!(
                matches!(
                    changer.main.set_window_corners(bad).await,
                    Err(Error::InvalidArgument(_))
                ),
                "{bad:?}"
            );
        }
        h.nothing_stored_or_applied(&before);
        // What's already set changes nothing: nothing stored, applied or signalled.
        changer
            .main
            .set_window_opacity("transparent")
            .await
            .expect("already set");
        changer
            .main
            .set_window_corners("sharp")
            .await
            .expect("already set");
        h.nothing_stored_or_applied(&before);
        // A caller polkit refuses changes nothing.
        h.polkit.refuse(&changer.name());
        assert!(matches!(
            changer.main.set_window_opacity("opaque").await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            changer.main.set_window_corners("rounded").await,
            Err(Error::NotAuthorized(_))
        ));
        h.nothing_stored_or_applied(&before);
    });
}

// docs/spec/dbus.md, OverrideAudio: the test session's sound replaces the helper's until
// ClearOverride or the session's end.
#[test]
fn the_sound_override_needs_the_session_and_ends_with_it() {
    let h = Harness::start("override-audio", music(13), music_status(), true);
    zbus::block_on(async {
        let (holder, other) = (h.client().await, h.client().await);
        let (read_end, write_end) =
            rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).expect("a pipe");
        let fd = zbus::wire::Fd::from(&read_end);
        assert!(
            matches!(
                holder.dev.override_audio(fd).await,
                Err(Error::NotAuthorized(_))
            ),
            "BeginTest() first"
        );
        holder.dev.begin_test().await.expect("the session");
        assert!(matches!(
            other
                .dev
                .override_audio(zbus::wire::Fd::from(&read_end))
                .await,
            Err(Error::Busy(_))
        ));
        holder
            .dev
            .override_audio(zbus::wire::Fd::from(&read_end))
            .await
            .expect("overridden");
        drop(read_end);
        assert!(h.audio.overridden());
        let mut test = fs::File::from(write_end);
        test.write_all(&frames(&[0.375; 4])).expect("written");
        assert!(soon(|| h.audio.window()[WINDOW - 1] == 0.375).await);
        // The engine has taken every override, as the D-Bus side alone has none.
        h.status.write().expect("the status").adopted = Some((u64::MAX, 0));
        holder.dev.clear_override().await.expect("cleared");
        assert!(!h.audio.overridden());
        assert!(soon(|| closed(&mut test)).await, "its pipe closed");
        let (read_end, _write_end) =
            rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).expect("a pipe");
        holder
            .dev
            .override_audio(zbus::wire::Fd::from(&read_end))
            .await
            .expect("overridden");
        holder.dev.end_test().await.expect("ended");
        assert!(!h.audio.overridden(), "ended with the session");
    });
}
