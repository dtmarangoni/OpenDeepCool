use super::*;

// The scenarios S1–S26 of each mode with values, the change in the top value, the bottom one
// and both, every k, in each unit where a temperature is shown: S14–S18 and S21–S26 above all,
// which only the engine can play out. Also those of each Music mode,
// with their sound and Music settings, and the switches into it from modes 1–12. The engine
// analyses each tick's sound itself, as the service does, so each mode's file plays on a thread
// of its own.
#[test]
fn every_scenario_of_every_mode_plays_out_through_the_engine_in_all_four_rotations() {
    let counts: Vec<usize> = std::thread::scope(|scope| {
        let threads: Vec<_> = Mode::all()
            .map(Mode::number)
            .map(|mode| {
                scope.spawn(move || {
                    let scenarios = mode_scenarios(mode);
                    assert!(!scenarios.is_empty(), "mode {mode}: no scenario");
                    for scenario in &scenarios {
                        for base in 0..4 {
                            replay(scenario, base);
                        }
                    }
                    scenarios.len()
                })
            })
            .collect();
        threads
            .into_iter()
            .map(|t| t.join().expect("a mode's scenarios replayed"))
            .collect()
    });
    let replayed: usize = counts.iter().sum();
    println!("{replayed} scenarios replayed");
    assert_eq!(
        replayed,
        10_403 + 18_194,
        "the scenarios of modes 1–12 and of the Music modes 13–16 and 18–26"
    );
}
