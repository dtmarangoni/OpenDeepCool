use super::*;

/// Service not running, at the start: the fake hasn't taken the bus, and systemd answers as a
/// new install. The off screen, "OpenDeepCool is off", with the switch off and usable, and
/// no terminal commands (they are in the README; owner, 2026-11-19; The off screen; D10).
pub(super) fn service_down(run: &mut Run) -> Result<String> {
    run.expect_description("group", "Service message", IS_OFF)?;
    run.check_state("switch", BOOT, State::Sensitive, true)?;
    run.check_state("switch", BOOT, State::Checked, false)?;
    run.expect_state("switch", BOOT, State::Focused, true)?;
    run.gone("group", "Start at boot status")?;
    run.no_terminal()?;
    run.node("group", "Startup settings")?;
    // The gear stays, dimmed and not pressed; Refresh belongs to the Display page, which the off
    // screen replaces; the close icon always works.
    run.check_state("toggle button", "Configuration", State::Sensitive, false)?;
    run.check_state("toggle button", "Configuration", State::Pressed, false)?;
    run.check_state("button", "Close", State::Sensitive, true)?;
    run.gone("button", "Refresh display")?;
    run.expect_texts("group", "Cooler status", &["Cooler: unknown"])?;
    run.expect_texts("group", "Service status", &["Service off"])?;
    run.expect_description("image", "Cooler display preview", "No picture")?;
    run.gone("grid cell", "Load: CPU")?;
    let calls = run.fake.calls_since(0)?;
    if !calls.is_empty() {
        return Err(format!("calls with no service: {calls:?}"));
    }
    // systemd read without a password: each unit's file state and run state, and the session.
    let read = run.systemd.calls_since((0, 0))?;
    for call in [
        "system GetUnitFileState(\"opendeepcool.service\")",
        "system LoadUnit(\"opendeepcool.service\")",
        "user GetUnitFileState(\"opendeepcool-audio.service\")",
        "user LoadUnit(\"opendeepcool-audio.service\")",
        "GetSession(\"auto\")",
    ] {
        if !read.iter().any(|c| c == call) {
            return Err(format!("{call} not read: {read:?}"));
        }
    }
    if read.iter().any(|c| c.contains("Subscribe")) {
        return Err(format!("subscribed with no job: {read:?}"));
    }
    // No Omarchy folder: the default theme (Theme).
    run.logged("opendeepcool: theme: no Omarchy theme")?;
    NO_NOTE
}

/// The service appears with a first start's state: the pages come back with no action, and
/// show the first start's defaults (States, First start).
pub(super) fn first_start(run: &mut Run) -> Result<String> {
    let mark = run.fake.mark()?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    // Watched, then read, then the frames (Talking to the service).
    let calls = run.fake.calls_since(mark)?;
    if calls.get(..3) != Some(&["Watch()", "GetState()", "WatchFrames()"].map(String::from)[..]) {
        return Err(format!("the app's first calls: {calls:?}"));
    }
    run.gone("group", "Service message")?;
    run.expect_texts("group", "Cooler status", &["Cooler connected"])?;
    run.expect_texts("group", "Service status", &["Service running"])?;
    run.gone("group", "Music sound status")?;
    run.expect_description("image", "Cooler display preview", "Load · CPU")?;
    run.node("label", "Load · CPU")?;
    run.check_state("toggle button", "Configuration", State::Sensitive, true)?;
    run.check_state("toggle button", "Configuration", State::Pressed, false)?;
    run.check_state("button", "Close", State::Sensitive, true)?;
    run.check_state("button", "Refresh display", State::Sensitive, true)?;
    run.page("Configuration")?;
    for degrees in [0, 90, 180, 270] {
        let name = format!("Rotation {degrees}°");
        run.check_state("toggle button", &name, State::Pressed, degrees == 0)?;
    }
    run.check_state("toggle button", "Celsius", State::Pressed, true)?;
    run.check_state("toggle button", "Fahrenheit", State::Pressed, false)?;
    // The Theme card: no Omarchy theme and no portal here, so no system theme can be found: the
    // switch off and dimmed, Dark pressed and both buttons usable, and the warning.
    run.check_state("switch", "Use system theme", State::Checked, false)?;
    run.check_state("switch", "Use system theme", State::Sensitive, false)?;
    run.check_state("toggle button", "Dark theme", State::Pressed, true)?;
    run.check_state("toggle button", "Light theme", State::Pressed, false)?;
    run.check_state("toggle button", "Dark theme", State::Sensitive, true)?;
    run.expect_texts(
        "group",
        "Theme status",
        &["OpenDeepCool can't find the system's theme and is using its own."],
    )?;
    run.expect_texts("combo box", "GPU", &["Automatic"])?;
    run.expect_description("combo box", "GPU", &format!("Using {DEDICATED_LABEL}"))?;
    run.expect_texts(
        "group",
        "GPU in use",
        &[&format!("Using {DEDICATED_LABEL}.")],
    )?;
    run.check_state("switch", "Peak dots", State::Checked, true)?;
    run.expect_value("slider", "Bar fall", 2.0, "Moderate")?;
    run.expect_value("slider", "Peak fall", 1.0, "Slow")?;
    run.page("Display")?;
    NO_NOTE
}

/// The design's accessible names that always show, each once, with its role as GTK gives it
/// (Accessible names; the others are checked in their own states).
pub(super) fn names(run: &mut Run) -> Result<String> {
    run.reset()?;
    let owned = |list: &[(&'static str, &str)]| -> Vec<(&'static str, String)> {
        list.iter()
            .map(|(role, name)| (*role, (*name).to_owned()))
            .collect()
    };
    let always = owned(&[
        ("toggle button", "Configuration"),
        ("button", "Close"),
        ("image", "Cooler display preview"),
        ("group", "Cooler status"),
        ("group", "Service status"),
    ]);
    let mut display = owned(&[
        ("button", "Refresh display"),
        ("grid", "Load options"),
        ("grid", "Temperature options"),
        ("grid", "Power options"),
        ("grid", "Music options"),
    ]);
    for mode in MODES {
        // Each tile, and its picture named as its tile (decisions log D57).
        display.push(("grid cell", tile(mode)));
        display.push(("image", tile(mode)));
    }
    let configuration = owned(&[
        ("button", "Back"),
        ("group", "Theme settings"),
        ("switch", "Use system theme"),
        ("toggle button", "Dark theme"),
        ("toggle button", "Light theme"),
        // The Default Theme subcategory (Phase 11a Task 50).
        ("group", "Default Theme"),
        ("group", "Appearance"),
        ("group", "Window Opacity"),
        ("group", "Window Corners"),
        ("toggle button", "Opaque window"),
        ("toggle button", "Transparent window"),
        ("toggle button", "Glassy window"),
        ("toggle button", "Sharp corners"),
        ("toggle button", "Rounded corners"),
        ("group", "Theme status"),
        ("group", "Display settings"),
        ("group", "Music settings"),
        ("toggle button", "Rotation 0°"),
        ("toggle button", "Rotation 90°"),
        ("toggle button", "Rotation 180°"),
        ("toggle button", "Rotation 270°"),
        ("toggle button", "Celsius"),
        ("toggle button", "Fahrenheit"),
        ("combo box", "GPU"),
        ("group", "GPU in use"),
        ("switch", "Peak dots"),
        ("slider", "Bar fall"),
        ("slider", "Peak fall"),
        ("group", "Startup settings"),
        ("switch", BOOT),
    ]);
    let check = |run: &Run, names: &[(&str, String)]| -> Result<()> {
        let tree = run.driver.tree()?;
        for (role, name) in names {
            let count = tree
                .iter()
                .filter(|(_, n)| n.role == *role && n.name == *name)
                .count();
            if count != 1 {
                return Err(format!("{role} {name:?} shown {count} times"));
            }
        }
        Ok(())
    };
    check(run, &always)?;
    check(run, &display)?;
    run.page("Configuration")?;
    check(run, &always)?;
    check(run, &configuration)?;
    run.page("Display")?;
    NO_NOTE
}

/// The frames the service sends reach the app, which logs each with its time (`--frame-log`,
/// for the real run's comparison with the dev recording; decisions log D84).
pub(super) fn frame_log(run: &mut Run) -> Result<String> {
    run.reset()?;
    for lit in [0_usize, 195, 97] {
        let mut dots = vec!['0'; 196];
        dots[lit] = '1';
        let dots: String = dots.into_iter().collect();
        run.fake.frame = Some(dots.clone());
        run.fake.write()?;
        wait(
            &format!("the app logging the frame with dot {lit} lit"),
            || {
                let log = fs::read_to_string(&run.frame_log)
                    .map_err(|e| format!("{}: {e}", run.frame_log.display()))?;
                let last = log.lines().last().unwrap_or("");
                let (time, logged) = last.split_once(' ').unwrap_or(("", ""));
                Ok((logged == dots && time.parse::<u64>().is_ok()).then_some(()))
            },
        )?;
    }
    NO_NOTE
}

/// Each of the 25 tiles asks for its own mode, once, and shows as chosen only once the service
/// confirms it; the preview's caption follows (Display page, Selection; Coverage check, One
/// choice, one mode). Music modes add the sound's status (Preview column, Status line).
pub(super) fn tiles(run: &mut Run) -> Result<String> {
    run.reset()?;
    let mut saved = 1;
    for mode in MODES.into_iter().skip(1).chain([1]) {
        let (name, before) = (tile(mode), tile(saved));
        run.change(
            |run| run.act("grid cell", &name, "tile.activate"),
            &format!("SetMode({mode})"),
            |run| {
                run.check_state("grid cell", &before, State::Selected, true)?;
                if mode == saved {
                    Ok(())
                } else {
                    run.check_state("grid cell", &name, State::Selected, false)
                }
            },
            |service| service.mode = mode,
            |run| {
                run.expect_state("grid cell", &name, State::Selected, true)?;
                if mode != saved {
                    run.expect_state("grid cell", &before, State::Selected, false)?;
                }
                run.expect_description("image", "Cooler display preview", &caption(mode))?;
                if (13..=26).contains(&mode) {
                    run.expect_texts("group", "Music sound status", &["Sound connected"])
                } else {
                    run.gone("group", "Music sound status")
                }
            },
        )?;
        saved = mode;
    }
    NO_NOTE
}
