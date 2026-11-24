use super::*;

/// The temperature unit, both ways (Display page, Temperature unit).
pub(super) fn unit(run: &mut Run) -> Result<String> {
    run.reset()?;
    // The unit is on the Configuration page, after Rotation.
    run.page("Configuration")?;
    for (name, other, unit) in [
        ("Fahrenheit", "Celsius", "fahrenheit"),
        ("Celsius", "Fahrenheit", "celsius"),
    ] {
        run.change(
            |run| run.act("toggle button", name, "Click"),
            &format!("SetTemperatureUnit({unit:?})"),
            |run| {
                run.check_state("toggle button", other, State::Pressed, true)?;
                run.check_state("toggle button", name, State::Pressed, false)
            },
            |service| service.unit = unit,
            |run| {
                run.expect_state("toggle button", name, State::Pressed, true)?;
                run.expect_state("toggle button", other, State::Pressed, false)
            },
        )?;
    }
    run.page("Display")?;
    NO_NOTE
}

/// Each rotation (Configuration page, Display, Rotation).
pub(super) fn rotation(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    let mut saved = 0_u8;
    for turn in [1_u8, 2, 3, 0] {
        let name = format!("Rotation {}°", u32::from(turn) * 90);
        let before = format!("Rotation {}°", u32::from(saved) * 90);
        run.change(
            |run| run.act("toggle button", &name, "Click"),
            &format!("SetRotation({turn})"),
            |run| {
                run.check_state("toggle button", &before, State::Pressed, true)?;
                run.check_state("toggle button", &name, State::Pressed, false)
            },
            |service| service.rotation = turn,
            |run| {
                run.expect_state("toggle button", &name, State::Pressed, true)?;
                run.expect_state("toggle button", &before, State::Pressed, false)
            },
        )?;
        saved = turn;
    }
    run.page("Display")?;
    NO_NOTE
}

/// The GPU: Automatic first and the default, naming the GPU it reads; a GPU chosen by hand,
/// then Automatic again (Configuration page, GPU).
pub(super) fn gpu(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    run.open("GPU")?;
    run.expect_items(
        "GPU",
        &[
            ("Automatic", ""),
            (DEDICATED_LABEL, ""),
            (INTEGRATED_LABEL, ""),
        ],
    )?;
    run.change(
        |run| run.pick("GPU", INTEGRATED_LABEL),
        &format!("ChooseGpu({:?})", INTEGRATED.pci),
        |run| run.check_texts("combo box", "GPU", &["Automatic"]),
        |service| {
            service.chosen_gpu = INTEGRATED.pci;
            service.gpu_in_use = INTEGRATED.pci;
        },
        |run| {
            run.expect_texts("combo box", "GPU", &[INTEGRATED_LABEL])?;
            run.expect_description("combo box", "GPU", "")?;
            // Chosen by hand, the GPU is named on the button alone.
            run.gone("group", "GPU in use")
        },
    )?;
    run.open("GPU")?;
    run.change(
        |run| run.pick("GPU", "Automatic"),
        "ChooseGpu(\"\")",
        |run| run.check_texts("combo box", "GPU", &[INTEGRATED_LABEL]),
        |service| {
            service.chosen_gpu = "";
            service.gpu_in_use = DEDICATED.pci;
        },
        |run| {
            run.expect_texts("combo box", "GPU", &["Automatic"])?;
            run.expect_texts(
                "group",
                "GPU in use",
                &[&format!("Using {DEDICATED_LABEL}.")],
            )?;
            run.expect_description("combo box", "GPU", &format!("Using {DEDICATED_LABEL}"))
        },
    )?;
    run.page("Display")?;
    NO_NOTE
}

/// A GPU whose kind query failed, with the unknown ones and its note; a GPU without a reading
/// rule last, with its note; chosen, its hint, and the GPU tiles disabled with the reason
/// (Configuration page, GPU; States, A GPU without a reading rule in use).
pub(super) fn gpu_notes(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.service.gpus = vec![DEDICATED, INTEGRATED, NO_RULE, QUERY_FAILED];
    run.fake.write()?;
    run.page("Configuration")?;
    run.open("GPU")?;
    run.expect_items(
        "GPU",
        &[
            ("Automatic", ""),
            (DEDICATED_LABEL, ""),
            (INTEGRATED_LABEL, ""),
            (QUERY_FAILED_LABEL, "driver couldn't be asked"),
            (NO_RULE_LABEL, "no readings"),
        ],
    )?;
    let reasons: Vec<(u32, String)> = GPU_MODES
        .iter()
        .map(|&(mode, metric)| (mode, format!("{metric}: no reading for this driver")))
        .collect();
    run.change(
        |run| run.pick("GPU", NO_RULE_LABEL),
        &format!("ChooseGpu({:?})", NO_RULE.pci),
        |run| run.check_texts("combo box", "GPU", &["Automatic"]),
        |service| {
            service.chosen_gpu = NO_RULE.pci;
            service.gpu_in_use = NO_RULE.pci;
            service.reasons = reasons.iter().cloned().collect();
            service.available = MODES
                .into_iter()
                .filter(|m| !reasons.iter().any(|(r, _)| r == m))
                .collect();
        },
        |run| {
            run.expect_texts("combo box", "GPU", &[NO_RULE_LABEL])?;
            run.expect_description("combo box", "GPU", "no readings")?;
            run.node(
                "label",
                "No reading for this GPU's driver: the GPU options are unavailable.",
            )
            .map(|_| ())
        },
    )?;
    run.page("Display")?;
    for (mode, metric) in GPU_MODES {
        let name = tile(mode);
        run.expect_state("grid cell", &name, State::Sensitive, false)?;
        run.expect_description(
            "grid cell",
            &name,
            &format!("{}: no reading for this driver", shown_metric(metric)),
        )?;
    }
    NO_NOTE
}

/// One GPU: no GPU row at all (States, One GPU).
pub(super) fn one_gpu(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.service.gpus = vec![DEDICATED];
    run.fake.write()?;
    run.page("Configuration")?;
    run.gone("combo box", "GPU")?;
    run.gone("group", "GPU in use")?;
    run.page("Display")?;
    NO_NOTE
}

/// The peak dots off and on, and each fall at each of its five stops, by name (Configuration
/// page, Music). AT-SPI reads the switch's handle, which moves at the press (GTK's delayed
/// state, decisions log D63), and a scale follows the drag (D61), so neither is held: the call
/// and the confirmed state are checked.
pub(super) fn music(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.page("Configuration")?;
    for on in [false, true] {
        run.change(
            |run| run.act("switch", "Peak dots", "Toggle"),
            &format!("SetMusicPeaks({on})"),
            |_| Ok(()),
            |service| service.peaks = on,
            |run| {
                run.expect_state("switch", "Peak dots", State::Checked, on)?;
                thread::sleep(HOLD);
                run.check_state("switch", "Peak dots", State::Checked, on)
            },
        )?;
    }
    for (scale, call, saved) in [
        ("Bar fall", "SetMusicBarFall", 2_usize),
        ("Peak fall", "SetMusicPeakFall", 1),
    ] {
        let order: Vec<usize> = (0..STOPS.len())
            .filter(|&s| s != saved)
            .chain([saved])
            .collect();
        for stop in order {
            let (bus, word) = STOPS[stop];
            run.change(
                |run| {
                    let node = run.node("slider", scale)?;
                    run.driver.set_value(&node, stop as f64)
                },
                &format!("{call}({bus:?})"),
                |_| Ok(()),
                |service| {
                    if scale == "Bar fall" {
                        service.bar_fall = bus;
                    } else {
                        service.peak_fall = bus;
                    }
                },
                |run| run.expect_value("slider", scale, stop as f64, word),
            )?;
        }
    }
    run.page("Display")?;
    NO_NOTE
}

/// Refresh: its call, and "Refreshed" for a moment (Refresh).
pub(super) fn refresh(run: &mut Run) -> Result<String> {
    run.reset()?;
    let mark = run.fake.mark()?;
    run.act("button", "Refresh display", "Click")?;
    run.fake.called(mark, "Refresh()")?;
    run.expect_texts("group", "Change feedback", &["Refreshed"])?;
    // About 1.5 s, then gone.
    run.gone("group", "Change feedback")?;
    let changes = run.fake.changes_since(mark)?;
    if changes != ["Refresh()"] {
        return Err(format!("the fake got {changes:?}"));
    }
    NO_NOTE
}

/// The shortcuts' actions, which the window offers AT-SPI: Configuration, Back and Refresh
/// (Keyboard, Shortcuts). A key typed through AT-SPI's device event controller is tried too.
pub(super) fn shortcuts(run: &mut Run) -> Result<String> {
    run.reset()?;
    let window = run.node("window", "OpenDeepCool")?;
    run.driver.act(&window, "win.configuration")?;
    run.expect_state("toggle button", "Configuration", State::Pressed, true)?;
    run.node("toggle button", "Rotation 0°")?;
    run.driver.act(&window, "win.back")?;
    run.expect_state("toggle button", "Configuration", State::Pressed, false)?;
    run.node("grid", "Load options")?;
    let mark = run.fake.mark()?;
    run.driver.act(&window, "win.refresh")?;
    run.fake.called(mark, "Refresh()")?;
    run.expect_texts("group", "Change feedback", &["Refreshed"])?;
    run.gone("group", "Change feedback")?;
    // F5, typed through the registry (XK_F5 = 0xffc2).
    let mark = run.fake.mark()?;
    let typed = run.driver.key_sym(0xffc2);
    thread::sleep(Duration::from_secs(1));
    let reached = run
        .fake
        .changes_since(mark)?
        .iter()
        .any(|c| c == "Refresh()");
    Ok(match (typed, reached) {
        (_, true) => "F5 typed through AT-SPI reached the app".to_owned(),
        _ => "keys typed through AT-SPI don't reach a Wayland app: their bindings are checked \
              in-process (decisions log D71)"
            .to_owned(),
    })
}

/// Refused changes: the control stays on the saved value, and the feedback says "Not changed."
/// or "Not refreshed." with the service's reason, or the app's own for NotAuthorized (Refused
/// changes).
pub(super) fn refused(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.refuse = Some("ModeUnavailable: Gpu usage: no device found".to_owned());
    run.fake.write()?;
    run.refused(
        |run| run.act("grid cell", "Load: GPU", "tile.activate"),
        "SetMode(2) refused ModeUnavailable",
        "Not changed. GPU load: no device found",
    )?;
    thread::sleep(HOLD);
    run.check_state("grid cell", "Load: CPU", State::Selected, true)?;
    run.check_state("grid cell", "Load: GPU", State::Selected, false)?;
    run.fake.refuse = Some("NotAuthorized: this session isn't the active one".to_owned());
    run.fake.write()?;
    const NOT_YOURS: &str = "Not changed. This session may not change settings.";
    run.refused(
        |run| run.act("button", "Refresh display", "Click"),
        "Refresh() refused NotAuthorized",
        "Not refreshed. This session may not change settings.",
    )?;
    run.page("Configuration")?;
    run.refused(
        |run| run.act("toggle button", "Fahrenheit", "Click"),
        "SetTemperatureUnit(\"fahrenheit\") refused NotAuthorized",
        NOT_YOURS,
    )?;
    run.expect_state("toggle button", "Celsius", State::Pressed, true)?;
    run.refused(
        |run| run.act("toggle button", "Rotation 90°", "Click"),
        "SetRotation(1) refused NotAuthorized",
        NOT_YOURS,
    )?;
    run.expect_state("toggle button", "Rotation 0°", State::Pressed, true)?;
    run.check_state("toggle button", "Rotation 90°", State::Pressed, false)?;
    run.refused(
        |run| {
            let node = run.node("slider", "Bar fall")?;
            run.driver.set_value(&node, 4.0)
        },
        "SetMusicBarFall(\"faster\") refused NotAuthorized",
        NOT_YOURS,
    )?;
    run.expect_value("slider", "Bar fall", 2.0, "Moderate")?;
    run.refused(
        |run| run.act("switch", "Peak dots", "Toggle"),
        "SetMusicPeaks(false) refused NotAuthorized",
        NOT_YOURS,
    )?;
    run.expect_state("switch", "Peak dots", State::Checked, true)?;
    run.open("GPU")?;
    run.refused(
        |run| run.pick("GPU", INTEGRATED_LABEL),
        &format!("ChooseGpu({:?}) refused NotAuthorized", INTEGRATED.pci),
        NOT_YOURS,
    )?;
    run.expect_texts("combo box", "GPU", &["Automatic"])?;
    run.fake.refuse = None;
    run.fake.write()?;
    run.page("Display")?;
    NO_NOTE
}
