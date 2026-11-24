use super::*;

// P7-06: Refresh's reset lands on the first tick that has the fresh snapshot, never on an older
// one that arrives meanwhile: when its answer is late, a tick or more after the Refresh.
#[test]
fn refresh_resets_on_the_first_tick_with_its_snapshot() {
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(100)));
    for _ in 0..5 {
        engine.tick(None, None, false);
    }
    let taken = engine.tick(Some(refresh(2)), None, false);
    assert_ne!(taken.frame, Some(home(1, &usage(100))), "no answer yet");
    // An older snapshot, with a new reading: only an update, the strip keeps moving.
    let old = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&usage(101)),
            answers: 1,
        }),
        false,
    );
    assert_ne!(old.frame, Some(home(1, &usage(101))));
    let reset = engine.tick(
        None,
        Some(Published {
            snapshot: snapshot(&usage(102)),
            answers: 2,
        }),
        false,
    );
    assert_eq!(reset.frame, Some(home(1, &usage(102))));
}

// S26 (owner, 2026-11-06): the D-Bus side asks for the snapshot as the call arrives, so its answer
// normally comes before the tick that takes the Refresh, which resets: the next frame is home.
// An answer the engine took as a plain snapshot a tick before the Refresh reached it counts too.
#[test]
fn refresh_resets_on_the_tick_that_takes_it_when_its_answer_came_first() {
    let mut engine = Engine::new(Settings::default(), snapshot(&usage(100)));
    for _ in 0..5 {
        engine.tick(None, None, false);
    }
    let answer = |answers, n| Published {
        snapshot: snapshot(&usage(n)),
        answers,
    };
    let same = engine.tick(Some(refresh(1)), Some(answer(1, 101)), false);
    assert_eq!(same.frame, Some(home(1, &usage(101))));
    for _ in 0..5 {
        engine.tick(None, None, false);
    }
    let first = engine.tick(None, Some(answer(2, 102)), false);
    assert_ne!(first.frame, Some(home(1, &usage(102))), "only an update");
    let taken = engine.tick(Some(refresh(2)), None, false);
    assert_eq!(taken.frame, Some(home(1, &usage(102))));
}

// Another GPU waits for a snapshot of it, so the first frame after the switch never shows the old
// GPU's values (Phase 7 plan, Task 6).
#[test]
fn another_gpu_resets_with_the_new_gpu_s_values() {
    let gpu = |usage: i64| {
        readings(&[
            ("CPU Usage", Some(42)),
            ("Gpu usage", Some(usage)),
            ("Memory Load", Some(85)),
        ])
    };
    let mode2 = Settings {
        mode: 2,
        ..Settings::default()
    };
    let choose = |pci: &str, asked| Apply {
        settings: Some(Settings {
            gpu_choice: Some(pci.to_owned()),
            ..mode2.clone()
        }),
        asked: Some(asked),
        ..Apply::default()
    };
    let answer = |answers, usage| Published {
        snapshot: snapshot(&gpu(usage)),
        answers,
    };
    let mut engine = Engine::new(mode2.clone(), snapshot(&gpu(67)));
    engine.tick(None, None, false);
    // Its answer late: no reset until it comes.
    let switch = engine.tick(Some(choose("0000:0b:00.0", 1)), None, false);
    assert_eq!(switch.frame, Some(home(2, &gpu(67))));
    let next = engine.tick(None, Some(answer(1, 12)), false);
    assert_eq!(next.frame, Some(home(2, &gpu(12))));
    // Its answer on time, as the D-Bus side asks at once (owner, 2026-11-06): the next frame.
    let mut engine = Engine::new(mode2.clone(), snapshot(&gpu(100)));
    for _ in 0..5 {
        engine.tick(None, None, false);
    }
    let switch = engine.tick(Some(choose("0000:0b:00.0", 1)), Some(answer(1, 101)), false);
    assert_eq!(switch.frame, Some(home(2, &gpu(101))));
    // P7-15: another GPU and back before a tick is no change: no reset, though answered.
    let at_b = || {
        let mut engine = Engine::new(
            Settings {
                gpu_choice: Some("0000:0b:00.0".to_owned()),
                ..mode2.clone()
            },
            snapshot(&gpu(100)),
        );
        for _ in 0..5 {
            engine.tick(None, None, false);
        }
        engine
    };
    let (mut changed, mut alone) = (at_b(), at_b());
    let back = changed.tick(Some(choose("0000:0b:00.0", 2)), Some(answer(2, 101)), false);
    assert_ne!(back.frame, Some(home(2, &gpu(101))), "no reset");
    assert_eq!(
        back.frame,
        alone.tick(None, Some(answer(2, 101)), false).frame
    );
}
