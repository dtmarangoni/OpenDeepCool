use super::*;

/// An Omarchy theme switch, done as Omarchy does it: the new theme staged, then moved in. The
/// app reloads it and keeps its page (Theme; States, Live theme switch).
pub(super) fn theme(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    let current = run.omarchy_state.join("current");
    let next = current.join("next-theme");
    fs::create_dir_all(&next).map_err(|e| format!("{}: {e}", next.display()))?;
    fs::write(next.join("colors.toml"), COLORS).map_err(|e| format!("colors.toml: {e}"))?;
    fs::rename(&next, current.join("theme")).map_err(|e| format!("the theme's move: {e}"))?;
    run.logged("opendeepcool: theme: reloaded, the Omarchy theme")?;
    run.check_state("toggle button", "Configuration", State::Pressed, true)?;
    run.node("toggle button", "Rotation 0°")?;
    // The Theme card follows it live: the Omarchy theme is the system's, followed by default, so
    // the switch is on and Dark and Light are dimmed on its mode (light).
    run.expect_state("switch", "Use system theme", State::Checked, true)?;
    run.check_state("switch", "Use system theme", State::Sensitive, true)?;
    run.expect_texts("group", "Theme status", &["Using the Omarchy theme."])?;
    run.check_state("toggle button", "Light theme", State::Pressed, true)?;
    run.check_state("toggle button", "Dark theme", State::Sensitive, false)?;
    run.check_state("toggle button", "Light theme", State::Sensitive, false)?;
    // The whole Default Theme is dimmed while the system theme is followed, the saved opacity and
    // corners still shown (Phase 11a Task 50).
    for name in [
        "Opaque window",
        "Transparent window",
        "Glassy window",
        "Sharp corners",
        "Rounded corners",
    ] {
        run.check_state("toggle button", name, State::Sensitive, false)?;
    }
    run.check_state("toggle button", "Opaque window", State::Pressed, true)?;
    run.check_state("toggle button", "Rounded corners", State::Pressed, true)?;
    // The switch turned off asks the service, and keeps its state until the service confirms;
    // then OpenDeepCool's own theme shows, and the buttons come alive on `DarkTheme`.
    run.change(
        |run| run.act("switch", "Use system theme", "Toggle"),
        "SetUseSystemTheme(false)",
        |run| run.check_state("toggle button", "Light theme", State::Sensitive, false),
        |service| service.use_system_theme = false,
        |run| {
            run.expect_state("switch", "Use system theme", State::Checked, false)?;
            run.expect_state("toggle button", "Dark theme", State::Sensitive, true)?;
            run.expect_state("toggle button", "Dark theme", State::Pressed, true)?;
            run.expect_texts(
                "group",
                "Theme status",
                &["Using OpenDeepCool's dark theme."],
            )
        },
    )?;
    // Window Opacity: asked with its word, the saved one stays pressed until the service says so.
    run.change(
        |run| run.act("toggle button", "Glassy window", "Click"),
        "SetWindowOpacity(\"glassy\")",
        |run| run.check_state("toggle button", "Opaque window", State::Pressed, true),
        |service| service.window_opacity = "glassy",
        |run| run.expect_state("toggle button", "Glassy window", State::Pressed, true),
    )?;
    run.change(
        |run| run.act("toggle button", "Sharp corners", "Click"),
        "SetWindowCorners(\"sharp\")",
        |run| run.check_state("toggle button", "Rounded corners", State::Pressed, true),
        |service| service.window_corners = "sharp",
        |run| run.expect_state("toggle button", "Sharp corners", State::Pressed, true),
    )?;
    // Light: asked, the saved Dark stays pressed until the service says so.
    run.change(
        |run| run.act("toggle button", "Light theme", "Click"),
        "SetDarkTheme(false)",
        |run| run.check_state("toggle button", "Dark theme", State::Pressed, true),
        |service| service.dark_theme = false,
        |run| {
            run.expect_state("toggle button", "Light theme", State::Pressed, true)?;
            run.expect_texts(
                "group",
                "Theme status",
                &["Using OpenDeepCool's light theme."],
            )
        },
    )?;
    // And on again: the Omarchy theme is followed once more.
    run.change(
        |run| run.act("switch", "Use system theme", "Toggle"),
        "SetUseSystemTheme(true)",
        |_| Ok(()),
        |service| {
            service.use_system_theme = true;
            service.dark_theme = true;
        },
        |run| run.expect_texts("group", "Theme status", &["Using the Omarchy theme."]),
    )?;
    run.page("Display")?;
    NO_NOTE
}

/// The live tiles (Phase 11a Task 14; design: Live tiles). A picture is no accessible
/// property, and the tiles' names and descriptions are static, so layer 4 checks what a person
/// with assistive technology gets: the flags follow what the service lists, not the readings. A
/// reading the service has no value for, with no reason listed, leaves every tile as it was
/// (its picture draws dashes, which the app's own tests check); with the reason listed for the
/// modes that show it, those tiles are "Unavailable", or "Reading lost" on the saved mode's,
/// and the others are untouched; the reading and the reason gone, everything is as it was.
pub(super) fn tiles_live(run: &mut Run) -> Result<String> {
    run.reset()?;
    flags(run, &[], None)?;
    // GPU usage has no value for a moment: nothing changes for assistive technology.
    run.fake.service.readings.remove("Gpu usage");
    run.fake.write()?;
    flags(run, &[], None)?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    // The service lists the reason for the modes that show it.
    let usage = [2_u32, 4, 6, 21];
    run.fake.service.reasons = usage
        .iter()
        .map(|&m| (m, "Gpu usage: no device found".to_owned()))
        .collect();
    run.fake.service.available = MODES.into_iter().filter(|m| !usage.contains(m)).collect();
    run.fake.write()?;
    flags(run, &usage, None)?;
    // The saved mode is one of them: its tile stays usable and says what was lost.
    run.fake.service.mode = 2;
    run.fake.write()?;
    flags(run, &[4, 6, 21], Some(2))?;
    // The reading comes back, and the reason goes.
    run.fake.service.readings.insert("Gpu usage", (63, 1));
    run.fake.service.reasons.clear();
    run.fake.service.available = MODES.to_vec();
    run.fake.write()?;
    flags(run, &[], None)?;
    run.check_texts("grid cell", "Load: GPU", &["GPU"])?;
    NO_NOTE
}

pub(super) fn tile_cost(run: &mut Run) -> Result<String> {
    let pid = run
        .app_pid
        .ok_or("--app-pid is needed for the tiles' cost")?;
    let levels = |n: u32| -> Vec<f64> {
        (0..14)
            .map(|c| (f64::from((n + c) % 20) + 1.0) / 21.0)
            .collect()
    };
    let mut notes = Vec::new();
    let mut measured = Vec::new();
    for (state, ceiling) in COST_CEILING {
        run.reset()?;
        run.fake.service.readings.insert("CPU Usage", (150, 1));
        let used = match state {
            // Readings once a second, one of them 100 or more: the scrolling tiles.
            Cost::Steady => {
                run.fake.write()?;
                cost(run, pid, |run, tick| {
                    if tick % 10 == 0 {
                        run.fake
                            .service
                            .readings
                            .insert("CPU Usage", (150 + i64::from(tick / 10), 1));
                        run.fake.write()?;
                    }
                    Ok(())
                })?
            }
            // A Music mode with sound, ten `Levels` a second that change each time: every tile
            // that moves, at its busiest.
            Cost::Busy => {
                run.fake.service.mode = 13;
                run.fake.write()?;
                cost(run, pid, |run, tick| {
                    run.fake.levels = Some(levels(tick));
                    run.fake.write()
                })?
            }
            // The Configuration page: no tile is on screen, so nothing moves.
            Cost::Still => {
                run.fake.write()?;
                run.page("Configuration")?;
                let used = cost(run, pid, |_, _| Ok(()))?;
                run.page("Display")?;
                used
            }
        };
        run.fake.levels = None;
        measured.push((state, used, ceiling));
        notes.push(format!("{} {:.2} % of a core", state.name(), used * 100.0));
    }
    run.reset()?;
    for (state, used, ceiling) in &measured {
        if used > ceiling {
            return Err(format!(
                "{}: {:.1} % of a core, over its ceiling of {:.0} % ({})",
                state.name(),
                used * 100.0,
                ceiling * 100.0,
                notes.join(", ")
            ));
        }
    }
    Ok(format!("{} (ceilings 100 %, 100 %, 1 %)", notes.join(", ")))
}
