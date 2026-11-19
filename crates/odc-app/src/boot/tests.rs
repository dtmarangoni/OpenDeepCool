//! The switch's model against the design's tables (`docs/design/settings-app.md`, Start at
//! boot): every combination of the four facts, every state of States and their text, and every
//! way a flip ends. Each expected word is typed here from the design.

use super::*;

const ENABLED: UnitFile = UnitFile::Enabled;
const DISABLED: UnitFile = UnitFile::Disabled;

fn facts(service_file: UnitFile, service: Run, sound_file: UnitFile, sound: bool) -> Facts {
    Facts {
        service_file,
        service,
        sound_file,
        sound,
    }
}

fn all_on() -> Facts {
    facts(ENABLED, Run::Running, ENABLED, true)
}

fn new_install() -> Facts {
    facts(DISABLED, Run::Stopped, DISABLED, false)
}

fn boot(known: Known) -> Boot {
    Boot {
        known,
        ..Boot::default()
    }
}

fn read(facts: Facts) -> Boot {
    boot(Known::Facts(facts))
}

fn note(boot: &Boot) -> Option<(Style, String)> {
    switch(boot).note.map(|note| (note.style, note.text))
}

fn warn(text: &str) -> Option<(Style, String)> {
    Some((Style::Warning, text.to_owned()))
}

#[test]
fn every_documented_active_state_reads_as_the_design_says() {
    for (active, sub, owned, want) in [
        ("active", "running", false, Some(Run::Running)),
        ("reloading", "reload", false, Some(Run::Running)),
        ("refreshing", "running", false, Some(Run::Running)),
        ("inactive", "dead", false, Some(Run::Stopped)),
        ("deactivating", "stop-sigterm", true, Some(Run::Stopped)),
        ("maintenance", "cleaning", false, Some(Run::Stopped)),
        ("failed", "failed", false, Some(Run::Failed)),
        ("activating", "auto-restart", false, Some(Run::Failed)),
        ("activating", "auto-restart-queued", true, Some(Run::Failed)),
        ("activating", "start", false, Some(Run::Starting)),
        ("activating", "start-pre", false, Some(Run::Starting)),
        // Its bus name already has an owner: it reports ready just after taking it.
        ("activating", "start", true, Some(Run::Running)),
        ("exploding", "running", true, None),
        ("", "", false, None),
    ] {
        assert_eq!(
            service_run(active, sub, owned),
            want,
            "{active} {sub} {owned}"
        );
    }
    for (active, want) in [
        ("active", Some(true)),
        ("reloading", Some(true)),
        ("refreshing", Some(true)),
        ("inactive", Some(false)),
        ("deactivating", Some(false)),
        ("maintenance", Some(false)),
        ("failed", Some(false)),
        ("activating", Some(false)),
        ("exploding", None),
    ] {
        assert_eq!(sound_runs(active), want, "{active}");
    }
    assert_eq!(UnitFile::read("enabled"), ENABLED);
    assert_eq!(UnitFile::read("disabled"), DISABLED);
    for other in ["masked", "enabled-runtime", "linked", "static", "bad", ""] {
        assert_eq!(UnitFile::read(other), UnitFile::Other(other.to_owned()));
    }
}

/// Every combination of the four facts: on only when all four hold, the switch sensitive unless
/// the display service is starting, and the note as The note composes it.
#[test]
fn the_note_names_what_is_missing_in_every_combination() {
    let display = [
        (ENABLED, Run::Running, None),
        (DISABLED, Run::Running, Some(RUNS_NOT_AT_BOOT)),
        (ENABLED, Run::Stopped, Some(STOPPED)),
        (ENABLED, Run::Failed, Some(FAILED)),
        // Neither starts at boot nor runs: OpenDeepCool is off, and there's no note at all.
        (DISABLED, Run::Stopped, None),
        (DISABLED, Run::Failed, None),
        (ENABLED, Run::Starting, None),
        (DISABLED, Run::Starting, None),
    ];
    let sound = [
        (ENABLED, true, None),
        (DISABLED, false, Some(SOUND_OFF)),
        (DISABLED, true, Some(SOUND_NOT_AT_LOGIN)),
        (ENABLED, false, Some(SOUND_NOT_HERE)),
    ];
    let mut combinations = 0;
    for (service_file, service, display_phrase) in &display {
        for (sound_file, runs, sound_phrase) in &sound {
            combinations += 1;
            let facts = facts(service_file.clone(), *service, sound_file.clone(), *runs);
            let what = format!("{facts:?}");
            let shown = switch(&read(facts.clone()));
            let on = facts == all_on();
            assert_eq!((shown.handle, shown.trough), (on, on), "{what}");
            let starting = *service == Run::Starting;
            assert_eq!(shown.sensitive, !starting, "{what}");
            let off = *service_file == DISABLED && matches!(service, Run::Stopped | Run::Failed);
            let want = if starting {
                Some((Style::Info, STARTING.to_owned()))
            } else if off {
                None
            } else {
                let phrases: Vec<&str> =
                    display_phrase.iter().chain(sound_phrase).copied().collect();
                (!phrases.is_empty()).then(|| {
                    let ending = if phrases == [SOUND_NOT_HERE] {
                        "Turn it on to start it, with no password."
                    } else {
                        "Turn it on to finish."
                    };
                    let journal = if *display_phrase == Some(FAILED) {
                        " The journal says why the service failed."
                    } else {
                        ""
                    };
                    (
                        Style::Warning,
                        format!(
                            "Not fully on: {}. {ending}{journal}",
                            phrases.join(", and ")
                        ),
                    )
                })
            };
            assert_eq!(note(&read(facts)), want, "{what}");
        }
    }
    assert_eq!(combinations, 32);
}

/// The design's three examples, word for word.
#[test]
fn the_design_s_examples_read_word_for_word() {
    assert_eq!(
        note(&read(facts(DISABLED, Run::Running, DISABLED, false))),
        warn(
            "Not fully on: the display service runs now, but doesn't start at boot, and the sound \
             for Music is off. Turn it on to finish."
        )
    );
    assert_eq!(
        note(&read(facts(ENABLED, Run::Failed, ENABLED, true))),
        warn(
            "Not fully on: the display service failed. Turn it on to finish. The journal says why \
             the service failed."
        )
    );
    assert_eq!(
        note(&read(facts(ENABLED, Run::Running, ENABLED, false))),
        warn(
            "Not fully on: the sound for Music isn't running in this session. Turn it on to start \
             it, with no password."
        )
    );
}

/// The unknown states: a failed read, and a unit file state that's neither enabled nor disabled.
#[test]
fn unknown_states_are_insensitive_at_off() {
    let shown = switch(&boot(Known::Failed));
    assert_eq!(
        shown,
        Switch {
            handle: false,
            trough: false,
            sensitive: false,
            note: Some(Note {
                style: Style::Info,
                text: "Can't tell whether OpenDeepCool is on: reading it from systemd failed."
                    .to_owned(),
            }),
        }
    );
    let masked = facts(
        UnitFile::Other("masked".to_owned()),
        Run::Stopped,
        ENABLED,
        true,
    );
    let shown = switch(&read(masked));
    assert_eq!(
        (shown.handle, shown.trough, shown.sensitive),
        (false, false, false)
    );
    assert_eq!(
        shown.note.map(|n| (n.style, n.text)),
        Some((
            Style::Info,
            "Can't change it here: systemd reports opendeepcool.service as “masked”.".to_owned()
        ))
    );
    // The display service first, then the sound.
    let both = facts(
        UnitFile::Other("linked".to_owned()),
        Run::Running,
        UnitFile::Other("enabled-runtime".to_owned()),
        true,
    );
    assert!(note(&read(both)).is_some_and(|(_, t)| t.contains("opendeepcool.service as “linked”")));
    let sound = facts(
        ENABLED,
        Run::Running,
        UnitFile::Other("bad".to_owned()),
        true,
    );
    assert_eq!(
        note(&read(sound)).map(|(_, t)| t),
        Some(
            "Can't change it here: systemd reports opendeepcool-audio.service as “bad”.".to_owned()
        )
    );
    // Not read yet: nothing to say.
    let shown = switch(&Boot::default());
    assert_eq!((shown.sensitive, shown.note), (false, None));
}

/// A flip from each state: what it runs, its line, the handle at the new position and the trough
/// at systemd's, insensitive until it ends.
#[test]
fn a_flip_runs_its_steps_and_shows_its_line() {
    // On from a new install: pkexec, then the sound in this session.
    let mut b = read(new_install());
    assert_eq!(
        b.begin(Direction::On),
        Some(vec![Step::Pkexec(Direction::On), Step::StartSound])
    );
    let shown = switch(&b);
    assert_eq!(
        (shown.handle, shown.trough, shown.sensitive),
        (true, false, false)
    );
    assert_eq!(
        shown.note,
        Some(Note {
            style: Style::Plain,
            text: "Turning on… Your desktop asks for the administrator's password first."
                .to_owned()
        })
    );
    assert_eq!(b.begin(Direction::On), None, "never twice at once");

    // Only the sound in this session missing: no pkexec, no password.
    let mut b = read(facts(ENABLED, Run::Running, ENABLED, false));
    assert_eq!(b.begin(Direction::On), Some(vec![Step::StartSound]));
    assert_eq!(
        note(&b),
        Some((
            Style::Plain,
            "Turning on… Starting the sound for Music in this session.".to_owned()
        ))
    );

    // Every partly-on state turns on through pkexec, the sound included.
    for partly in [
        facts(DISABLED, Run::Running, DISABLED, false),
        facts(ENABLED, Run::Failed, ENABLED, true),
        facts(ENABLED, Run::Stopped, ENABLED, false),
        facts(ENABLED, Run::Running, DISABLED, true),
        facts(DISABLED, Run::Stopped, ENABLED, true),
    ] {
        let mut b = read(partly.clone());
        assert_eq!(
            b.begin(Direction::On),
            Some(vec![Step::Pkexec(Direction::On), Step::StartSound]),
            "{partly:?}"
        );
        // A partly-on setup can't be turned off in one flip.
        let mut b = read(partly);
        assert_eq!(b.begin(Direction::Off), None);
    }

    // Off, only from on.
    let mut b = read(all_on());
    assert_eq!(b.begin(Direction::On), None, "already on");
    assert_eq!(
        b.begin(Direction::Off),
        Some(vec![Step::Pkexec(Direction::Off), Step::StopSound])
    );
    let shown = switch(&b);
    assert_eq!(
        (shown.handle, shown.trough, shown.sensitive),
        (false, true, false)
    );
    assert_eq!(
        note(&b),
        Some((
            Style::Plain,
            "Turning off… Your desktop asks for the administrator's password first.".to_owned()
        ))
    );

    // Never while unknown, starting or not read.
    for known in [
        Known::NotRead,
        Known::Failed,
        Known::Facts(facts(ENABLED, Run::Starting, ENABLED, true)),
        Known::Facts(facts(
            UnitFile::Other("masked".to_owned()),
            Run::Stopped,
            ENABLED,
            true,
        )),
    ] {
        let mut b = boot(known.clone());
        assert_eq!(b.begin(Direction::On), None, "{known:?}");
        assert_eq!(b.flip, Flip::Idle);
    }
}

/// How a flip ends: each failure's note, and the switch back at systemd's state, sensitive again.
#[test]
fn each_failure_shows_its_note() {
    let on_failures = [
        (
            Failure::NotAuthorized(Direction::On),
            "Not changed: the password wasn't accepted, or no password dialog could open in this \
             session.",
        ),
        (
            Failure::Step(
                Direction::On,
                "step 3 of 3, `systemctl start opendeepcool.service`, failed: exit status: 1"
                    .to_owned(),
            ),
            "Didn't finish: step 3 of 3, `systemctl start opendeepcool.service`, failed: exit \
             status: 1. The switch shows what's set now; the journal says more.",
        ),
        (
            Failure::SoundNotStarted,
            "Didn't finish: the sound for Music didn't start in this session. It starts at your \
             next login; the journal says more.",
        ),
    ];
    for (failure, text) in on_failures {
        let mut b = read(new_install());
        b.begin(Direction::On).expect("a flip");
        b.end(Some(failure.clone()), false);
        assert_eq!(b.flip, Flip::Idle, "{failure:?}");
        let shown = switch(&b);
        assert_eq!(
            (shown.handle, shown.trough, shown.sensitive),
            (false, false, true)
        );
        assert_eq!(
            shown.note,
            Some(Note {
                style: Style::Warning,
                text: text.to_owned()
            })
        );
    }
    for failure in [
        Failure::NotAuthorized(Direction::Off),
        Failure::Step(Direction::Off, "step 1 of 2 failed".to_owned()),
    ] {
        let mut b = read(all_on());
        b.begin(Direction::Off).expect("a flip");
        b.end(Some(failure.clone()), true);
        let shown = switch(&b);
        assert_eq!(
            (shown.handle, shown.trough, shown.sensitive),
            (true, true, true)
        );
        assert_eq!(
            shown.note.map(|n| n.style),
            Some(Style::Warning),
            "{failure:?}"
        );
    }
    // Turned off, but the sound still runs here: the whole PC's change was made.
    let mut b = read(all_on());
    b.begin(Direction::Off).expect("a flip");
    b.end(Some(Failure::SoundNotStopped), false);
    b.read(Known::Facts(facts(DISABLED, Run::Stopped, DISABLED, true)));
    let shown = switch(&b);
    assert_eq!(
        shown.note.map(|n| n.text).as_deref(),
        Some(
            "Off, but the sound for Music didn't stop in this session. It stops when you log out."
        )
    );
    // The failure note wins over the missing parts', which come back once it goes.
    let mut b = read(facts(DISABLED, Run::Running, DISABLED, false));
    b.begin(Direction::On).expect("a flip");
    b.end(Some(Failure::NotAuthorized(Direction::On)), true);
    b.read(Known::Facts(facts(DISABLED, Run::Running, DISABLED, false)));
    assert!(note(&b).is_some_and(|(_, t)| t.starts_with("Not changed")));
    b.failure = None;
    assert!(note(&b).is_some_and(|(_, t)| t.starts_with("Not fully on")));
}

/// A failure's note stays until the next flip, or until a read shows what its flip wanted.
#[test]
fn a_failure_s_note_goes_once_the_state_it_wanted_shows() {
    let mut b = read(new_install());
    b.begin(Direction::On).expect("a flip");
    b.end(Some(Failure::NotAuthorized(Direction::On)), false);
    b.read(Known::Facts(new_install()));
    assert!(b.failure.is_some(), "nothing changed");
    b.read(Known::Facts(facts(ENABLED, Run::Running, ENABLED, false)));
    assert!(b.failure.is_some(), "still not on");
    b.read(Known::Facts(all_on()));
    assert_eq!(b.failure, None, "typed in a terminal");

    let mut b = read(all_on());
    b.begin(Direction::Off).expect("a flip");
    b.end(Some(Failure::NotAuthorized(Direction::Off)), true);
    b.read(Known::Facts(all_on()));
    assert!(b.failure.is_some());
    b.read(Known::Facts(facts(DISABLED, Run::Failed, DISABLED, false)));
    assert_eq!(b.failure, None);

    // The next flip clears it.
    let mut b = read(new_install());
    b.begin(Direction::On).expect("a flip");
    b.end(Some(Failure::NotAuthorized(Direction::On)), false);
    b.read(Known::Facts(new_install()));
    b.begin(Direction::On).expect("again");
    assert_eq!(b.failure, None);
    // A failed read keeps it.
    b.end(Some(Failure::NotAuthorized(Direction::On)), false);
    b.read(Known::Failed);
    assert!(b.failure.is_some());
    assert!(
        note(&b).is_some_and(|(s, _)| s == Style::Warning),
        "the failure's note first"
    );
    assert!(!switch(&b).sensitive, "unknown");
}

/// After a successful `on`, the flip counts as running until the service's name has an owner,
/// or 5 s pass; on the off screen, the heading stays as it began.
#[test]
fn after_on_the_flip_waits_for_the_service_s_name() {
    let mut b = read(new_install());
    b.begin(Direction::On).expect("a flip");
    assert_eq!(off_screen(&b).heading, Some(IS_OFF));
    b.end(None, false);
    assert_eq!(b.flip, Flip::Waiting { was_off: true });
    // Read meanwhile: enabled, and starting without its name.
    b.read(Known::Facts(facts(ENABLED, Run::Starting, ENABLED, true)));
    let screen = off_screen(&b);
    assert_eq!(screen.heading, Some("OpenDeepCool is off"));
    assert_eq!(
        screen.switch.note.map(|n| n.text).as_deref(),
        Some(TURNING_ON)
    );
    assert!(!screen.switch.sensitive);
    b.owned();
    assert_eq!(b.flip, Flip::Idle);
    assert_eq!(off_screen(&b).heading, Some("OpenDeepCool isn't running"));

    // From "isn't running", it stays that.
    let mut b = read(facts(ENABLED, Run::Failed, ENABLED, true));
    b.begin(Direction::On).expect("a flip");
    b.end(None, false);
    b.read(Known::Facts(facts(ENABLED, Run::Starting, ENABLED, true)));
    assert_eq!(off_screen(&b).heading, Some(NOT_RUNNING));

    // The name already has an owner, or the flip needed no password: it ends at once.
    let mut b = read(new_install());
    b.begin(Direction::On).expect("a flip");
    b.end(None, true);
    assert_eq!(b.flip, Flip::Idle);
    let mut b = read(facts(ENABLED, Run::Running, ENABLED, false));
    b.begin(Direction::On).expect("a flip");
    b.end(None, false);
    assert_eq!(b.flip, Flip::Idle);
    // A failure ends it too.
    let mut b = read(new_install());
    b.begin(Direction::On).expect("a flip");
    b.end(Some(Failure::SoundNotStarted), false);
    assert_eq!(b.flip, Flip::Idle);
    b.owned();
    assert_eq!(b.flip, Flip::Idle);
}

/// The off screen: its heading, and the card in a session that may flip.
#[test]
fn the_off_screen_says_off_or_not_running() {
    for (known, heading, status_off) in [
        (Known::Facts(new_install()), IS_OFF, true),
        (
            Known::Facts(facts(DISABLED, Run::Stopped, ENABLED, true)),
            IS_OFF,
            true,
        ),
        (
            Known::Facts(facts(ENABLED, Run::Stopped, ENABLED, true)),
            NOT_RUNNING,
            false,
        ),
        (
            Known::Facts(facts(ENABLED, Run::Failed, ENABLED, false)),
            NOT_RUNNING,
            false,
        ),
        (
            Known::Facts(facts(ENABLED, Run::Starting, ENABLED, true)),
            NOT_RUNNING,
            false,
        ),
        (
            Known::Facts(facts(
                UnitFile::Other("masked".to_owned()),
                Run::Stopped,
                DISABLED,
                false,
            )),
            NOT_RUNNING,
            false,
        ),
        (Known::Failed, NOT_RUNNING, false),
    ] {
        let b = boot(known.clone());
        let screen = off_screen(&b);
        assert_eq!(screen.heading, Some(heading), "{known:?}");
        assert_eq!(service_off(&b), Some(status_off), "{known:?}");
        assert!(screen.card, "{known:?}");
    }
    // A new install has no note; a failed service names it.
    assert_eq!(off_screen(&read(new_install())).switch.note, None);
    assert!(
        off_screen(&read(facts(ENABLED, Run::Failed, ENABLED, true)))
            .switch
            .note
            .is_some_and(|n| n
                .text
                .starts_with("Not fully on: the display service failed."))
    );
    // A failed read: the read's note only.
    let screen = off_screen(&boot(Known::Failed));
    assert_eq!(
        screen.switch.note.map(|n| n.text).as_deref(),
        Some(READ_FAILED)
    );
    // Turning off ends on "is off".
    let mut b = read(all_on());
    b.begin(Direction::Off).expect("a flip");
    assert_eq!(off_screen(&b).heading, Some(IS_OFF));
    // Not read yet: it waits, with nothing shown.
    let screen = off_screen(&Boot::default());
    assert_eq!((screen.heading, screen.card), (None, false));
    assert_eq!(service_off(&Boot::default()), None);
    // A remote or inactive session: no card, but the heading.
    let b = Boot {
        may_flip: false,
        ..read(new_install())
    };
    let screen = off_screen(&b);
    assert_eq!((screen.heading, screen.card), (Some(IS_OFF), false));
}

/// `pkexec`'s end: 126 a closed dialog, 127 everything else of its own, any other code the
/// program's message.
#[test]
fn pkexec_s_codes_and_the_program_s_message() {
    let stderr = "Job for opendeepcool.service failed because the control process exited.\n\
                  opendeepcool-boot: step 3 of 3, `systemctl start opendeepcool.service`, failed: \
                  exit status: 1\n";
    assert_eq!(pkexec_failure(Direction::On, &Exit::Code(0), stderr), None);
    assert_eq!(
        pkexec_failure(Direction::On, &Exit::Code(126), "x"),
        Some(Failure::Dismissed(Direction::On))
    );
    assert_eq!(
        pkexec_failure(
            Direction::Off,
            &Exit::Code(127),
            "Error executing command as another user"
        ),
        Some(Failure::NotAuthorized(Direction::Off))
    );
    assert_eq!(
        pkexec_failure(Direction::On, &Exit::Code(1), stderr),
        Some(Failure::Step(
            Direction::On,
            "step 3 of 3, `systemctl start opendeepcool.service`, failed: exit status: 1"
                .to_owned()
        ))
    );
    // A refusal (exit 2) is followed by the usage line, which isn't the message.
    let refused = "opendeepcool-boot: refused: \"maybe\" is neither on nor off.\n\
                   usage: pkexec /usr/lib/opendeepcool/opendeepcool-boot on|off\n";
    assert_eq!(
        program_message(refused).as_deref(),
        Some("refused: \"maybe\" is neither on nor off")
    );
    assert_eq!(
        pkexec_failure(Direction::On, &Exit::Code(3), "nothing of the program's"),
        Some(Failure::Step(Direction::On, "exit status 3".to_owned()))
    );
    assert_eq!(program_message("opendeepcool-boot: \n"), None);
    assert_eq!(
        pkexec_failure(Direction::Off, &Exit::Signal(9), ""),
        Some(Failure::Step(
            Direction::Off,
            "pkexec ended by signal 9".to_owned()
        ))
    );
    assert_eq!(
        pkexec_failure(
            Direction::On,
            &Exit::NotStarted("no such file".to_owned()),
            ""
        ),
        Some(Failure::Step(
            Direction::On,
            "pkexec can't start: no such file".to_owned()
        ))
    );
    assert_eq!(program_name(), "opendeepcool-boot");
}

#[test]
fn the_card_s_words_as_the_design_writes_them() {
    assert_eq!(
        HINT,
        "When it's on, the display service starts at every boot, and the sound for the Music \
         options at every login."
    );
    assert_eq!(TITLE, "Start at boot");
    assert_eq!(
        OFF_SCREEN_TEXT,
        "The cooler shows nothing until the display service runs."
    );
}

// Owner, 2026-11-19: a closed password dialog changed nothing, and the switch shows the state, so
// it has no note and isn't kept: the card is exactly what it was, the flip is over (a dismissed
// `on` doesn't wait for the service), and the state's own notes show as before.
#[test]
fn a_closed_password_dialog_leaves_no_note_and_nothing_kept() {
    assert_eq!(Failure::Dismissed(Direction::On).text(), None);
    assert_eq!(Failure::Dismissed(Direction::Off).text(), None);
    for (direction, facts_before) in [
        (Direction::On, new_install()),
        (Direction::Off, all_on()),
        (Direction::On, facts(ENABLED, Run::Running, ENABLED, false)),
        (
            Direction::On,
            facts(DISABLED, Run::Running, DISABLED, false),
        ),
    ] {
        let untouched = switch(&read(facts_before.clone()));
        for owned in [false, true] {
            let mut b = read(facts_before.clone());
            b.begin(direction).expect("a flip");
            b.end(Some(Failure::Dismissed(direction)), owned);
            assert_eq!(b.failure, None, "not kept");
            assert_eq!(b.flip, Flip::Idle, "the flip is over, not waiting");
            // The switch is as it was before the flip, and sensitive.
            assert_eq!(switch(&b), untouched, "{direction:?}");
        }
    }
    // pkexec's 126 is still the closed dialog; it is what ends the flip.
    assert_eq!(
        pkexec_failure(Direction::On, &Exit::Code(126), ""),
        Some(Failure::Dismissed(Direction::On))
    );
}
