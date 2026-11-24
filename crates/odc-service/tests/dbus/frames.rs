use super::*;

// The whole service (Phase 9 plan, Task 3): a tone through the pipe draws its bar; when the pipe
// closes (here its writer ends; polkit's check closes it the same way, above), the spectrum hears
// silence and the bar falls to the floor at its bar fall rate: 2 rows a tick at moderate, H 12
// (music.md point 5), a peak dot above it.
#[test]
fn a_tone_through_the_pipe_draws_its_bar_and_a_closed_pipe_lets_it_fall() {
    let s = Service::start_on("music-pipe", vec![Presence::Present], false, &music(13));
    zbus::block_on(async {
        let helper = Client::on(&s.bus).await;
        let mut pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        // Column 7's centre at −20 dBFS, written as it plays, 20 ms at a time.
        let tone: Vec<f32> = (0..960 * 40)
            .map(|n| {
                (0.1 * (2.0 * std::f64::consts::PI * 645.891_059 * n as f64 / 48_000.0).sin())
                    as f32
            })
            .collect();
        let player = std::thread::spawn(move || {
            for chunk in tone.chunks(960) {
                if pipe.write_all(&frames(chunk)).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            drop(pipe);
        });
        assert!(
            soon(|| frames_written(&s).iter().any(|f| column_7(f).0 >= 8)).await,
            "the tone's bar rises"
        );
        player.join().expect("played");
        // Its writer closed: silence. The bar falls from where it was to the floor, 2 rows a tick;
        // the peak dot, at its slow default, takes about 1.4 s to come down from there.
        sleep(Duration::from_millis(2500)).await;
        let shown: Vec<(usize, bool)> = frames_written(&s).iter().map(column_7).collect();
        let top = shown
            .iter()
            .rposition(|&(bar, _)| bar >= 8)
            .expect("the bar was up");
        let bars: Vec<usize> = shown[top..].iter().map(|&(bar, _)| bar).collect();
        // A dot right above the bar looks like a bar a row taller: once the bar is down, at most
        // the floor and a dot touching it.
        let falling = bars.iter().take_while(|&&bar| bar > 2).count();
        assert!(
            bars[..falling].windows(2).all(|w| w[0] - w[1] == 2),
            "falling at the bar fall rate, 2 rows a tick: {bars:?}"
        );
        assert!(bars[falling..].iter().all(|&bar| bar <= 2), "{bars:?}");
        assert_eq!(
            shown.last(),
            Some(&(1, false)),
            "down to the floor, the dot too"
        );
        assert!(
            s.journal
                .lines()
                .iter()
                .any(|(_, l)| l.contains("stopped, its writer closed it"))
        );
    });
}

// Phase 10 plan, Task 2: `Frame` goes to each frame watcher alone, once per change; a newcomer
// gets the next frame drawn; a connection with a match rule but no watch gets nothing; a second
// WatchFrames() changes nothing; one leaving keeps the signal for the other.
#[test]
fn frames_go_to_each_frame_watcher_alone_once_per_change() {
    let h = Harness::start(
        "frames",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (watcher, bystander) = (h.client().await, h.client().await);
        let mut seen = watcher.main.receive_frame().await.expect("a stream");
        let mut unseen = bystander.main.receive_frame().await.expect("a stream");
        h.frame(lit(0));
        assert_eq!(
            next_frame(&mut seen, QUIET).await,
            None,
            "no watcher, no frame"
        );
        watcher.main.watch_frames().await.expect("watches");
        watcher
            .main
            .watch_frames()
            .await
            .expect("a second call changes nothing");
        h.frame(lit(1));
        assert_eq!(next_frame(&mut seen, WAIT).await, Some(lit(1)));
        h.frame(lit(1));
        assert_eq!(next_frame(&mut seen, QUIET).await, None, "unchanged");
        h.frame(lit(2));
        assert_eq!(next_frame(&mut seen, WAIT).await, Some(lit(2)));
        assert_eq!(
            next_frame(&mut unseen, QUIET).await,
            None,
            "a match rule without a watch"
        );
        // A newcomer gets the next frame drawn, unchanged or not; the first gets only changes.
        let newcomer = h.client().await;
        let mut fresh = newcomer.main.receive_frame().await.expect("a stream");
        newcomer.main.watch_frames().await.expect("watches");
        h.frame(lit(2));
        assert_eq!(next_frame(&mut fresh, WAIT).await, Some(lit(2)));
        assert_eq!(next_frame(&mut seen, QUIET).await, None);
        drop(seen);
        drop(watcher);
        h.frame(lit(3));
        assert_eq!(next_frame(&mut fresh, WAIT).await, Some(lit(3)), "kept");
        newcomer.main.unwatch_frames().await.expect("stops");
        h.frame(lit(4));
        assert_eq!(
            next_frame(&mut fresh, QUIET).await,
            None,
            "after UnwatchFrames"
        );
    });
}

// Only the active session's callers watch frames: a refused caller can't start, and a watcher
// the check stops allowing gets none until it passes again, then the next frame.
#[test]
fn only_authorized_callers_get_frames_and_a_refused_watcher_waits_for_its_check() {
    let h = Harness::start(
        "frames-polkit",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (refused, watcher) = (h.client().await, h.client().await);
        h.polkit.refuse(&refused.name());
        assert!(matches!(
            refused.main.watch_frames().await,
            Err(Error::NotAuthorized(_))
        ));
        let mut frames = watcher.main.receive_frame().await.expect("a stream");
        watcher.main.watch_frames().await.expect("watches");
        h.frame(lit(5));
        assert_eq!(next_frame(&mut frames, WAIT).await, Some(lit(5)));
        // The check runs on the D-Bus side's one-second timer: its journal line says when.
        let h = &h;
        let until = |line: String| async move {
            for _ in 0..30 {
                if h.journal_has(&line) {
                    return;
                }
                sleep(Duration::from_millis(100)).await;
            }
            panic!("no journal line {line:?}");
        };
        h.polkit.refuse(&watcher.name());
        until(format!("watcher {} no longer passes", watcher.name())).await;
        h.frame(lit(6));
        assert_eq!(next_frame(&mut frames, QUIET).await, None, "skipped");
        h.polkit.allow(&watcher.name());
        until(format!(
            "watcher {} passes polkit's check again",
            watcher.name()
        ))
        .await;
        h.frame(lit(6));
        assert_eq!(next_frame(&mut frames, WAIT).await, Some(lit(6)), "back");
    });
}

// Phase 11a Task 13, docs/spec/dbus.md, State, `Readings`: every caller's `GetState()` has each
// metric's exact fraction and leaves out the one with no value; `StateChanged` sends the key only
// to a watcher that also watches frames, the whole of it the first time and then only when it
// changed; a watcher of state alone gets every other key and never this one.
#[test]
fn readings_reach_only_frame_watchers_whole_the_first_time_then_when_they_change() {
    let h = Harness::start(
        "readings",
        Settings::default(),
        status(fractions(50_000)),
        false,
    );
    zbus::block_on(async {
        let (plain, both, reader, changer) = (
            h.client().await,
            h.client().await,
            h.client().await,
            h.client().await,
        );
        for client in [&plain, &both, &reader] {
            let state = state_of(&client.main.get_state().await.expect("the state"));
            assert_eq!(state.readings, Some(readings_of(50_000)), "GetState");
            assert!(
                !state.readings.expect("readings").contains_key("CPU Power"),
                "a metric with no value is left out"
            );
        }
        plain.main.watch().await.expect("watches");
        both.main.watch().await.expect("watches");
        reader.main.watch_frames().await.expect("watches frames");
        let mut plain_changes = plain.main.receive_state_changed().await.expect("a stream");
        let mut both_changes = both.main.receive_state_changed().await.expect("a stream");
        let mut reader_changes = reader.main.receive_state_changed().await.expect("a stream");
        // Other readings: no state watcher that doesn't watch frames hears of it at all.
        h.read(fractions(60_000));
        assert!(next_changes(&mut plain_changes, QUIET).await.is_none());
        assert!(next_changes(&mut both_changes, QUIET).await.is_none());
        // A watcher that starts watching frames gets the whole key at once, from the status as
        // it is now.
        both.main.watch_frames().await.expect("watches frames");
        let (names, state) = next_changes(&mut both_changes, WAIT)
            .await
            .expect("Readings");
        assert_eq!(names, ["Readings"]);
        assert_eq!(state.readings, Some(readings_of(60_000)));
        // Readings that change are sent as that key alone, and only to it.
        h.read(fractions(61_000));
        let (names, state) = next_changes(&mut both_changes, WAIT)
            .await
            .expect("Readings");
        assert_eq!(names, ["Readings"]);
        assert_eq!(state.readings, Some(readings_of(61_000)));
        assert!(next_changes(&mut plain_changes, QUIET).await.is_none());
        // Other keys change: the frame watcher is not sent the readings again, and the plain
        // one never is.
        changer.main.set_mode(2).await.expect("mode 2");
        for stream in [&mut plain_changes, &mut both_changes] {
            let (names, state) = next_changes(stream, WAIT).await.expect("Mode");
            assert_eq!(names, ["Mode"]);
            assert_eq!(state.readings, None);
        }
        // Readings that are the same again are no change.
        h.read(fractions(61_000));
        assert!(next_changes(&mut both_changes, QUIET).await.is_none());
        // A caller that watches frames but not the state is addressed by no `StateChanged`.
        assert!(next_changes(&mut reader_changes, QUIET).await.is_none());
        // Every metric lost its value: an empty map, not an absent key.
        h.read(Snapshot::new(|_| {
            Reading::Unavailable(Unavailable::NoDevice)
        }));
        let (names, state) = next_changes(&mut both_changes, WAIT)
            .await
            .expect("Readings");
        assert_eq!(names, ["Readings"]);
        assert_eq!(state.readings, Some(HashMap::new()));
    });
}

// A frame watcher that stops passing polkit's check gets nothing until it passes again, and then
// the whole state, readings included.
#[test]
fn a_frame_watcher_that_stops_passing_gets_the_whole_state_with_the_readings_again() {
    let h = Harness::start(
        "readings-polkit",
        Settings::default(),
        status(fractions(50_000)),
        false,
    );
    zbus::block_on(async {
        let (refused, watcher) = (h.client().await, h.client().await);
        h.polkit.refuse(&refused.name());
        assert!(matches!(
            refused.main.watch_frames().await,
            Err(Error::NotAuthorized(_))
        ));
        watcher.main.watch().await.expect("watches");
        watcher.main.watch_frames().await.expect("watches frames");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let h = &h;
        let until = |line: String| async move {
            for _ in 0..30 {
                if h.journal_has(&line) {
                    return;
                }
                sleep(Duration::from_millis(100)).await;
            }
            panic!("no journal line {line:?}");
        };
        h.polkit.refuse(&watcher.name());
        until(format!("watcher {} no longer passes", watcher.name())).await;
        h.read(fractions(70_000));
        assert!(next_changes(&mut changes, QUIET).await.is_none(), "skipped");
        h.polkit.allow(&watcher.name());
        until(format!(
            "watcher {} passes polkit's check again",
            watcher.name()
        ))
        .await;
        let (names, state) = next_changes(&mut changes, WAIT).await.expect("the state");
        assert!(names.contains(&"Readings".to_owned()) && names.contains(&"Mode".to_owned()));
        assert_eq!(state.readings, Some(readings_of(70_000)));
    });
}

// Phase 11a Task 13, docs/spec/dbus.md, `Levels`: to each frame watcher alone, only while sound
// is attached, never with a value that isn't finite and within 0 to 1, and not once the Music
// mode is left (which closes the sound).
#[test]
fn levels_go_to_each_frame_watcher_alone_while_sound_is_attached_and_never_when_not_finite() {
    let h = Harness::start("levels", music(13), music_status(), false);
    zbus::block_on(async {
        let (watcher, bystander, plain, helper, changer) = (
            h.client().await,
            h.client().await,
            h.client().await,
            h.client().await,
            h.client().await,
        );
        let mut seen = watcher.main.receive_levels().await.expect("a stream");
        let mut unseen = bystander.main.receive_levels().await.expect("a stream");
        let mut state_only = plain.main.receive_levels().await.expect("a stream");
        watcher.main.watch_frames().await.expect("watches frames");
        plain.main.watch().await.expect("watches");
        // No sound is attached: none.
        h.levels(levels_at(1.0));
        assert_eq!(next_levels(&mut seen, QUIET).await, None, "no sound");
        let _pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        h.levels(levels_at(2.0));
        assert_eq!(
            next_levels(&mut seen, WAIT).await,
            Some(levels_at(2.0).to_vec())
        );
        assert_eq!(
            next_levels(&mut unseen, QUIET).await,
            None,
            "a match rule without a watch"
        );
        assert_eq!(
            next_levels(&mut state_only, QUIET).await,
            None,
            "a watcher of the state alone"
        );
        // Not finite, or outside 0 to 1: never sent. A good one after each shows none was.
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.001, 1.001] {
            let mut levels = levels_at(3.0);
            levels[6] = bad;
            h.levels(levels);
            assert_eq!(next_levels(&mut seen, QUIET).await, None, "{bad}");
        }
        h.levels(levels_at(4.0));
        assert_eq!(
            next_levels(&mut seen, WAIT).await,
            Some(levels_at(4.0).to_vec())
        );
        // The edges of the range are levels.
        h.levels([0.0; 14]);
        assert_eq!(next_levels(&mut seen, WAIT).await, Some(vec![0.0; 14]));
        h.levels([1.0; 14]);
        assert_eq!(next_levels(&mut seen, WAIT).await, Some(vec![1.0; 14]));
        // A watcher that left the bus is dropped; another still gets them.
        let newcomer = h.client().await;
        let mut fresh = newcomer.main.receive_levels().await.expect("a stream");
        newcomer.main.watch_frames().await.expect("watches frames");
        drop(seen);
        drop(watcher);
        h.levels(levels_at(5.0));
        assert_eq!(
            next_levels(&mut fresh, WAIT).await,
            Some(levels_at(5.0).to_vec()),
            "kept"
        );
        // The mode leaves Music: the sound closes, and none is sent any more.
        changer.main.set_mode(1).await.expect("mode 1");
        assert!(
            soon(|| h.audio.holder().is_none()).await,
            "the sound closes"
        );
        h.levels(levels_at(6.0));
        assert_eq!(next_levels(&mut fresh, QUIET).await, None, "no sound");
        newcomer.main.unwatch_frames().await.expect("stops");
    });
}

// A caller polkit refuses can't start watching, so it gets no levels; a watcher the check stops
// allowing gets none until it passes again.
#[test]
fn a_refused_levels_watcher_waits_for_its_check() {
    let h = Harness::start("levels-polkit", music(13), music_status(), false);
    zbus::block_on(async {
        let (refused, watcher, helper) = (h.client().await, h.client().await, h.client().await);
        h.polkit.refuse(&refused.name());
        let mut refused_levels = refused.main.receive_levels().await.expect("a stream");
        assert!(matches!(
            refused.main.watch_frames().await,
            Err(Error::NotAuthorized(_))
        ));
        let mut levels = watcher.main.receive_levels().await.expect("a stream");
        watcher.main.watch_frames().await.expect("watches frames");
        let _pipe = writer(helper.main.attach_audio().await.expect("a pipe"));
        h.levels(levels_at(7.0));
        assert_eq!(
            next_levels(&mut levels, WAIT).await,
            Some(levels_at(7.0).to_vec())
        );
        assert_eq!(next_levels(&mut refused_levels, QUIET).await, None);
        let h = &h;
        let until = |line: String| async move {
            for _ in 0..30 {
                if h.journal_has(&line) {
                    return;
                }
                sleep(Duration::from_millis(100)).await;
            }
            panic!("no journal line {line:?}");
        };
        h.polkit.refuse(&watcher.name());
        until(format!("watcher {} no longer passes", watcher.name())).await;
        h.levels(levels_at(8.0));
        assert_eq!(next_levels(&mut levels, QUIET).await, None, "skipped");
        h.polkit.allow(&watcher.name());
        until(format!(
            "watcher {} passes polkit's check again",
            watcher.name()
        ))
        .await;
        h.levels(levels_at(9.0));
        assert_eq!(
            next_levels(&mut levels, WAIT).await,
            Some(levels_at(9.0).to_vec()),
            "back"
        );
    });
}

// The GPU in use follows the choice, Automatic reading the first GPU; each GPU has its IDs and
// rule flag.
#[test]
fn the_state_names_the_gpu_in_use_and_its_ids() {
    let h = Harness::start(
        "gpu-in-use",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let caller = h.client().await;
        let state = || async {
            State::from_dict(&caller.main.get_state().await.expect("the state")).expect("a state")
        };
        let now = state().await;
        assert_eq!(now.chosen_gpu.as_deref(), Some(""));
        assert_eq!(now.gpu_in_use.as_deref(), Some("0000:0a:00.0"), "Automatic");
        let gpus = now.gpus.expect("Gpus");
        assert_eq!((gpus[0].vendor, gpus[0].device), (0x0abc, 0x0def));
        assert_eq!((gpus[1].vendor, gpus[1].device), (0, 0), "none in sysfs");
        assert!(
            gpus.iter().all(|g| g.has_rule),
            "amdgpu and i915 have rules"
        );
        caller
            .main
            .choose_gpu("0000:00:02.0")
            .await
            .expect("chosen");
        assert_eq!(state().await.gpu_in_use.as_deref(), Some("0000:00:02.0"));
    });
}
