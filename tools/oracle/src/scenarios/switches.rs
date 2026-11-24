use super::*;

/// S14: a switch from `mode` into every other mode, at every k of a scrolling strip, with values
/// below 100, above it, and mixed. A switch into a mode that shows a temperature runs in both
/// units, from a mode without one too. Modes 1–12 switch into modes 1–12, whose files stay as
/// Phase 8 wrote them; a Music mode switches into every mode, and into a Music mode while the
/// sound plays, so the spectrum's levels carry on.
pub(super) fn mode_switches(mode: u16, unit: Option<bool>) -> Vec<Scenario> {
    let mut out = Vec::new();
    for target in MODES
        .into_iter()
        .filter(|&t| t != mode && (mode > 12 || t <= 12))
    {
        let into_units: Vec<Option<bool>> = match unit {
            Some(_) => vec![unit],
            None => units(target).to_vec(),
        };
        let audio = if mode >= 13 {
            music_audio(target)
        } else {
            None
        };
        for u in into_units {
            out.extend(switches(mode, target, u, sets_for(target), audio));
        }
    }
    out
}

/// S14 into a Music mode from each mode 1–12, kept in the Music mode's file: its sound plays from
/// tick 0, and its levels start at 0 when it's entered (music.md point 8).
pub(super) fn switches_into(target: u16) -> Vec<Scenario> {
    let mut out = Vec::new();
    for source in 1..=12 {
        for &unit in units(source) {
            let into_units: Vec<Option<bool>> = match unit {
                Some(_) => vec![unit],
                None => units(target).to_vec(),
            };
            for u in into_units {
                out.extend(switches(
                    source,
                    target,
                    u,
                    sets_for(target),
                    music_audio(target),
                ));
            }
        }
    }
    out
}

/// The readings a switch into `target` needs: all three sets where it shows values; where it
/// shows none, only what keeps the mode left static or scrolling.
pub(super) fn sets_for(target: u16) -> &'static [SwitchSet] {
    if value_mode(target).is_some() {
        &SWITCH_SETS
    } else {
        &SWITCH_SETS[..2]
    }
}

/// The switches from `mode`, which has values, into `target` in unit `u`, one set after another:
/// at every k when the strip scrolls, at tick 3 when it's static, with `audio` from tick 0.
pub(super) fn switches(
    mode: u16,
    target: u16,
    u: Option<bool>,
    sets: &[SwitchSet],
    audio: Option<&'static str>,
) -> Vec<Scenario> {
    let mut out = Vec::new();
    for &(label, high) in sets {
        let readings: Vec<(&'static str, String)> = METRICS
            .iter()
            .map(|&m| {
                let at = if high(m) { Level::High } else { Level::Base };
                (m, level(m, fahrenheit(u), at))
            })
            .collect();
        let into = format!("into mode {target}");
        let switch = |detail: &[String], t: u32, end: u32| {
            Script::with("S14", mode, u, detail, &readings)
                .at(0, audio.map(Step::Audio))
                .at(t, [Step::Reset(Cause::Mode(target))])
                .end(End::Total(end))
        };
        if shown(mode).iter().any(|m| high(m)) {
            for k in 0..21 {
                let t = tick_at(k, 21);
                let detail = [into.clone(), label.to_owned(), format!("k {k}")];
                out.push(switch(&detail, t, t + 5));
            }
        } else {
            out.push(switch(&[into, label.to_owned()], 3, 8));
        }
    }
    out
}
