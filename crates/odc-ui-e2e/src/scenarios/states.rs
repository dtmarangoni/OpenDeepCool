use super::*;

/// The cooler's states: one banner at a time, the status and the preview's description (no line
/// of the column says it), everything still changeable; `write-failed` as connected for its
/// first 2 s (Banners; States).
pub(super) fn cooler_states(run: &mut Run) -> Result<String> {
    run.reset()?;
    const NOT_CONNECTED: (Option<&str>, &str, &str) = (
        Some(
            "The cooler isn't connected. Changes are still saved, and the cooler shows them \
             when it's back.",
        ),
        "Cooler not connected",
        "Load · CPU. Not on the cooler: it isn't connected.",
    );
    for (state, (banner, status, described)) in [
        ("not-found", NOT_CONNECTED),
        (
            "no-access",
            (
                Some(
                    "The cooler is plugged in, but the service can't open it. The udev rule \
                     that gives the service access is missing or not applied yet. Restart the \
                     PC; if that doesn't help, reinstall OpenDeepCool. The journal says more.",
                ),
                "Cooler: no access",
                "Load · CPU. Not on the cooler: the service can't open it.",
            ),
        ),
        // The dev copy's own state, and one a later service might send: as not-found.
        ("released", NOT_CONNECTED),
        ("on fire", NOT_CONNECTED),
        ("connected", (None, "Cooler connected", "Load · CPU")),
    ] {
        run.fake.service.cooler = state;
        run.fake.write()?;
        match banner {
            Some(text) => run.expect_description("group", "Cooler banner", text)?,
            None => run.gone("group", "Cooler banner")?,
        }
        run.expect_texts("group", "Cooler status", &[status])
            .map_err(|e| format!("{state}: {e}"))?;
        run.expect_description("image", "Cooler display preview", described)
            .map_err(|e| format!("{state}: {e}"))?;
        // Nothing is disabled.
        let tile = run.node("grid cell", "Load: GPU")?;
        if !run.driver.has(&tile, State::Sensitive)? {
            return Err(format!("{state}: the tiles are disabled"));
        }
    }
    run.fake.service.cooler = "write-failed";
    run.fake.write()?;
    let taken = Instant::now();
    thread::sleep(Duration::from_millis(500));
    run.gone("group", "Cooler banner")?;
    run.check_texts("group", "Cooler status", &["Cooler connected"])?;
    run.expect_description("image", "Cooler display preview", "Load · CPU")?;
    run.expect_description(
        "group",
        "Cooler banner",
        "The cooler is connected, but sending it the picture fails. The service keeps trying \
         by itself, and changes are still saved. The journal says more.",
    )?;
    let after = taken.elapsed();
    if after < Duration::from_millis(1900) {
        return Err(format!("write-failed's banner after {after:?}, not 2 s"));
    }
    run.expect_texts("group", "Cooler status", &["Cooler: sending fails"])?;
    run.expect_description(
        "image",
        "Cooler display preview",
        "Load · CPU. Not on the cooler: sending it fails.",
    )?;
    run.fake.service.cooler = "connected";
    run.fake.write()?;
    run.gone("group", "Cooler banner")?;
    Ok(format!(
        "write-failed's banner after {:.1} s",
        after.as_secs_f64()
    ))
}

/// The GPU's metrics unavailable: their tiles insensitive, "Unavailable", with the reason, and
/// each group's reasons once (States, Metric unavailable).
pub(super) fn unavailable_metric(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.fake.service.reasons = GPU_MODES
        .iter()
        .map(|&(mode, metric)| (mode, format!("{metric}: no device found")))
        .collect();
    run.fake.service.available = MODES
        .into_iter()
        .filter(|m| !GPU_MODES.iter().any(|(g, _)| g == m))
        .collect();
    // The readings the reasons are about have no value.
    run.fake
        .service
        .readings
        .retain(|metric, _| !metric.starts_with("Gpu"));
    run.fake.write()?;
    for mode in MODES {
        let name = tile(mode);
        let (_, option) = entry(mode);
        match GPU_MODES.iter().find(|(m, _)| *m == mode) {
            Some((_, metric)) => {
                run.expect_state("grid cell", &name, State::Sensitive, false)?;
                run.expect_description(
                    "grid cell",
                    &name,
                    &format!("{}: no device found", shown_metric(metric)),
                )?;
                run.check_texts("grid cell", &name, &[option, "Unavailable"])?;
                // No action while insensitive: GTK offers none (decisions log D67).
                let node = run.node("grid cell", &name)?;
                let actions = run.driver.actions(&node)?;
                if !actions.is_empty() {
                    return Err(format!("{name:?} offers {actions:?}"));
                }
            }
            None => {
                run.check_state("grid cell", &name, State::Sensitive, true)?;
                run.check_texts("grid cell", &name, &[option])?;
            }
        }
    }
    for (group, reasons) in [
        ("Load", "GPU load: no device found"),
        ("Temperature", "GPU temperature: no device found"),
        ("Power", "GPU power: no device found"),
        (
            "Music",
            "GPU load: no device found\nGPU temperature: no device found",
        ),
    ] {
        run.expect_texts("group", &format!("{group} unavailable reasons"), &[reasons])?;
    }
    run.reset()?;
    run.gone("group", "Load unavailable reasons")?;
    NO_NOTE
}

/// The saved mode loses its reading: its tile stays chosen and usable, flagged "Reading lost",
/// and the reason shows under the preview (States, The saved mode lost a reading).
pub(super) fn reading_lost(run: &mut Run) -> Result<String> {
    run.reset()?;
    let usage = [2, 4, 6, 21];
    run.fake.service.mode = 2;
    run.fake.service.reasons = usage
        .iter()
        .map(|&m| (m, "Gpu usage: no device found".to_owned()))
        .collect();
    run.fake.service.available = MODES.into_iter().filter(|m| !usage.contains(m)).collect();
    run.fake.service.readings.remove("Gpu usage");
    run.fake.write()?;
    run.expect_state("grid cell", "Load: GPU", State::Selected, true)?;
    run.check_state("grid cell", "Load: GPU", State::Sensitive, true)?;
    run.expect_texts("grid cell", "Load: GPU", &["GPU", "Reading lost"])?;
    run.expect_description(
        "grid cell",
        "Load: GPU",
        "Reading lost: GPU load: no device found",
    )?;
    run.expect_texts(
        "group",
        "Unavailable reading",
        &["GPU load: no device found. The cooler shows dashes until it's back."],
    )?;
    run.expect_state("grid cell", "Load: GPU + RAM", State::Sensitive, false)?;
    run.reset()?;
    run.gone("group", "Unavailable reading")?;
    NO_NOTE
}

/// The sound's status, only in a Music mode (Preview column, Status line).
pub(super) fn music_sound(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.gone("group", "Music sound status")?;
    run.fake.service.mode = 13;
    run.fake.write()?;
    run.expect_texts("group", "Music sound status", &["Sound connected"])?;
    run.fake.service.audio_connected = false;
    run.fake.write()?;
    run.expect_texts(
        "group",
        "Music sound status",
        &["No sound: spectrum is silent"],
    )?;
    run.fake.service.mode = 1;
    run.fake.write()?;
    run.gone("group", "Music sound status")?;
    NO_NOTE
}

/// The service crashes: systemd waits to restart it, and its name goes. The off screen, "isn't
/// running", says it failed; it returns: the pages come back with no action (The window around
/// a flip).
pub(super) fn service_stops(run: &mut Run) -> Result<String> {
    run.reset()?;
    run.systemd.units.service = ("enabled", "activating", "auto-restart");
    run.systemd.write()?;
    run.fake.on_bus = false;
    run.fake.write()?;
    run.expect_description("group", "Service message", NOT_RUNNING)?;
    run.note(
        "Not fully on: the display service failed. Turn it on to finish. The journal says why the \
         service failed.",
    )?;
    run.no_terminal()?;
    run.expect_state("toggle button", "Configuration", State::Sensitive, false)?;
    run.gone("grid cell", "Load: CPU")?;
    run.expect_texts("group", "Service status", &["Service not running"])?;
    run.expect_texts("group", "Cooler status", &["Cooler: unknown"])?;
    let mark = run.fake.mark()?;
    run.systemd.units = Units::all_on();
    run.systemd.write()?;
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    run.gone("group", "Service message")?;
    let calls = run.fake.calls_since(mark)?;
    for call in ["Watch()", "GetState()"] {
        if !calls.iter().any(|c| c == call) {
            return Err(format!("no {call} after the service came back: {calls:?}"));
        }
    }
    NO_NOTE
}

/// Not available in this session: the service refuses every read, so the window holds one
/// message and nothing else, and sends nothing; allowed again, nothing changes until the window
/// is active again, with no polling (Not available in this session).
pub(super) fn not_available(run: &mut Run) -> Result<String> {
    run.reset()?;
    let mark = run.fake.mark()?;
    run.fake.reads = false;
    run.fake.on_bus = false;
    run.fake.write()?;
    // A new owner of the service's name: the app connects again, and is refused.
    run.fake.on_bus = true;
    run.fake.write()?;
    run.expect_description(
        "group",
        "Not available in this session",
        "OpenDeepCool can be used only from the session at the PC. This session is remote or \
         not the active one.",
    )?;
    // Only the title and the close icon of the header remain.
    run.node("button", "Close")?;
    run.check_state("button", "Close", State::Sensitive, true)?;
    for (role, name) in [
        ("toggle button", "Configuration"),
        ("button", "Refresh display"),
        ("button", "Back"),
        ("image", "Cooler display preview"),
        ("grid cell", "Load: CPU"),
        ("group", "Cooler status"),
        ("group", "Service message"),
        ("switch", BOOT),
    ] {
        run.gone(role, name)?;
    }
    let window = run.node("window", "OpenDeepCool")?;
    let actions: Vec<String> = run
        .driver
        .actions(&window)?
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    for action in ["win.refresh", "win.configuration", "win.back"] {
        if actions.iter().any(|a| a == action) {
            return Err(format!("{action} offered when not available"));
        }
    }
    // Refused at its first call, `Watch()`, the app asks nothing more.
    let calls = run.fake.calls_since(mark)?;
    if !calls.iter().any(|c| c.ends_with(" refused NotAuthorized")) {
        return Err(format!("no refused read: {calls:?}"));
    }
    let changes = run.fake.changes_since(mark)?;
    if !changes.is_empty() {
        return Err(format!("calls when not available: {changes:?}"));
    }
    // Allowed again: the app doesn't poll, so nothing changes yet.
    run.fake.reads = true;
    run.fake.write()?;
    let quiet = run.fake.mark()?;
    thread::sleep(Duration::from_millis(1500));
    run.node("group", "Not available in this session")?;
    let calls = run.fake.calls_since(quiet)?;
    if !calls.is_empty() {
        return Err(format!("the app asked with no reason to: {calls:?}"));
    }
    // The window active again: it connects again, and the pages come back.
    run.activate_again()?;
    run.expect_state("grid cell", "Load: CPU", State::Selected, true)?;
    run.gone("group", "Not available in this session")?;
    let calls = run.fake.calls_since(quiet)?;
    for call in ["Watch()", "GetState()"] {
        if !calls.iter().any(|c| c == call) {
            return Err(format!("no {call} on activation: {calls:?}"));
        }
    }
    NO_NOTE
}
