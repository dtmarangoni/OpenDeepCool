use super::*;

/// Runs a scenario: the movement procedure for a value, and in Music modes the spectrum, the
/// emoji loop or the Dancer, from the sound the script plays.
pub(super) fn run(scenario: &Scenario) -> Vec<Tick> {
    let last = *scenario.script.keys().last().expect("tick 0");
    let mut values: HashMap<&'static str, Option<Ratio>> = HashMap::new();
    let mut mode = scenario.mode;
    let mut in_fahrenheit = fahrenheit(scenario.unit);
    let mut movement: Option<Movement> = None;
    let mut music = MusicSettings::default();
    let mut audio: Option<(&'static [[f64; COLUMNS]], u32)> = None;
    let mut spectrum: Option<Spectrum> = None;
    // An emoji's place in its loop, and the ticks since sound last played; it stands at rest, on
    // its first picture, from the start and after REST_TICKS quiet ticks (music.md, Emoji
    // animations).
    let mut emoji_at = 0;
    let mut quiet = REST_TICKS;
    let mut dancer: Option<Dancer> = None;
    let mut ticks = Vec::new();
    let mut stopped = 0;
    for n in 0.. {
        let steps = scenario.script.get(&n).cloned().unwrap_or_default();
        let mut reset = false;
        let mut changed = false;
        let mut missed = false;
        for step in &steps {
            match step {
                Step::Set(metric, value) => {
                    values.insert(metric, reading(value));
                    changed = true;
                }
                Step::Reset(Cause::Mode(m)) => {
                    mode = *m;
                    reset = true;
                }
                Step::Reset(Cause::Unit(f)) => {
                    in_fahrenheit = *f;
                    reset = true;
                }
                Step::Reset(_) => reset = true,
                // Saved for the next mode that shows a temperature; this one draws none.
                Step::UnitSaved(f) => in_fahrenheit = *f,
                Step::Missed => missed = true,
                Step::Audio(signal) => audio = Some((signal_levels(signal), n)),
                Step::MusicPeaks(on) => music.peaks = *on,
                Step::MusicBarFall(name) => music.bar = name,
                Step::MusicPeakFall(name) => music.peak = name,
                _ => {}
            }
        }
        assert!(
            !(missed && audio.is_some()),
            "{}: a missed tick comes only in silence",
            scenario.heading
        );
        // The tick's window: the signal's tick since it started, or silence.
        let levels = audio
            .and_then(|(levels, from)| levels.get((n - from) as usize))
            .copied()
            .unwrap_or([0.0; COLUMNS]);
        let mut placed = None;
        let mut moving = false;
        let mut strip_frame = None;
        if value_mode(mode).is_some() {
            let readings: Vec<Option<Ratio>> = shown(mode)
                .iter()
                .map(|m| {
                    *values
                        .get(m)
                        .expect("every metric has a reading from tick 0")
                })
                .collect();
            let strip = strip_of(mode, &readings, in_fahrenheit);
            let m = match &mut movement {
                Some(m) if !reset => {
                    if changed {
                        m.change(strip);
                    }
                    m
                }
                _ => movement.insert(Movement::reset(strip)),
            };
            placed = Some(Placed {
                x: m.x(),
                home: m.strip().home(),
                cycle: m.strip().cycle(),
            });
            strip_frame = (!missed).then(|| m.tick());
            moving = m.moving();
        } else {
            movement = None;
        }
        // The spectrum, in 13–16 and 18–22: entering it from any other mode, 23 included, starts its levels
        // at 0, and a switch between them keeps them (music.md point 8). A missed tick, which only
        // comes in silence, leaves it, the emoji loop and the Dancer as they are (point 8).
        if has_spectrum(mode) {
            let s = spectrum.get_or_insert_with(Spectrum::enter);
            if !missed {
                s.update(&levels, stop(music.bar).0, stop(music.peak).1);
            }
        } else {
            spectrum = None;
        }
        // A reset restarts the emoji loop, at rest, and the Dancer (music.md).
        if reset {
            emoji_at = 0;
            quiet = REST_TICKS;
        }
        // The loop runs while sound plays, and for REST_TICKS - 1 ticks after it stops. A missed
        // tick moves nothing.
        let mut runs = false;
        if is_emoji(mode) && !missed {
            if levels.iter().any(|&level| level > 0.0) {
                quiet = 0;
            } else if quiet < REST_TICKS {
                quiet += 1;
            }
            runs = quiet < REST_TICKS;
            if !runs {
                emoji_at = 0;
            }
        }
        let pose = if mode == 23 {
            if reset || dancer.is_none() {
                dancer = Some(Dancer::reset());
            }
            let d = dancer.as_mut().expect("the Dancer");
            (!missed).then(|| d.tick(&levels))
        } else {
            dancer = None;
            None
        };
        let frame = (!missed).then(|| {
            let mut g = BLANK;
            match mode {
                13 => {}
                14..=16 | 24..=26 => {
                    let picture = emoji_ticks(mode)[emoji_at];
                    for (r, line) in emoji(mode)[picture].iter().enumerate() {
                        g[r] = row(line);
                    }
                    // The emoji alone fill the grid and draw no spectrum.
                    if is_emoji_alone(mode) {
                        return g;
                    }
                }
                18..=22 => g[..8].copy_from_slice(&strip_frame.expect("a value's frame")[..8]),
                23 => return poses()[usize::from(pose.expect("a pose"))],
                _ => return strip_frame.expect("a value's frame"),
            }
            let h = if mode == 13 { 12 } else { 6 };
            let s = spectrum.as_ref().expect("the spectrum");
            draw_spectrum(&mut g, s, h, music.peaks);
            g
        });
        if runs {
            emoji_at = (emoji_at + 1) % emoji_ticks(mode).len();
        }
        ticks.push(Tick {
            n,
            frame,
            reset,
            placed,
            moving,
            sound: levels.iter().any(|&level| level > 0.0),
            steps,
        });
        match scenario.end {
            End::Total(total) if n + 1 >= total => break,
            End::Total(_) => {}
            End::UntilStopped(more) => {
                if n >= last && !moving {
                    stopped += 1;
                    if stopped > more {
                        break;
                    }
                }
                assert!(n < last + 200, "{}: never stops", scenario.heading);
            }
        }
    }
    ticks
}
