//! The attended tour (owner, 2026-10-24; Phase 7 plan, Task 9), `odc-e2e --attended`: a short run
//! for the owner to watch on the cooler, never a gate (AGENTS.md rule 3: every required test stays
//! unattended). Each listed mode's upright cases, its three value sets in each unit it shows, and
//! its dashes, then one variant of each scenario type the dev service can be driven through for
//! each kind of mode, each shown on the real cooler, with the expected picture as text and one
//! line on what should happen (`--modes` lists the modes; Phase 8 plan, Task 3); an emoji mode
//! also gets one Refresh mid-loop (Phase 9 plan, Task 7), a value over the spectrum its value
//! scrolling over the burst train (Task 8), and the Dancer a beat train soft, then loud, then
//! silence (Task 9). The owner answers `y`, `n` (with an optional note) or `r` to see it again; optionally, one real
//! standby. The answers go to `target/e2e/attended-<unix time>.txt`, and any `n` makes the exit
//! status non-zero. Afterwards the owner's settings are as the tour found them. Later phases add
//! one case of each of their modes and edge cases.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use odc_protocol::Message;
use odc_render::{EmojiLoop, Mode, draws, mode_info, spectrum_height};
use odc_service::engine::shows_temperature;
use odc_testkit::expected::{
    Case, Kind, Picture, Unit, load_mode, load_unavailable, mode_scenarios,
};
use odc_testkit::hardware::{Capability, Probe, Roots, probe};
use odc_testkit::sysfs::{LP360_PRODUCT, LP360_VENDOR, usb_device};

use crate::plays::{Current, Plan, message, play_real, recording_lines};
use crate::service::{Restore, gpu_addresses, real_or_why, scenario_plans, state};
use crate::session::Session;

/// One answer of the owner's.
struct Answer {
    case: String,
    looked_right: bool,
    note: String,
}

/// Asks until the owner answers `y`, `n` or `r`; `None` for `r`.
fn ask(input: &mut impl BufRead, prompt: &str) -> io::Result<Option<(bool, String)>> {
    loop {
        print!("{prompt} [y = it looked right, n = it didn't, r = show it again]: ");
        io::stdout().flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "no more answers",
            ));
        }
        match line.trim() {
            "y" => return Ok(Some((true, String::new()))),
            "r" => return Ok(None),
            "n" => {
                print!("what was wrong (optional): ");
                io::stdout().flush()?;
                let mut note = String::new();
                input.read_line(&mut note)?;
                return Ok(Some((false, note.trim().to_owned())));
            }
            _ => println!("y, n or r"),
        }
    }
}

/// What should happen, from `display.md`'s scenario table: the Expected column of `id`'s row.
fn expected_text(id: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/spec/display.md");
    fs::read_to_string(path)
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                (cells.get(1) == Some(&id)).then(|| cells.get(3).copied().unwrap_or("").to_owned())
            })
        })
        .unwrap_or_else(|| format!("{id}: see docs/spec/display.md"))
}

fn describe(case: &Case) -> String {
    let readings: Vec<String> = case
        .readings
        .iter()
        .map(|(metric, value)| match value {
            Some(v) => format!("{metric} = {}", v.num() as f64 / v.den() as f64),
            None => format!("{metric} unavailable"),
        })
        .collect();
    let movement = match &case.kind {
        Kind::Static(_) => "it stands still".to_owned(),
        Kind::Scrolling(cycle) => format!(
            "it scrolls left one column per frame, a {}-frame cycle, again and again",
            cycle.len()
        ),
        Kind::Loop(steps) => format!(
            "a loop of {} pictures, {} ms in all, again and again",
            steps.len(),
            steps.iter().map(|(d, _)| d.as_millis()).sum::<u128>()
        ),
    };
    let unit = match case.unit {
        Some(Unit::Celsius) => "in °C, ",
        Some(Unit::Fahrenheit) => "in °F, ",
        None => "",
    };
    format!("{unit}{}: {movement}", readings.join(", "))
}

fn print_picture(picture: &Picture) {
    println!("{picture:?}");
}

/// For each emoji mode among `modes` that `accepted` takes, S26 mid-loop (Phase 9 plan, Task 7):
/// at the first tick from halfway on whose picture isn't the loop's first, so the restart shows
/// on its own frame. The first S26 of an emoji's file, which the scenario types take, comes as the
/// loop is back at its first picture, where a restart shows nothing.
fn refreshes_mid_loop(
    modes: &[u16],
    accepted: &dyn Fn(u16) -> bool,
) -> Result<Vec<(String, Plan)>, String> {
    let mut plans = Vec::new();
    for &mode in modes.iter().filter(|&&mode| accepted(mode)) {
        let Some(mut emoji) = Mode::new(mode).and_then(EmojiLoop::of) else {
            continue;
        };
        let pictures: Vec<_> = (0..emoji.ticks()).map(|_| emoji.tick()).collect();
        let tick = (pictures.len() / 2..pictures.len())
            .find(|&t| pictures[t] != pictures[0])
            .ok_or_else(|| format!("mode {mode}: one picture from halfway on"))?;
        let heading = format!("## S26 · mode {mode} · at tick {tick}");
        let scenario = mode_scenarios(mode)
            .into_iter()
            .find(|s| s.heading == heading)
            .ok_or_else(|| format!("scenarios/mode-{mode:03}.txt: no `{heading}`"))?;
        plans.push(("S26".to_owned(), Plan::of_scenario(&scenario)));
    }
    Ok(plans)
}

/// For each mode with a value over the spectrum among `modes` that `accepted` takes, its values of
/// 100 or more scrolling over the burst train for a whole cycle, in °F where it shows a
/// temperature, before it switches into mode 13 (Phase 9 plan, Task 8): S14 at its largest *k*.
/// The value modes' other scenarios play in silence.
fn values_over_sound(
    modes: &[u16],
    accepted: &dyn Fn(u16) -> bool,
) -> Result<Vec<(String, Plan)>, String> {
    let mut plans = Vec::new();
    for &number in modes.iter().filter(|&&n| accepted(n) && accepted(13)) {
        let Some(mode) = Mode::new(number)
            .filter(|&m| spectrum_height(m).is_some() && !mode_info(m).metrics.is_empty())
        else {
            continue;
        };
        let unit = if shows_temperature(mode) {
            "°F · "
        } else {
            ""
        };
        let prefix = format!("## S14 · mode {number} · {unit}into mode 13 · values above 100 · k ");
        let scenario = mode_scenarios(number)
            .into_iter()
            .filter_map(|s| {
                let k: u32 = s.heading.strip_prefix(&prefix)?.parse().ok()?;
                Some((k, s))
            })
            .max_by_key(|(k, _)| *k)
            .map(|(_, s)| s)
            .ok_or_else(|| format!("scenarios/mode-{number:03}.txt: no `{prefix}…`"))?;
        plans.push(("S14".to_owned(), Plan::of_scenario(&scenario)));
    }
    Ok(plans)
}

/// The Dancer's tour signal: a beat train soft, then loud, then silence (owner, 2026-11-06).
const TOUR_DANCE: &str = "beat train, bass rising 0.13 a tick from 0.38 to 0.90, then 2 silent";

/// What the Dancer's tour item should show.
const DANCE_EXPECTED: &str =
    "a pose a tick, as spectrum/dancer.txt lists them for this beat train (music.md, Dancer)";

/// The Dancer to [`TOUR_DANCE`], when `modes` lists mode 23 and `accepted` takes it.
fn dance(modes: &[u16], accepted: &dyn Fn(u16) -> bool) -> Result<Vec<(String, Plan)>, String> {
    if !(modes.contains(&23) && accepted(23)) {
        return Ok(Vec::new());
    }
    Ok(vec![("the Dancer".to_owned(), Plan::of_dance(TOUR_DANCE)?)])
}

/// What should happen in the tour item `id`: a scenario's Expected column, or the Dancer's.
fn what(id: &str) -> String {
    if id == "the Dancer" {
        DANCE_EXPECTED.to_owned()
    } else {
        expected_text(id)
    }
}

/// Runs the tour; `Ok(true)` when every answer was `y`.
pub fn run(roots: &Roots, out_dir: &Path, modes: &[u16]) -> Result<bool, String> {
    match probe(Capability::DevService, roots) {
        Probe::Real => {}
        Probe::Absent { reason } => {
            return Err(format!("the tour needs the dev service: {reason}"));
        }
        Probe::Unusable { reason, fix } => return Err(format!("{reason}; fix: {fix}")),
    }
    let session = Session::begin()?;
    let found = state(&session)?;
    let offered = found.available_modes.clone().unwrap_or_default();
    let accepted =
        |mode: u16| Mode::new(mode).is_some_and(draws) && offered.contains(&u32::from(mode));
    let current = Current {
        chosen_gpu: found.chosen_gpu.clone().filter(|pci| !pci.is_empty()),
        gpus: gpu_addresses(&found),
    };
    let _restore = Restore {
        session: &session,
        state: found,
    };
    let switch = usb_device(&roots.sysfs, LP360_VENDOR, LP360_PRODUCT)
        .map(|usb| usb.join("authorized"))
        .filter(|_| probe(Capability::Lp360Reconnect, roots) == Probe::Real);
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut answers = Vec::new();
    let io = |e: io::Error| format!("stdin: {e}");

    println!("The tour shows each case on the cooler. Watch it, then answer.\n");
    for &mode in modes {
        let file = load_mode(mode);
        let dashes = load_unavailable();
        // In silence the Dancer stands still: of mode 23's poses, only pose 0 shows on the
        // service (Phase 9 plan, Task 5).
        let cases = file
            .cases
            .iter()
            .filter(|c| c.rotation.setting() == 0 && c.pose.is_none_or(|p| p == 0))
            .chain(
                dashes
                    .iter()
                    .filter(|(m, c)| *m == mode && c.rotation.setting() == 0)
                    .map(|(_, c)| c),
            );
        for case in cases {
            let plan = Plan::of_case(mode, case)?;
            let name = format!("mode {mode}, {}", describe(case));
            let first = match &case.kind {
                Kind::Static(p) => p,
                Kind::Scrolling(cycle) => &cycle[0],
                Kind::Loop(steps) => &steps[0].1,
            };
            loop {
                println!("== {name}");
                print_picture(first);
                let shown = play_real(&session, &plan, &current, None);
                if let Err(e) = &shown {
                    println!("(the recording disagrees: {e})");
                }
                if let Some((right, note)) =
                    ask(&mut input, "Did the cooler show this?").map_err(io)?
                {
                    answers.push(Answer {
                        case: name,
                        looked_right: right,
                        note,
                    });
                    break;
                }
            }
        }
    }

    let mut plans: Vec<(String, Plan)> = scenario_plans(modes, &accepted)
        .into_iter()
        .map(|(id, _, plan)| (id, plan))
        .collect();
    plans.extend(refreshes_mid_loop(modes, &accepted)?);
    plans.extend(values_over_sound(modes, &accepted)?);
    plans.extend(dance(modes, &accepted)?);
    for (id, plan) in plans {
        if let Err(why) = real_or_why(&id, current.gpus.len(), switch.is_some()) {
            println!("== {id} isn't shown: {why}");
            continue;
        }
        let name = format!("{id} · {}", plan.name);
        loop {
            println!("== {name}\n   what should happen: {}", what(&id));
            if let Some((_, Some(first))) = plan.ticks.first() {
                print_picture(&first.picture);
            }
            let shown = play_real(&session, &plan, &current, switch.as_deref());
            if let Err(e) = &shown {
                println!("(the recording disagrees: {e})");
            }
            if let Some((right, note)) = ask(&mut input, "Did it happen so?").map_err(io)? {
                answers.push(Answer {
                    case: name,
                    looked_right: right,
                    note,
                });
                break;
            }
        }
    }

    print!(
        "\nOptional: a real standby. Type s to suspend and wake the PC yourself now, or anything \
         else to skip: "
    );
    io::stdout().flush().map_err(io)?;
    let mut line = String::new();
    input.read_line(&mut line).map_err(io)?;
    if line.trim() == "s" {
        let before = recording_lines(None)?.len();
        print!("Suspend the PC now (for example `systemctl suspend`), wake it, then press Enter: ");
        io::stdout().flush().map_err(io)?;
        input.read_line(&mut String::new()).map_err(io)?;
        let after = recording_lines(None)?;
        let messages: Vec<Message> = after[before.min(after.len())..]
            .iter()
            .map(message)
            .collect::<Result<_, _>>()?;
        let again = messages
            .windows(2)
            .any(|w| w[0] == Message::Startup && matches!(w[1], Message::Frame(_)));
        println!(
            "the recording {} 03 08 again before the next frame",
            if again { "shows" } else { "doesn't show" }
        );
        if let Some((right, note)) =
            ask(&mut input, "Does the cooler show the mode again?").map_err(io)?
        {
            answers.push(Answer {
                case: "standby".into(),
                looked_right: right && again,
                note: if again {
                    note
                } else {
                    format!("no 03 08 recorded; {note}")
                },
            });
        }
    }

    let unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let path = out_dir.join(format!("attended-{unix}.txt"));
    let text: String = answers
        .iter()
        .map(|a| {
            format!(
                "{} | {}{}\n",
                a.case,
                if a.looked_right { "y" } else { "n" },
                if a.note.is_empty() {
                    String::new()
                } else {
                    format!(" | {}", a.note)
                }
            )
        })
        .collect();
    fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    let wrong = answers.iter().filter(|a| !a.looked_right).count();
    println!(
        "{} answers, {wrong} that didn't look right; written to {}",
        answers.len(),
        path.display()
    );
    Ok(wrong == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each emoji mode the service takes gets one Refresh mid-loop, on a picture other than the
    // loop's first, from its own scenario file; no other mode does.
    #[test]
    fn each_emoji_mode_gets_a_refresh_mid_loop() {
        let plans = refreshes_mid_loop(&[1, 13, 14, 15, 16, 17, 18, 23], &|_| true).expect("found");
        let names: Vec<&str> = plans.iter().map(|(_, plan)| plan.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "S26 · mode 14 · at tick 4",
                "S26 · mode 15 · at tick 4",
                "S26 · mode 16 · at tick 4",
                "S26 · mode 17 · at tick 6",
            ]
        );
        assert!(plans.iter().all(|(id, _)| id == "S26"));
        assert!(
            refreshes_mid_loop(&[14], &|_| false)
                .expect("none")
                .is_empty()
        );
    }

    // Each value mode over the spectrum the service takes gets its values scrolling over the
    // burst train for a whole cycle, from its own scenario file; no other mode does, nor one
    // whose switch into mode 13 the service would refuse.
    #[test]
    fn each_value_mode_over_the_spectrum_gets_its_value_scrolling_over_sound() {
        let plans =
            values_over_sound(&[1, 13, 14, 18, 19, 20, 21, 22, 23], &|_| true).expect("found");
        let names: Vec<&str> = plans.iter().map(|(_, plan)| plan.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "S14 · mode 18 · °F · into mode 13 · values above 100 · k 20",
                "S14 · mode 19 · °F · into mode 13 · values above 100 · k 20",
                "S14 · mode 20 · into mode 13 · values above 100 · k 20",
                "S14 · mode 21 · into mode 13 · values above 100 · k 20",
                "S14 · mode 22 · into mode 13 · values above 100 · k 20",
            ]
        );
        assert!(
            values_over_sound(&[18], &|mode| mode != 13)
                .expect("none")
                .is_empty()
        );
    }

    // The Dancer's tour item plays its beat train from dancer.txt, only when mode 23 is listed and
    // taken.
    #[test]
    fn the_dancer_gets_its_beat_train() {
        let plans = dance(&[13, 23], &|_| true).expect("found");
        assert_eq!(plans.len(), 1);
        assert!(
            plans[0].1.name.starts_with(TOUR_DANCE),
            "{}",
            plans[0].1.name
        );
        assert!(dance(&[13], &|_| true).expect("none").is_empty());
        assert!(dance(&[23], &|_| false).expect("none").is_empty());
    }
}
