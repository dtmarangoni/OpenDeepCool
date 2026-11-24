use super::*;

// Phase 10 plan, Task 2, through the whole service: a value of 100 or more scrolls, so the frame
// changes every tick: about ten `Frame` signals a second, each a frame of its cycle, none twice
// in a row. They come with no cooler at all, so the preview shows what the cooler would (D20).
#[test]
fn a_scrolling_value_gives_ten_frames_a_second_even_with_no_cooler() {
    let s = Service::start("frames-scroll", vec![Presence::Absent], true);
    let reading = |m: Metric| {
        if m == Metric::CpuUsage {
            100
        } else {
            13 + 7 * m as i64
        }
    };
    zbus::block_on(async {
        let client = Client::on(&s.bus).await;
        client.dev.begin_test().await.expect("the session");
        let values: HashMap<String, (i64, u64)> = Metric::ALL
            .into_iter()
            .map(|m| (m.spec_name().to_owned(), (reading(m), 1)))
            .collect();
        client
            .dev
            .override_readings(values, HashMap::new())
            .await
            .expect("the override");
        let mut frames = client.main.receive_frame().await.expect("a stream");
        client.main.watch_frames().await.expect("watches");
        let start = std::time::Instant::now();
        let mut got = Vec::new();
        while start.elapsed() < Duration::from_secs(2) {
            if let Some(frame) = next_frame(&mut frames, Duration::from_millis(300)).await {
                got.push(frame);
            }
        }
        assert!(
            (17..=23).contains(&got.len()),
            "{} frames in 2 s",
            got.len()
        );
        let snapshot = Snapshot::new(|m| value(reading(m)));
        let mut engine = Engine::new(Settings::default(), snapshot);
        let cycle: Vec<Frame> = (0..60)
            .filter_map(|_| engine.tick(None, None, false).frame)
            .collect();
        assert!(got.iter().all(|f| cycle.contains(f)), "frames of the cycle");
        assert!(got.windows(2).all(|w| w[0] != w[1]), "one per change");
    });
}
