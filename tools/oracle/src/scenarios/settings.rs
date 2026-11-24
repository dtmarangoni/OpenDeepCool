use super::*;

/// S14–S18, S21 and S23–S26: settings, mode switches and the device.
pub(super) fn settings_scenarios(mode: u16, unit: Option<bool>) -> Vec<Scenario> {
    use Level::{Base, High};
    let f = fahrenheit(unit);
    let mut out = Vec::new();
    out.extend(mode_switches(mode, unit));
    for &v in Variant::of(mode) {
        let roles = v.roles(mode);
        let s = |id: &str, rest: &[&str], start: Level| {
            Script::new(id, mode, unit, &detail(v, rest), &roles, start)
        };
        out.extend(unit_changes(mode, unit, v));
        for (start, label) in [(Base, "static"), (High, "scrolling")] {
            let mut turns = s("S16", &[label], start);
            for (tick, d) in [(2, 1), (4, 1), (6, 1), (8, 1), (10, 2), (13, 3), (16, 1)] {
                turns = turns.at(tick, [Step::Reset(Cause::Rotation(d))]);
            }
            out.push(turns.end(End::Total(20)));
        }
        out.push(s("S17", &["99 or less"], Base).end(End::Total(3)));
        out.push(s("S17", &["100 or more"], High).end(End::Total(23)));
        let rates_away = || {
            RATES
                .iter()
                .map(|&m| Step::Set(m, "unavailable".to_owned()))
                .collect::<Vec<_>>()
        };
        let rates_back = |start: Level| {
            RATES
                .iter()
                .map(|&m| Step::Set(m, start_value(m, f, &roles, start)))
                .collect::<Vec<_>>()
        };
        out.push(
            s("S18", &["the cooler away"], High)
                .at(5, [Step::CoolerAway])
                .at(10, [Step::CoolerBack])
                .end(End::Total(25)),
        );
        out.push(
            s("S18", &["wake-up, node still there"], High)
                .at(7, [Step::Wake])
                .at(7, rates_away())
                .at(17, rates_back(High))
                .end(End::Total(30)),
        );
        out.push(
            s("S18", &["wake-up, node gone"], High)
                .at(7, [Step::Wake, Step::CoolerAway])
                .at(7, rates_away())
                .at(12, [Step::CoolerBack])
                .at(17, rates_back(High))
                .end(End::Total(30)),
        );
        out.push(
            s("S21", &["rotation"], High)
                .at(6, [Step::Reset(Cause::Rotation(1))])
                .set(6, &roles, Base)
                .end(End::Total(10)),
        );
        let next = next_in_kind(mode);
        out.push(
            s("S21", &[&format!("into mode {next}")], High)
                .at(6, [Step::Reset(Cause::Mode(next))])
                .set(6, &roles, Base)
                .end(End::Total(10)),
        );
        for (start, label) in [(Base, "static"), (High, "scrolling")] {
            // A mode outside the table (owner, 2026-11-05): from Phase 9 every mode is drawn, so
            // none stays "not yet available". The two numbers outside it are 17, the retired
            // Emoji 4, and 27, the one past the last (music.md; owner, 2026-11-23).
            out.push(
                s("S23", &[label], start)
                    .at(
                        3,
                        [
                            Step::Refused(17, "not in the mode table"),
                            Step::Refused(27, "not in the mode table"),
                        ],
                    )
                    .at(6, [Step::Refused(refused_unavailable(mode), "unavailable")])
                    .end(End::Total(10)),
            );
        }
        let already = || {
            ["mode", "unit", "rotation", "GPU"]
                .into_iter()
                .map(Step::AlreadySet)
                .collect::<Vec<_>>()
        };
        out.push(
            s("S24", &["static"], Base)
                .at(3, already())
                .end(End::Total(6)),
        );
        // Another GPU brings its own readings for every GPU metric.
        let another_gpu = |scrolling: bool| -> Vec<Step> {
            let mut steps = vec![Step::Reset(Cause::AnotherGpu)];
            for (metric, static_, high) in ANOTHER_GPU {
                let value = if scrolling && roles.contains(&metric) {
                    high
                } else {
                    static_
                };
                steps.push(Step::Set(metric, value.to_owned()));
            }
            steps
        };
        out.push(
            s("S25", &["static"], Base)
                .at(3, another_gpu(false))
                .end(End::Total(6)),
        );
        let fresh = |base: Level| -> Vec<Step> {
            roles
                .iter()
                .map(|&m| Step::Set(m, offset(m, f, base, 2, 1)))
                .collect()
        };
        out.push(
            s("S26", &["static"], Base)
                .at(3, [Step::Reset(Cause::Refresh)])
                .at(3, fresh(Base))
                .end(End::Total(6)),
        );
        out.push(
            s("S26", &["scrolling, then 99 or less"], High)
                .at(5, [Step::Reset(Cause::Refresh)])
                .set(5, &roles, Base)
                .end(End::Total(9)),
        );
        for k in 0..21 {
            let t = tick_at(k, 21);
            let at_k = format!("k {k}");
            out.push(
                s("S24", &["scrolling", &at_k], High)
                    .at(t, already())
                    .end(End::Total(t + 4)),
            );
            out.push(
                s("S25", &["scrolling", &at_k], High)
                    .at(t, another_gpu(true))
                    .end(End::Total(t + 4)),
            );
            out.push(
                s("S26", &["scrolling", &at_k], High)
                    .at(t, [Step::Reset(Cause::Refresh)])
                    .at(t, fresh(High))
                    .end(End::Total(t + 4)),
            );
        }
    }
    // S17's first second: a reading averaged over the interval shows dashes until its second
    // sample (display.md, Unavailable values).
    for rate in RATES.into_iter().filter(|m| shown(mode).contains(m)) {
        out.push(
            Script::with(
                "S17",
                mode,
                unit,
                &["the first second's dashes, then 100 or more".to_owned()],
                &[(rate, "unavailable".to_owned())],
            )
            .set(10, &[rate], Level::High)
            .end(End::Total(32)),
        );
    }
    out
}

/// S15: a unit change. In a mode that shows a temperature it resets, like a mode switch: static
/// and scrolling, `57°C` ↔ `135°F`, and at every k of a scrolling strip. In any other mode it's
/// only saved: static and scrolling, and in Power at every k too.
pub(super) fn unit_changes(mode: u16, unit: Option<bool>, v: Variant) -> Vec<Scenario> {
    use Level::{Base, High};
    let roles = v.roles(mode);
    let s = |rest: &[&str], start: Level| {
        Script::new("S15", mode, unit, &detail(v, rest), &roles, start)
    };
    let mut out = Vec::new();
    let (there, back) = if shows_temperature(mode) {
        let f = fahrenheit(unit);
        (Step::Reset(Cause::Unit(!f)), Step::Reset(Cause::Unit(f)))
    } else {
        (Step::UnitSaved(true), Step::UnitSaved(false))
    };
    for (start, label) in [(Base, "static"), (High, "scrolling")] {
        out.push(
            s(&[label], start)
                .at(3, [there.clone()])
                .at(8, [back.clone()])
                .end(End::Total(12)),
        );
    }
    if shows_temperature(mode) {
        // 57 °C is static and 135 °F scrolls; the other value of mode 9 at 56 °C, 133 °F.
        let readings: Vec<(&'static str, String)> = roles
            .iter()
            .zip(["57", "56"])
            .map(|(&m, value)| (m, value.to_owned()))
            .collect();
        out.push(
            Script::with("S15", mode, unit, &detail(v, &["57 °C, 135 °F"]), &readings)
                .at(3, [there.clone()])
                .at(8, [back.clone()])
                .end(End::Total(30)),
        );
    }
    if kind(mode) != Kind::Usage {
        for k in 0..21 {
            let t = tick_at(k, 21);
            out.push(
                s(&["scrolling", &format!("k {k}")], High)
                    .at(t, [there.clone()])
                    .end(End::Total(t + 5)),
            );
        }
    }
    out
}

/// The reading `metric` starts at in a scenario whose `roles` start at `start`.
pub(super) fn start_value(
    metric: &'static str,
    fahrenheit: bool,
    roles: &[&'static str],
    start: Level,
) -> String {
    if roles.contains(&metric) {
        level(metric, fahrenheit, start)
    } else {
        level(metric, fahrenheit, Level::Base)
    }
}
