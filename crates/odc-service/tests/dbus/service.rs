use super::*;

#[test]
fn an_override_applies_from_the_tick_it_returns_and_a_release_hands_the_cooler_over() {
    let s = Service::start("release", vec![Presence::Present], true);
    zbus::block_on(async {
        let (holder, reader) = (Client::on(&s.bus).await, Client::on(&s.bus).await);
        holder.dev.begin_test().await.expect("the session");
        let values: HashMap<String, (i64, u64)> = Metric::ALL
            .into_iter()
            .map(|m| (m.spec_name().to_owned(), (13 + 7 * m as i64, 1)))
            .collect();
        let tick = holder
            .dev
            .override_readings(values, HashMap::new())
            .await
            .expect("the override's tick");
        let overridden = Snapshot::new(|m| value(13 + 7 * m as i64));
        // The recording is flushed once a second.
        sleep(Duration::from_millis(1500)).await;
        let from_tick: Vec<Frame> = read_recording(&s.recording)
            .filter(|r| r.tick >= tick)
            .map(|r| {
                match decode(r.bytes.as_slice().try_into().expect("64 bytes")).expect("a report") {
                    Message::Frame(frame) => frame,
                    Message::Startup => panic!("03 08 after the start"),
                }
            })
            .collect();
        assert!(from_tick.len() >= 5, "{}", from_tick.len());
        let want = home(overridden);
        assert!(from_tick.iter().all(|f| *f == want), "from tick {tick} on");
        // The threads timed each write and snapshot of the session (owner, 2026-11-04).
        let (writes, snapshots) = holder.dev.durations().await.expect("the durations");
        assert!(writes.len() >= from_tick.len(), "{}", writes.len());
        assert!(!snapshots.is_empty());
        // The release: the connection closes and nothing more is written.
        holder.dev.release_cooler().await.expect("released");
        assert_eq!(s.log.open(), 0, "the cooler's node is closed");
        let state =
            State::from_dict(&reader.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(state.cooler_state, Some(CoolerState::Released));
        assert_eq!(state.cooler_connected, Some(false));
        let written = s.messages();
        sleep(QUIET).await;
        assert_eq!(s.messages(), written, "nothing written while released");
        // The holder's connection closes: the next report is 03 08, then the real readings,
        // from the tick the engine took the override's end, at most a tick later.
        let connections = s.log.connections().len();
        drop(holder);
        let mut back = false;
        for _ in 0..30 {
            sleep(Duration::from_millis(100)).await;
            if s.log.connections().len() > connections
                && s.log.connections().last().is_some_and(|c| c.len() >= 5)
            {
                back = true;
                break;
            }
        }
        assert!(back, "the service takes the cooler back");
        let last = s.log.connections().pop().expect("a connection");
        assert_eq!(last[0], Message::Startup, "03 08 first");
        assert!(
            matches!(last[1], Message::Frame(_)),
            "then that tick's frame"
        );
        assert_eq!(
            last.last(),
            Some(&Message::Frame(home(readings(value(51))))),
            "the override ended with the session"
        );
        assert!(
            s.journal
                .lines()
                .iter()
                .any(|(_, l)| l.contains("taken back"))
        );
    });
}

#[test]
fn the_journal_says_the_readings_the_first_frame_is_drawn_from() {
    let s = Service::start("first-readings", vec![Presence::Present], false);
    let lines = s.journal.lines();
    let line = lines
        .iter()
        .map(|(_, l)| l.as_str())
        .find(|l| l.starts_with(FIRST_READINGS))
        .expect("a first readings line");
    let first = parse_first_readings(line).expect("the line reads back");
    let want = readings(value(51));
    assert!(
        first
            .iter()
            .all(|(metric, reading)| reading == want.get(*metric)),
        "{line}"
    );
}

#[test]
fn cooler_state_follows_the_cooler_once_per_change() {
    let mut script = vec![Presence::Absent; 20];
    script.extend(vec![
        Presence::NoAccess(PathBuf::from("/dev/hidraw-fake")),
        Presence::NoAccess(PathBuf::from("/dev/hidraw-fake")),
        Presence::FailsAfter(4),
        Presence::Present,
    ]);
    let s = Service::start("cooler-state", script, false);
    zbus::block_on(async {
        let watcher = Client::on(&s.bus).await;
        watcher.main.watch().await.expect("watches");
        let mut changes = watcher
            .main
            .receive_state_changed()
            .await
            .expect("a stream");
        let first =
            State::from_dict(&watcher.main.get_state().await.expect("the state")).expect("a state");
        assert_eq!(first.cooler_state, Some(CoolerState::NotFound));
        let mut seen = Vec::new();
        while seen.last() != Some(&CoolerState::Connected) || seen.len() < 4 {
            let Some(Some(signal)) = within(Duration::from_secs(5), next(&mut changes)).await
            else {
                break;
            };
            let changes = signal.args().expect("its changes").changes;
            if let Some(state) = State::from_dict(&changes).expect("a state").cooler_state {
                seen.push(state);
            }
        }
        assert_eq!(
            seen,
            [
                CoolerState::NoAccess,
                CoolerState::Connected,
                CoolerState::WriteFailed,
                CoolerState::Connected
            ]
        );
    });
    let lines: Vec<String> = s
        .journal
        .lines()
        .into_iter()
        .map(|(_, line)| line)
        .filter(|line| line.contains("cooler"))
        .collect();
    assert_eq!(
        lines,
        [
            "no cooler is attached",
            // The fake's error has no errno.
            "the cooler at /dev/hidraw-fake can't be opened",
            "the cooler is connected",
            "a write to the cooler failed",
            "the cooler is connected",
        ]
    );
}
