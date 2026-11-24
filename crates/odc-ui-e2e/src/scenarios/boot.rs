use super::*;

/// Fully on: the Startup card is the Configuration page's last, the switch on and usable, with
/// no note and no commands (Configuration page, Startup).
pub(super) fn boot_on(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.units(Units::all_on())?;
    run.page("Configuration")?;
    run.node("group", "Startup settings")?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.check_state("switch", BOOT, State::Sensitive, true)?;
    run.gone("group", "Start at boot status")?;
    run.no_terminal()?;
    // The card holds the title and the switch, and a note when one applies: no hint at its foot
    // (owner, 2026-11-19).
    run.gone(
        "label",
        "When it's on, the display service starts at every boot, and the sound for the Music \
         options at every login.",
    )?;
    run.page("Display")?;
    NO_NOTE
}

/// A flip off from the Configuration page, then on from the off screen (A flip, step by step;
/// The window around a flip): exactly one `pkexec` run each, with the program's path and the
/// direction; the switch changing meanwhile; the sound in this session stopped or started
/// after it; and, after `on`, the off screen kept until the service's name has an owner.
pub(super) fn boot_flip(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.units(Units::all_on())?;
    run.page("Configuration")?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    let mark = run.flip("off")?;
    run.expect_state("switch", BOOT, State::Sensitive, false)?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    thread::sleep(HOLD);
    run.no_note()?;
    if run.systemd.calls_since(mark)?.iter().any(|c| is_job(c)) {
        return Err("the sound moved before pkexec ended".to_owned());
    }
    // What the program leaves: both units disabled, the service stopped and its name gone.
    run.systemd.units = Units {
        sound: ("disabled", "active"),
        ..Units::new_install()
    };
    run.systemd.write()?;
    run.fake.on_bus = false;
    run.fake.write()?;
    run.systemd.answer(
        0,
        "opendeepcool-boot: turned off at boot, and the service stopped\n",
    )?;
    run.systemd.called(mark, &format!("{STOP_SOUND} done"))?;
    run.systemd.units.sound.1 = "inactive";
    run.expect_description("group", "Service message", IS_OFF)?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    run.gone("group", "Start at boot status")?;
    run.expect_texts("group", "Service status", &["Service off"])?;
    run.no_terminal()?;

    // On, from the off screen.
    let mark = run.flip("on")?;
    run.expect_state("switch", BOOT, State::Sensitive, false)?;
    run.check_state("switch", BOOT, State::Checked, true)?;
    thread::sleep(HOLD);
    run.no_note()?;
    // What the program leaves: both enabled, the service started. It takes its name just
    // before it's ready, here after pkexec ends, so the flip waits for it.
    run.systemd.units = Units {
        service: ("enabled", "activating", "start"),
        sound: ("enabled", "inactive"),
        ..Units::new_install()
    };
    run.systemd.write()?;
    run.systemd.answer(
        0,
        "opendeepcool-boot: turned on at boot, and the service started\n",
    )?;
    run.systemd.called(mark, &format!("{START_SOUND} done"))?;
    run.systemd.units.sound.1 = "active";
    thread::sleep(HOLD);
    run.expect_description("group", "Service message", IS_OFF)?;
    run.no_note()?;
    run.check_state("switch", BOOT, State::Sensitive, false)?;
    // The service owns its name: the pages come back on the page last shown, the switch on.
    run.systemd.units.service = ("enabled", "active", "running");
    run.systemd.write()?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("toggle button", "Configuration", State::Pressed, true)?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.gone("group", "Start at boot status")?;
    run.gone("group", "Service message")?;
    let calls = run.systemd.calls_since(mark)?;
    let jobs: Vec<&str> = calls
        .iter()
        .map(String::as_str)
        .filter(|c| is_job(c))
        .collect();
    let want = [
        "user Subscribe()".to_owned(),
        format!("{START_SOUND} done"),
        "user Unsubscribe()".to_owned(),
    ];
    if jobs != want {
        return Err(format!("the sound's start: {jobs:?}"));
    }
    if calls.iter().any(|c| c == "system Subscribe()") {
        return Err("subscribed to the system manager".to_owned());
    }
    run.page("Display")?;
    NO_NOTE
}

/// A flip that doesn't happen: the dialog closed (126), not authorized or no agent (127), and a
/// failed step (1, the program's message). The switch goes back, usable; a closed dialog has no
/// note (it changed nothing: owner, 2026-11-19), the others a note; the app's own step is
/// skipped. The note goes once systemd shows what the
/// flip wanted (States and their text).
pub(super) fn boot_refused(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.units(Units::all_on())?;
    run.page("Configuration")?;
    // A closed dialog: the switch back, usable, with no note.
    let mark = run.flip("off")?;
    run.expect_state("switch", BOOT, State::Sensitive, false)?;
    run.systemd.answer(126, "")?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.check_state("switch", BOOT, State::Checked, true)?;
    run.gone("group", "Start at boot status")?;
    let calls = run.systemd.calls_since(mark)?;
    if calls.iter().any(|c| c.contains("StopUnit")) {
        return Err(format!("the sound stopped after exit 126: {calls:?}"));
    }
    for (code, stderr, note) in [
        (
            127,
            "the fake pkexec: not authorized\n",
            "Not changed: the password wasn't accepted, or no password dialog could open in this \
             session.",
        ),
        (
            1,
            "Failed to disable unit: Access denied\nopendeepcool-boot: step 1 of 2, `systemctl \
             --global disable opendeepcool-audio.service`, failed: exit status: 1\n",
            "Didn't finish: step 1 of 2, `systemctl --global disable \
             opendeepcool-audio.service`, failed: exit status: 1. The switch shows what's set \
             now; the journal says more.",
        ),
    ] {
        let mark = run.flip("off")?;
        run.expect_state("switch", BOOT, State::Sensitive, false)?;
        run.systemd.answer(code, stderr)?;
        run.note(note)?;
        run.expect_state("switch", BOOT, State::Sensitive, true)?;
        run.check_state("switch", BOOT, State::Checked, true)?;
        run.no_terminal()?;
        let calls = run.systemd.calls_since(mark)?;
        if calls.iter().any(|c| c.contains("StopUnit")) {
            return Err(format!("the sound stopped after exit {code}: {calls:?}"));
        }
    }
    // Turned off in a terminal: the note goes once the window is active again.
    run.units(Units::new_install())?;
    run.gone("group", "Start at boot status")?;
    run.no_terminal()?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    run.page("Display")?;
    NO_NOTE
}

/// Only the sound in this session missing: a flip on starts it through the user's own manager,
/// with no `pkexec` and no password; when it doesn't start, its note.
pub(super) fn boot_only_sound(run: &mut Run) -> Result<String> {
    run.reset()?;
    let missing = Units {
        sound: ("enabled", "inactive"),
        ..Units::all_on()
    };
    run.units(missing.clone())?;
    run.page("Configuration")?;
    run.note(
        "Not fully on: the sound for Music isn't running in this session. Turn it on to start \
         it, with no password.",
    )?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    let mark = run.systemd.mark()?;
    run.act("switch", BOOT, "Toggle")?;
    run.systemd.called(mark, &format!("{START_SOUND} done"))?;
    run.systemd.units.sound.1 = "active";
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.gone("group", "Start at boot status")?;
    let runs = run.systemd.pkexec_since(mark)?;
    if !runs.is_empty() {
        return Err(format!("pkexec ran: {runs:?}"));
    }
    // The sound doesn't start: its note.
    run.units(Units {
        job: "failed",
        ..missing
    })?;
    let mark = run.systemd.mark()?;
    run.act("switch", BOOT, "Toggle")?;
    run.systemd.called(mark, &format!("{START_SOUND} failed"))?;
    run.note(
        "Didn't finish: the sound for Music didn't start in this session. It starts at your next \
         login; the journal says more.",
    )?;
    run.no_terminal()?;
    run.expect_state("switch", BOOT, State::Sensitive, true)?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    // The note stays at a read that still misses the sound, and goes once a read shows it
    // running: the state the flip wanted.
    run.units(Units {
        job: "failed",
        ..missing
    })?;
    run.check_texts("group", "Start at boot status", &[
        "Didn't finish: the sound for Music didn't start in this session. It starts at your next \
         login; the journal says more.",
    ])?;
    run.units(Units::all_on())?;
    run.gone("group", "Start at boot status")?;
    run.no_terminal()?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.page("Display")?;
    NO_NOTE
}

/// Partly on: the note names what's missing, and a flip on runs `pkexec on` whatever it is
/// (The note).
pub(super) fn boot_partly(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    for (units, note) in [
        (
            Units {
                service: ("disabled", "active", "running"),
                ..Units::new_install()
            },
            "Not fully on: the display service runs now, but doesn't start at boot, and the sound \
             for Music is off. Turn it on to finish.",
        ),
        (
            Units {
                sound: ("disabled", "active"),
                ..Units::all_on()
            },
            "Not fully on: the sound for Music runs here, but doesn't start at login. Turn it on \
             to finish.",
        ),
    ] {
        run.units(units)?;
        run.note(note)?;
        run.check_state("switch", BOOT, State::Checked, false)?;
        run.check_state("switch", BOOT, State::Sensitive, true)?;
        run.no_terminal()?;
    }
    let mark = run.flip("on")?;
    run.systemd.units = Units::all_on();
    run.systemd.write()?;
    run.systemd.answer(0, "")?;
    run.systemd.called(mark, &format!("{START_SOUND} done"))?;
    run.expect_state("switch", BOOT, State::Checked, true)?;
    run.gone("group", "Start at boot status")?;
    run.page("Display")?;
    NO_NOTE
}

/// The off screen's states (Where its state comes from; States and their text): starting, a
/// masked unit, a failed read and one that doesn't answer within 1 s; each insensitive, with its
/// note.
pub(super) fn boot_off_screen(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.systemd.units.service = ("enabled", "activating", "start");
    run.systemd.write()?;
    run.fake.on_bus = false;
    run.fake.write()?;
    run.expect_description("group", "Service message", NOT_RUNNING)?;
    run.note("The display service is starting.")?;
    run.expect_state("switch", BOOT, State::Sensitive, false)?;
    for (units, note) in [
        (
            Units {
                service: ("masked", "inactive", "dead"),
                ..Units::all_on()
            },
            "Can't change it here: systemd reports opendeepcool.service as “masked”.",
        ),
        (
            Units {
                service: ("", "inactive", "dead"),
                ..Units::all_on()
            },
            "Can't tell whether OpenDeepCool is on: reading it from systemd failed.",
        ),
        (
            Units {
                delay_ms: 1500,
                ..Units::all_on()
            },
            "Can't tell whether OpenDeepCool is on: reading it from systemd failed.",
        ),
    ] {
        run.units(units)?;
        run.note(note)?;
        run.expect_description("group", "Service message", NOT_RUNNING)?;
        run.check_state("switch", BOOT, State::Sensitive, false)?;
        run.check_state("switch", BOOT, State::Checked, false)?;
        run.no_terminal()?;
    }
    run.systemd.units = Units::all_on();
    run.systemd.write()?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    NO_NOTE
}

/// A session logind reports as remote: the off screen keeps its heading, but has no switch,
/// since polkit would refuse the flip there (Who may flip it). logind's change is read when it
/// comes.
pub(super) fn boot_remote(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.systemd.units = Units::new_install();
    run.systemd.write()?;
    run.fake.on_bus = false;
    run.fake.write()?;
    run.expect_description("group", "Service message", IS_OFF)?;
    run.node("switch", BOOT)?;
    run.systemd.units.session = (true, true);
    run.systemd.write()?;
    run.gone("switch", BOOT)?;
    run.gone("group", "Startup settings")?;
    run.expect_description("group", "Service message", IS_OFF)?;
    run.no_terminal()?;
    run.systemd.units = Units::all_on();
    run.systemd.write()?;
    run.node("switch", BOOT)?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    NO_NOTE
}
