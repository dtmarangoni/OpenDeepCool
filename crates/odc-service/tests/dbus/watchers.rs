use super::*;

#[test]
fn a_watcher_that_stops_passing_is_skipped_then_gets_the_whole_state() {
    let h = Harness::start(
        "recheck",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (watcher, changer) = (h.client().await, h.client().await);
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        h.polkit.refuse(&watcher.name());
        // The check each second sees it.
        sleep(Duration::from_millis(1300)).await;
        changer.main.set_mode(3).await.expect("mode 3");
        assert!(
            within(Duration::from_millis(600), next(&mut changes))
                .await
                .is_none(),
            "skipped while it fails"
        );
        assert!(h.journal_has(&format!("watcher {} no longer passes", watcher.name())));
        h.polkit.allow(&watcher.name());
        let signal = within(WAIT, next(&mut changes))
            .await
            .flatten()
            .expect("the whole state");
        let changes = signal.args().expect("its changes").changes;
        // Every key but `Readings`, which only a watcher of frames is sent.
        assert_eq!(
            changes.len(),
            STATE_KEYS.len() - 1,
            "every other key in one signal"
        );
        assert!(!changes.contains_key("Readings"), "not to this watcher");
        assert_eq!(State::from_dict(&changes).expect("a state").mode, Some(3));
    });
}

#[test]
fn a_wake_up_comes_only_from_the_connection_that_owns_logind_s_name() {
    let h = Harness::start(
        "wake",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let (logind, faker) = (h.client().await, h.client().await);
        logind
            .conn
            .request_name("org.freedesktop.login1")
            .await
            .expect("logind's name");
        let prepare = |conn: &Connection, to: Option<&'static str>, start: bool| {
            let conn = conn.clone();
            async move {
                conn.emit_signal(
                    to,
                    "/org/freedesktop/login1",
                    "org.freedesktop.login1.Manager",
                    "PrepareForSleep",
                    &start,
                )
                .await
                .expect("sent");
            }
        };
        // From another connection, broadcast or addressed to the service: nothing.
        prepare(&faker.conn, None, false).await;
        prepare(&faker.conn, Some(BUS_NAME), false).await;
        // From logind, before a sleep: nothing.
        prepare(&logind.conn, None, true).await;
        sleep(QUIET).await;
        assert!(
            h.device.try_recv().is_err(),
            "no Wake for the device thread"
        );
        assert_eq!(h.apply.take(), None, "no wake-up for the engine");
        // From logind, after a sleep.
        prepare(&logind.conn, None, false).await;
        let mut woke = false;
        for _ in 0..30 {
            if h.device.try_recv() == Ok(DeviceControl::Wake) {
                woke = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        assert!(woke, "Wake reaches the device thread");
        let applied = h.apply.take().expect("the engine is told");
        assert!(
            applied.wake,
            "the engine restarts the sensor thread's intervals"
        );
        assert!(h.device.try_recv().is_err(), "one Wake");
        assert!(h.journal_has("back from standby"));
    });
}
