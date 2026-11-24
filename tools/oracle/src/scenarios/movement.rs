use super::*;

/// S1–S13, S19, S20 and S22: values and movement.
pub(super) fn movement_scenarios(mode: u16, unit: Option<bool>) -> Vec<Scenario> {
    use Level::{Base, Four, High, Max, OneDigit};
    let f = fahrenheit(unit);
    let mut out = Vec::new();
    let pair = shown(mode).len() == 2;
    for &v in Variant::of(mode) {
        let roles = v.roles(mode);
        let s = |id: &str, rest: &[&str], start: Level| {
            Script::new(id, mode, unit, &detail(v, rest), &roles, start)
        };
        let sv = |id: &str, rest: &[&str], value: &str| {
            Script::literal(id, mode, unit, &detail(v, rest), &roles, value)
        };
        let set = |metric: &'static str, value: &str| Step::Set(metric, value.to_owned());
        out.push(s("S1", &[], High).end(End::Total(43)));
        for (from, label) in [(Base, "from two digits"), (OneDigit, "from one digit")] {
            if f && label == "from one digit" {
                continue;
            }
            out.push(
                s("S2", &[label], from)
                    .set(3, &roles, High)
                    .end(End::Total(27)),
            );
        }
        out.push(
            s("S3", &[], High)
                .set(5, &roles, Base)
                .end(End::UntilStopped(3)),
        );
        out.push(
            s("S4", &[], High)
                .set(5, &roles, Base)
                .set(10, &roles, High)
                .end(End::Total(53)),
        );
        out.push(
            s("S5", &[], Base)
                .set(2, &roles, High)
                .set(6, &roles, Base)
                .set(10, &roles, High)
                .set(14, &roles, Base)
                .end(End::UntilStopped(3)),
        );
        for k in 0..21 {
            out.push(
                s("S6", &[&format!("k {k}")], High)
                    .set(tick_at(k, 21), &roles, Base)
                    .end(End::UntilStopped(2)),
            );
        }
        let near = |above: bool| -> Vec<Step> {
            roles
                .iter()
                .map(|&m| set(m, near_limit(m, f, above)))
                .collect()
        };
        out.push(
            Script::with(
                "S7",
                mode,
                unit,
                &detail(v, &[]),
                &roles
                    .iter()
                    .map(|&m| (m, near_limit(m, f, false).to_owned()))
                    .collect::<Vec<_>>(),
            )
            .at(2, near(true))
            .at(25, near(false))
            .end(End::UntilStopped(2)),
        );
        let all_at = |value: &str| -> Vec<Step> { roles.iter().map(|&m| set(m, value)).collect() };
        let all_drawing = |n: u32| -> Vec<Step> {
            roles
                .iter()
                .map(|&m| Step::Set(m, drawing(m, f, n)))
                .collect()
        };
        out.push(
            Script::with(
                "S8",
                mode,
                unit,
                &detail(v, &["999 and 1000"]),
                &roles
                    .iter()
                    .map(|&m| (m, drawing(m, f, 999)))
                    .collect::<Vec<_>>(),
            )
            .set(5, &roles, Four)
            .at(15, all_drawing(999))
            .set(25, &roles, Base)
            .end(End::UntilStopped(2)),
        );
        out.push(
            s("S8", &["past the cap"], Max)
                .at(5, all_at("10000"))
                .set(10, &roles, Max)
                .set(25, &roles, Base)
                .end(End::UntilStopped(2)),
        );
        // S9: the other value one-digit too, so two values re-align when the width changes. No
        // reading draws one digit in °F.
        if !f {
            let others: Vec<(&'static str, String)> = shown(mode)
                .iter()
                .map(|&m| {
                    let value = if roles.contains(&m) {
                        "9".to_owned()
                    } else {
                        level(m, f, OneDigit)
                    };
                    (m, value)
                })
                .collect();
            out.push(
                Script::with("S9", mode, unit, &detail(v, &[]), &others)
                    .at(2, all_at("10"))
                    .at(4, all_at("9"))
                    .end(End::Total(6)),
            );
        }
        out.push(
            sv("S11", &["while static"], "unavailable")
                .set(2, &roles, OneDigit)
                .at(4, all_at("unavailable"))
                .set(6, &roles, Base)
                .end(End::Total(8)),
        );
        out.push(
            s("S11", &["while scrolling"], High)
                .at(5, all_at("unavailable"))
                .set(10, &roles, High)
                .at(20, all_at("unavailable"))
                .end(End::UntilStopped(2)),
        );
        out.push(
            sv("S11", &["to 100 or more"], "unavailable")
                .set(3, &roles, High)
                .end(End::Total(25)),
        );
        out.push(
            s("S12", &[], Max)
                .at(26, all_at("123456"))
                .at(30, all_at("1000000000"))
                .end(End::Total(52)),
        );
        let mut every_tenth = s("S13", &["three digits"], High);
        for i in 1..=21 {
            let steps: Vec<Step> = roles
                .iter()
                .map(|&m| Step::Set(m, offset(m, f, High, 11, i64::from(i))))
                .collect();
            every_tenth = every_tenth.at(10 * i, steps);
        }
        out.push(every_tenth.end(End::Total(212)));
        for phase in 0..5 {
            let mut every_tenth = s("S13", &["four digits", &format!("phase {phase}")], Four);
            for i in 1..=5 {
                let steps: Vec<Step> = roles
                    .iter()
                    .map(|&m| Step::Set(m, offset(m, f, Four, 111, i64::from(i))))
                    .collect();
                every_tenth = every_tenth.at(10 * i + phase, steps);
            }
            out.push(every_tenth.end(End::Total(52 + phase)));
        }
        out.push(
            sv("S19", &["while static"], "-1")
                .at(2, all_at("-0.5"))
                .set(4, &roles, Base)
                .end(End::Total(6)),
        );
        out.push(
            s("S19", &["while scrolling"], High)
                .at(5, all_at("-7"))
                .set(10, &roles, High)
                .at(20, all_at("-7"))
                .end(End::UntilStopped(2)),
        );
        let points = boundaries(roles[0], f);
        let mut s20 = sv("S20", &[], points[0]);
        for (tick, value) in (1..).zip(&points[1..]) {
            s20 = s20.at(tick, all_at(value));
        }
        let after = u32::try_from(points.len()).expect("a few boundaries");
        out.push(s20.set(after, &roles, Base).end(End::UntilStopped(2)));
        out.push(
            s("S22", &[], High)
                .at(5, [Step::Missed])
                .at(6, [Step::Missed])
                .at(10, [Step::Missed])
                .end(End::Total(25)),
        );
    }
    if pair {
        let names = shown(mode);
        let (top, bottom) = (&names[..1], &names[1..]);
        out.push(
            Script::new("S10", mode, unit, &[], top, High)
                .set(6, bottom, High)
                .set(12, top, Base)
                .set(18, top, High)
                .set(18, bottom, Base)
                .set(24, top, Base)
                .end(End::UntilStopped(2)),
        );
        out.push(
            Script::new("S10b", mode, unit, &[], top, Four)
                .at(5, [Step::Set(names[0], drawing(names[0], f, 999))])
                .set(10, top, Four)
                .at(15, [Step::Set(names[0], drawing(names[0], f, 99))])
                .set(15, bottom, Four)
                .set(25, names, Base)
                .end(End::UntilStopped(2)),
        );
    }
    out
}
