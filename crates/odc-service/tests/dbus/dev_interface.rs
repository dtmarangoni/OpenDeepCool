use super::*;

#[test]
fn the_dev_interface_takes_only_the_service_s_group() {
    let h = Harness::start(
        "dev-group",
        Settings::default(),
        status(readings(value(51))),
        true,
    );
    zbus::block_on(async {
        let (outsider, unknown, member) = (h.client().await, h.client().await, h.client().await);
        h.credentials.give(
            &outsider.name(),
            Identity {
                user: Some(USER),
                groups: Some(vec![GROUP + 1]),
                pid: Some(PID),
            },
        );
        h.credentials.give(
            &unknown.name(),
            Identity {
                user: Some(USER),
                groups: None,
                pid: Some(PID),
            },
        );
        assert!(matches!(
            outsider.dev.begin_test().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(
            matches!(unknown.dev.begin_test().await, Err(Error::NotAuthorized(_))),
            "a missing group list means no"
        );
        // A method call without an interface field (dbus-daemon(1): the field is optional).
        let raw = outsider
            .conn
            .call_method(Some(BUS_NAME), DEV_PATH, None::<&str>, "BeginTest", &())
            .await;
        assert!(raw.is_err(), "{raw:?}");
        // No session began: a member of the group takes it.
        member.dev.begin_test().await.expect("the session");
        assert!(h.journal_has(&format!("held by {}", member.name())));
    });
}

#[test]
fn the_test_session_holds_the_service_until_its_holder_leaves() {
    let h = Harness::start(
        "session",
        Settings::default(),
        status(readings(value(51))),
        true,
    );
    zbus::block_on(async {
        let (holder, other) = (h.client().await, h.client().await);
        let values = HashMap::from([("CPU Usage".to_owned(), (42, 1))]);
        let none = HashMap::new();
        // Without a session.
        assert!(matches!(
            holder
                .dev
                .override_readings(values.clone(), none.clone())
                .await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            holder.dev.clear_override().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            holder.dev.release_cooler().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            holder.dev.end_test().await,
            Err(Error::NotAuthorized(_))
        ));
        assert!(matches!(
            holder.dev.durations().await,
            Err(Error::NotAuthorized(_))
        ));
        // A session starts the durations afresh (owner, 2026-11-04).
        h.durations.write(Duration::from_micros(9));
        h.durations.snapshot(Duration::from_micros(9));
        holder.dev.begin_test().await.expect("the session");
        holder.dev.begin_test().await.expect("again: no change");
        h.durations.write(Duration::from_micros(1_234));
        h.durations.snapshot(Duration::from_millis(56));
        assert_eq!(
            holder.dev.durations().await.expect("the durations"),
            (vec![1_234], vec![56_000])
        );
        assert!(matches!(other.dev.durations().await, Err(Error::Busy(_))));
        // Everyone else is refused.
        assert!(matches!(other.main.set_mode(3).await, Err(Error::Busy(_))));
        assert!(matches!(other.main.refresh().await, Err(Error::Busy(_))));
        assert!(matches!(other.dev.begin_test().await, Err(Error::Busy(_))));
        assert!(matches!(
            other
                .dev
                .override_readings(values.clone(), none.clone())
                .await,
            Err(Error::Busy(_))
        ));
        assert!(matches!(
            other.dev.release_cooler().await,
            Err(Error::Busy(_))
        ));
        assert!(matches!(other.dev.end_test().await, Err(Error::Busy(_))));
        // Reads aren't changes.
        other.main.get_state().await.expect("the state");
        // The holder may change.
        holder.main.set_mode(3).await.expect("mode 3");
        assert_eq!(
            h.apply.take().and_then(|a| a.settings).map(|s| s.mode),
            Some(3)
        );
        // Overrides out of form.
        let all = |except: Option<Metric>| -> HashMap<String, (i64, u64)> {
            Metric::ALL
                .into_iter()
                .filter(|m| Some(*m) != except)
                .map(|m| (m.spec_name().to_owned(), (40 + m as i64, 1)))
                .collect()
        };
        let bad: Vec<(Values, Reasons)> = vec![
            (all(Some(Metric::MemoryLoad)), HashMap::new()),
            (
                all(None),
                HashMap::from([("Memory Load".to_owned(), "no-device".to_owned())]),
            ),
            (
                {
                    let mut v = all(Some(Metric::MemoryLoad));
                    v.insert("RAM".to_owned(), (5, 1));
                    v
                },
                HashMap::new(),
            ),
            (
                all(Some(Metric::MemoryLoad)),
                HashMap::from([("Memory Load".to_owned(), "gone".to_owned())]),
            ),
            (
                {
                    let mut v = all(None);
                    v.insert("CPU Usage".to_owned(), (5, 0));
                    v
                },
                HashMap::new(),
            ),
        ];
        for (i, (values, unavailable)) in bad.into_iter().enumerate() {
            assert!(
                matches!(
                    holder.dev.override_readings(values, unavailable).await,
                    Err(Error::InvalidArgument(_))
                ),
                "{i}"
            );
        }
        assert_eq!(h.apply.take(), None, "no override posted");
        // The holder's connection closes, as when the test is killed.
        let gone = holder.name();
        drop(holder);
        let mut passed = false;
        for _ in 0..30 {
            if other.main.set_mode(2).await.is_ok() {
                passed = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        assert!(passed, "changes pass again once the holder left");
        assert!(h.journal_has(&format!("the test session of {gone} ended")));
        let cleared = h
            .apply
            .take()
            .and_then(|a| a.readings)
            .expect("an override's end");
        assert_eq!(cleared.readings, None);
    });
}

#[test]
fn watchers_are_capped_per_user_and_in_all_and_dropped_when_they_leave() {
    let h = Harness::start(
        "caps",
        Settings::default(),
        status(readings(value(51))),
        false,
    );
    zbus::block_on(async {
        let mut clients = Vec::new();
        for _ in 0..33 {
            clients.push(h.client().await);
        }
        // The first nine share a user; the rest have one each.
        for (i, client) in clients.iter().enumerate() {
            let user = if i < 9 { 2000 } else { 3000 + i as u32 };
            h.credentials.give(
                &client.name(),
                Identity {
                    user: Some(user),
                    groups: Some(vec![]),
                    pid: Some(PID),
                },
            );
        }
        for client in &clients[..8] {
            client.main.watch().await.expect("watches");
        }
        clients[0]
            .main
            .watch()
            .await
            .expect("a second Watch() changes nothing");
        assert!(matches!(
            clients[8].main.watch().await,
            Err(Error::TooManyWatchers(_))
        ));
        for client in &clients[9..33] {
            client.main.watch().await.expect("watches");
        }
        // 8 + 24 = 32 in all.
        let late = h.client().await;
        assert!(matches!(
            late.main.watch().await,
            Err(Error::TooManyWatchers(_))
        ));
        // One leaves: its place is free.
        clients.truncate(32);
        let mut passed = false;
        for _ in 0..30 {
            if late.main.watch().await.is_ok() {
                passed = true;
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        assert!(passed, "a watcher whose connection closed is dropped");
    });
}
