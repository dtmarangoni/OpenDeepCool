use super::*;

/// An emoji loop as the picture of each 100 ms tick (music.md, Emoji animations).
pub(super) fn emoji_ticks(mode: u16) -> Vec<usize> {
    emoji_loop(mode)
        .iter()
        .flat_map(|&(picture, ms)| std::iter::repeat_n(picture, (ms / 100) as usize))
        .collect()
}

/// The ticks at which a change comes in a Music mode without values: every tick of a burst's
/// period in 13, every loop position in 14–16 and 24–26, the first ten ticks of the beat train in
/// 23.
pub(super) fn positions(mode: u16) -> Vec<u32> {
    let cycle = match mode {
        13 => 5,
        14..=16 | 24..=26 => u32::try_from(emoji_ticks(mode).len()).expect("a short loop"),
        _ => return (1..=10).collect(),
    };
    (0..cycle).map(|p| tick_at(p, cycle)).collect()
}

/// S14–S18, S25 and S26 for a Music mode without values (display.md, Which modes): switches into
/// every other mode, rotation changes, another GPU and Refresh at every position, a unit change
/// (only saved), start-up and the device, while its sound plays.
pub(super) fn still_scenarios(mode: u16) -> Vec<Scenario> {
    let own = music_audio(mode).expect("a Music mode");
    let s = |id: &str, detail: &[String]| {
        Script::with(id, mode, None, detail, &[]).at(0, [Step::Audio(own)])
    };
    let at = |t: u32| format!("at tick {t}");
    let mut out = Vec::new();
    for target in MODES.into_iter().filter(|&t| t != mode) {
        let audio = if target == 23 { BEATS } else { own };
        let sets: &[SwitchSet] = if value_mode(target).is_some() {
            &SWITCH_SETS
        } else {
            &SWITCH_SETS[..1]
        };
        for &u in units(target) {
            for &(label, high) in sets {
                let readings: Vec<(&'static str, String)> = METRICS
                    .iter()
                    .map(|&m| {
                        let level_ = if high(m) { Level::High } else { Level::Base };
                        (m, level(m, fahrenheit(u), level_))
                    })
                    .collect();
                for t in positions(mode) {
                    let detail = [format!("into mode {target}"), label.to_owned(), at(t)];
                    out.push(
                        Script::with("S14", mode, u, &detail, &readings)
                            .at(0, [Step::Audio(audio)])
                            .at(t, [Step::Reset(Cause::Mode(target))])
                            .end(End::Total(t + 5)),
                    );
                }
            }
        }
    }
    out.push(
        s("S15", &[])
            .at(3, [Step::UnitSaved(true)])
            .at(8, [Step::UnitSaved(false)])
            .end(End::Total(12)),
    );
    for (i, t) in positions(mode).into_iter().enumerate() {
        let turns = u8::try_from(i % 3 + 1).expect("1 to 3");
        out.push(
            s("S16", &[at(t)])
                .at(t, [Step::Reset(Cause::Rotation(turns))])
                .end(End::Total(t + 5)),
        );
    }
    let quiet = Script::with("S17", mode, None, &["in silence".to_owned()], &[]);
    out.push(quiet.end(End::Total(12)));
    out.push(s("S17", &["with sound".to_owned()]).end(End::Total(12)));
    let rates = |value: &str| -> Vec<Step> {
        RATES
            .iter()
            .map(|&m| Step::Set(m, value.to_owned()))
            .collect()
    };
    let base = |m: &'static str| level(m, false, Level::Base);
    let rates_back: Vec<Step> = RATES.iter().map(|&m| Step::Set(m, base(m))).collect();
    out.push(
        s("S18", &["the cooler away".to_owned()])
            .at(5, [Step::CoolerAway])
            .at(10, [Step::CoolerBack])
            .end(End::Total(25)),
    );
    out.push(
        s("S18", &["wake-up, node still there".to_owned()])
            .at(7, [Step::Wake])
            .at(7, rates("unavailable"))
            .at(17, rates_back.clone())
            .end(End::Total(30)),
    );
    out.push(
        s("S18", &["wake-up, node gone".to_owned()])
            .at(7, [Step::Wake, Step::CoolerAway])
            .at(7, rates("unavailable"))
            .at(12, [Step::CoolerBack])
            .at(17, rates_back)
            .end(End::Total(30)),
    );
    for t in positions(mode) {
        let mut another_gpu = vec![Step::Reset(Cause::AnotherGpu)];
        for (metric, value, _) in ANOTHER_GPU {
            another_gpu.push(Step::Set(metric, value.to_owned()));
        }
        out.push(s("S25", &[at(t)]).at(t, another_gpu).end(End::Total(t + 5)));
    }
    for t in positions(mode) {
        out.push(
            s("S26", &[at(t)])
                .at(t, [Step::Reset(Cause::Refresh)])
                .end(End::Total(t + 5)),
        );
    }
    out
}

/// S15b: the Music settings, each stop and the peak dots, changed while the sound plays: nothing
/// resets, and the spectrum shows the change on the next frame (music.md point 7). Where nothing
/// draws a spectrum they're only saved, and the next spectrum draws them.
pub(super) fn music_settings(mode: u16) -> Vec<Scenario> {
    use Step::{MusicBarFall as Bar, MusicPeakFall as Peak, MusicPeaks as Peaks};
    let mut out = Vec::new();
    if has_spectrum(mode) {
        let mut changes = Script::with("S15b", mode, units(mode)[0], &[], &[])
            .at(0, music_audio(mode).map(Step::Audio))
            .at(3, [Peaks(false)])
            .at(8, [Peaks(true)]);
        for (tick, bar, peak) in [
            (11, "slower", "slower"),
            (16, "faster", "faster"),
            (21, "slow", "moderate"),
            (26, "fast", "fast"),
            (31, "moderate", "slow"),
        ] {
            changes = changes.at(tick, [Bar(bar), Peak(peak)]);
        }
        out.push(changes.end(End::Total(40)));
    }
    if mode == 13 || mode == 23 || is_emoji_alone(mode) {
        let from = if mode == 13 { 1 } else { mode };
        out.push(
            Script::with(
                "S15b",
                from,
                None,
                &["only saved, then into mode 13".to_owned()],
                &[],
            )
            .at(0, music_audio(from).or(Some(BURST)).map(Step::Audio))
            .at(2, [Peaks(false), Bar("faster"), Peak("faster")])
            .at(4, [Step::Reset(Cause::Mode(13))])
            .end(End::Total(15)),
        );
    }
    out
}
